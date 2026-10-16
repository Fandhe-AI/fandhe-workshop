//! 停止・移動・復帰・確認の操作の連打の直後に、保存を待たずに再読み込みしても直後の状態へ戻ることの試験（TASK-321、SAVE-5）。
//!
//! 操作の直後の包みを `PendingSave` に積み、保存の遅延を待たずに `pagehide` 相当の `flush` で localStorage の偽物へ
//! 同期に書き、新しいページを模して同じ保存先から読み戻し、`load_history`（段 1〜8）で開いて `reload` する。
//! 期待は core の `reload_after_control.rs` と同じ（世代 +1 とログ末尾の `Reloaded` 1 件を除いて直後の状態と等しく、
//! 操作の前の宛先は `Ignored(Stale)`）で、ここでは web の保存の経路（保留・書き出し・保存先の選択）を通すことを確かめる。
//! ブラウザの結線（受講中の `ops` の記録・変更ごとの積み込み・読み込み時の復元）は TASK-319 の範囲で、本試験は触れない。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::storage::{HistoryStore, StorageKind};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::history_envelope::{build_envelope, record_draft, record_submit};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::ops_record::{apply_recorded, OpsRecorder};
use fandhe_workshop_core::session::{reload, SessionState};
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{apply, load_ir};
use fandhe_workshop_web::save_pending::PendingSave;
use fandhe_workshop_web::storage_select::HistoryStorage;
use std::collections::BTreeMap;
use std::path::PathBuf;
use storage_fakes::{fixture, key, FakeIdb};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 契約の試料から教材（IR）と最初の状態を読む（core の `contract_sample` と同じファイル）。
fn load() -> (Ir, SessionState) {
    let bytes = std::fs::read(testdata().join("contract/event-sequence.json")).expect("sample");
    let sample: serde_json::Value = serde_json::from_slice(&bytes).expect("parse sample");
    let ir_path = sample["ir"].as_str().expect("ir path");
    let ir = load_ir(&std::fs::read(testdata().join(ir_path)).expect("read ir")).expect("ir");
    let initial = serde_json::from_value(sample["initial"].clone()).expect("initial");
    (ir, initial)
}

/// 記録しながら進める小さな経路（core の `RecordPath` に当たる）。
struct Path<'a> {
    ir: &'a Ir,
    state: SessionState,
    rec: OpsRecorder,
}

impl Path<'_> {
    fn op(&mut self, op: Op) {
        match &op {
            Op::Input(Input::Command(s) | Input::Text(s)) => {
                record_draft(&mut self.state, &mut self.rec, s.clone());
                assert!(record_submit(&mut self.state, &mut self.rec, s.clone()));
                self.state = apply(self.ir, self.state.clone(), op).0;
            }
            _ => {
                self.state = apply_recorded(self.ir, self.state.clone(), &mut self.rec, op).0;
            }
        }
    }

    fn tick(&mut self) -> TickAddr {
        let addr = self.next_tick().expect("next tick");
        self.op(Op::Tick(addr.clone()));
        addr
    }

    fn next_tick(&self) -> Option<TickAddr> {
        Some(TickAddr {
            generation: self.state.run.generation,
            step: self.state.run.current.clone()?,
            index: self.state.run.busy.as_ref()?.done,
        })
    }
}

fn busy_path<'a>(ir: &'a Ir, initial: &SessionState, ticks: u32) -> Path<'a> {
    let mut path = Path {
        ir,
        state: initial.clone(),
        rec: OpsRecorder::new(),
    };
    path.op(Op::Input(Input::Command("/make-note first-note".into())));
    for _ in 0..ticks {
        path.tick();
    }
    path
}

/// 保存を待たずに再読み込みする。積む → `flush` → 新しいページで読み戻す → 開く → `reload` の順で、再読み込みの後の状態を返す。
fn save_without_waiting_and_reload(
    ir: &Ir,
    initial: &SessionState,
    path: Path<'_>,
) -> SessionState {
    let Path { state, rec, .. } = path;
    let env = build_envelope(ir, "s-1", rec.into_ops(), state.clone()).expect("build");
    let value = String::from_utf8(export_bytes(&env).expect("export")).expect("utf8");

    let f = fixture(false);
    let k = key("p");
    let pending = PendingSave::new();
    pending.stage(k.clone(), value);
    // 保存の遅延のタイマーは無い。`flush` まで保存先には何も書かれない。
    assert_eq!(f.local.raw(&k.render()), None);
    let out = pending.flush(&f.storage);
    assert_eq!((out.written, out.failed), (1, 0));
    assert_eq!(f.storage.kind_for(&k), StorageKind::LocalStorage);

    // 新しいページ。同じ localStorage の上に保存先を作り直して読む。
    let reopened: HistoryStorage<_, FakeIdb> =
        HistoryStorage::new(f.local.clone(), None, BTreeMap::new());
    let bytes = reopened.read(&k).expect("read").expect("saved value");
    let opening = record_versions(ir, &content_hash(ir).expect("hash")).expect("versions");
    let opened = load_history(
        bytes.as_bytes(),
        ir,
        &opening,
        &OpeningCaps::current(),
        initial.clone(),
    );
    let Opened::Open { state: restored } = opened else {
        panic!("history must open: {:?}", opened.verdict());
    };
    assert_eq!(restored, state, "restored state differs from the saved one");
    reload(restored).0
}

/// 世代の +1 とログ末尾の `Reloaded` 1 件だけが直後の状態と違うことを確かめる。
fn assert_only_reload_differs(before: &SessionState, after: &SessionState) {
    let (mut a, mut b) = (after.clone(), before.clone());
    a.run.generation = Generation(0);
    b.run.generation = Generation(0);
    a.log.clear();
    b.log.clear();
    assert_eq!(a, b, "something other than generation and log changed");
    assert_eq!(after.run.generation.0, before.run.generation.0 + 1);
    let n = before.log.len();
    assert_eq!(after.log.get(..n), Some(before.log.as_slice()));
    assert!(matches!(after.log.get(n..), Some([Event::Reloaded { .. }])));
}

/// 古い宛先が `Ignored(Stale)` 1 件だけで、ログ以外の状態を変えないことを確かめて新しい状態を返す。
fn assert_stale_ignored(ir: &Ir, state: SessionState, old: TickAddr) -> SessionState {
    let (after, events) = apply(ir, state.clone(), Op::Tick(old));
    assert!(matches!(
        events.as_slice(),
        [Event::Ignored {
            reason: IgnoreReason::Stale,
            ..
        }]
    ));
    let mut expected = state;
    expected.log = after.log.clone();
    assert_eq!(after, expected);
    after
}

#[test]
fn 停止の直後に保存を待たず再読み込みしても停止の直後の状態へ戻る() {
    let (ir, initial) = load();
    let mut path = busy_path(&ir, &initial, 2);
    let old = path.next_tick().expect("next tick");
    path.op(Op::Control(Control::Stop));
    let stopped = path.state.clone();
    assert_eq!(stopped.run.busy, None);
    let reloaded = save_without_waiting_and_reload(&ir, &initial, path);
    assert_only_reload_differs(&stopped, &reloaded);
    assert_eq!(reloaded.run.busy, None);
    assert_stale_ignored(&ir, reloaded, old);
}

#[test]
fn 移動の直後に保存を待たず再読み込みしても移動の直後の状態へ戻る() {
    let (ir, initial) = load();
    let mut path = busy_path(&ir, &initial, 2);
    let old = path.next_tick().expect("next tick");
    let g0 = path.state.run.generation.0;
    path.op(Op::Control(Control::GoTo(StepUid("main/st-2".into()))));
    let moved = path.state.clone();
    let reloaded = save_without_waiting_and_reload(&ir, &initial, path);
    assert_only_reload_differs(&moved, &reloaded);
    assert_eq!(reloaded.run.generation.0, g0 + 2);
    assert_stale_ignored(&ir, reloaded, old);
}

#[test]
fn 復帰の直後に保存を待たず再読み込みしても復帰の直後の状態へ戻る() {
    let (ir, initial) = load();
    let mut path = Path {
        ir: &ir,
        state: initial.clone(),
        rec: OpsRecorder::new(),
    };
    path.op(Op::Control(Control::Save));
    path.op(Op::Input(Input::Command("/make-note first-note".into())));
    path.tick();
    path.tick();
    let old0 = path.next_tick().expect("next tick");
    let old1 = TickAddr {
        index: old0.index + 1,
        ..old0.clone()
    };
    path.op(Op::Control(Control::Restore(CheckpointId(1))));
    let restored = path.state.clone();
    assert_eq!(restored.run.busy, None);
    let reloaded = save_without_waiting_and_reload(&ir, &initial, path);
    assert_only_reload_differs(&restored, &reloaded);
    assert_eq!(reloaded.checkpoints, restored.checkpoints);
    let n = reloaded.log.len();
    let s = assert_stale_ignored(&ir, reloaded, old0);
    let s = assert_stale_ignored(&ir, s, old1);
    assert_eq!(s.log.len(), n + 2);
}

#[test]
fn 連打の直後に保存を待たず再読み込みしても達成は1回で直後の状態へ戻る() {
    let (ir, initial) = load();
    let mut path = Path {
        ir: &ir,
        state: initial.clone(),
        rec: OpsRecorder::new(),
    };
    path.op(Op::Input(Input::Command("/make-note first-note".into())));
    while path.next_tick().is_some() {
        path.tick();
    }
    assert_eq!(path.state.run.busy, None);
    for _ in 0..20 {
        path.op(Op::Input(Input::Ui {
            view: "v-files".into(),
            path: "notes/draft".into(),
        }));
    }
    let burst = path.state.clone();
    let achieved = |s: &SessionState| {
        s.log
            .iter()
            .filter(|e| matches!(e, Event::Achieved { .. }))
            .count()
    };
    assert_eq!(achieved(&burst), 1);
    let reloaded = save_without_waiting_and_reload(&ir, &initial, path);
    assert_only_reload_differs(&burst, &reloaded);
    assert_eq!(achieved(&reloaded), 1);
    assert_eq!(reloaded.run.current, Some(StepUid("main/st-2".into())));
}
