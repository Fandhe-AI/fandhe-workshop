//! 停止・Step の移動・チェックポイントの復帰の直後の再読み込みを守る試験（TASK-321 1/2、SAVE-5・PoC-6 の S-6・S-7・S-19 の固定版）。
//!
//! 停止・移動・復帰は世代を上げて古い宛先の進行を無効にする操作である。その直後に、保存の遅延を待たずに
//! 再読み込みしても次の 2 つが成り立つことを、保存の経路（記録 → 包み → 書き出し → 読み込みの段 1〜8 → 再読み込み）
//! を通して確かめる。
//! - 復元して再読み込みした状態は、世代の +1 とログ末尾の `Reloaded` 1 件を除いて操作の直後の状態と等しい
//! - 操作の前に予約された宛先の進行が再読み込みの後に届いても、`Ignored(Stale)` が 1 件ずつ増えるだけで状態は変わらない
//!
//! 再読み込みの後の停止は `reload_busy.rs`（`stop_after_reload_returns_to_step_entry`）が持つので、ここでは重ねない。
//! 確認の操作の連打（S-9）は TASK-321 2/2、ブラウザでの 3 場面は web に履歴の記録・復元の結線が入る
//! （TASK-319）後に足す（`docs/testing.md` 5.69）。変異 `SAVE5-1`〜`SAVE5-3` の検出先になる。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::record_path::RecordPath;
use common::tick::next_tick;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::session::{reload, Session, SessionState};
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{apply, load_ir, Projection};

const ST2: &str = "main/st-2";

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

/// 受け付けて、`ticks` 回進めた処理中の記録の経路。
fn busy_path(ir: &Ir, ticks: u32) -> RecordPath<'_> {
    let mut path = RecordPath::new(ir, &load().initial);
    path.op(first_note());
    for _ in 0..ticks {
        let addr = next_tick(&path.state).expect("next tick");
        path.op(Op::Tick(addr));
    }
    assert!(path.state.run.busy.is_some());
    path
}

/// 操作の直後の包みを `pagehide` の書き出しと同じ形で保存し、読み込みの段 1〜8 を通して復元し、再読み込みする。
/// 復元した状態が直後の状態と完全に一致することも、ここで主張する。返すのは再読み込みの後の状態。
fn save_and_reload(ir: &Ir, path: RecordPath<'_>) -> SessionState {
    let RecordPath { state, rec, .. } = path;
    let env = build_envelope(ir, "s-1", rec.into_ops(), state.clone()).expect("build");
    let bytes = export_bytes(&env).expect("export");
    let opening = record_versions(ir, &content_hash(ir).expect("hash")).expect("versions");
    let opened = load_history(
        &bytes,
        ir,
        &opening,
        &OpeningCaps::current(),
        load().initial,
    );
    let Opened::Open { state: restored } = opened else {
        panic!("history must open: {:?}", opened.verdict());
    };
    assert_eq!(restored, state, "restored state differs from the saved one");
    reload(restored).0
}

/// 再読み込みの後の状態が、直後の状態から世代の +1 とログ末尾の `Reloaded` 1 件だけ違うことを確かめる。
fn assert_only_reload_differs(before: &SessionState, after: &SessionState) {
    let mut a = after.clone();
    let mut b = before.clone();
    a.run.generation = Generation(0);
    b.run.generation = Generation(0);
    a.log.clear();
    b.log.clear();
    assert_eq!(a, b, "something other than generation and log changed");
    assert_eq!(after.run.generation.0, before.run.generation.0 + 1);
    let n = before.log.len();
    assert_eq!(after.log.get(..n), Some(before.log.as_slice()));
    assert_eq!(
        after.log.get(n..),
        Some(
            [Event::Reloaded {
                at: Stamp {
                    generation: before.run.generation,
                    step: before.run.current.clone().expect("current"),
                },
                new_generation: after.run.generation,
            }]
            .as_slice()
        )
    );
    // 復習とログの行は、再読み込みで増えも減りもしない。
    let (p0, p1) = (
        Projection::from_events(&before.log),
        Projection::from_events(&after.log),
    );
    assert_eq!(p1.log(), p0.log());
    assert_eq!(p1.review(), p0.review());
}

/// 古い宛先を送り、`Ignored(Stale)` がちょうど 1 件で、ログ以外の状態が変わらないことを確かめて新しい状態を返す。
fn assert_stale_ignored(ir: &Ir, state: SessionState, old: TickAddr) -> SessionState {
    let (after, events) = apply(ir, state.clone(), Op::Tick(old));
    let step = state.run.current.clone().expect("current");
    assert_eq!(
        events,
        [Event::Ignored {
            at: Stamp {
                generation: state.run.generation,
                step,
            },
            reason: IgnoreReason::Stale
        }]
    );
    assert_eq!(after.log.get(state.log.len()..), Some(events.as_slice()));
    let mut expected = state;
    expected.log = after.log.clone();
    assert_eq!(after, expected);
    after
}

#[test]
fn reload_right_after_stop_restores_the_stopped_state() {
    let ir = ir();
    let mut path = busy_path(&ir, 2);
    let old = next_tick(&path.state).expect("next tick");
    let entry = path.state.entry.clone().expect("entry");
    path.op(Op::Control(Control::Stop));
    let stopped = path.state.clone();
    assert_eq!(stopped.run.busy, None);
    assert_eq!(stopped.run.generation.0, old.generation.0 + 1);
    let reloaded = save_and_reload(&ir, path);
    assert_only_reload_differs(&stopped, &reloaded);
    // 停止の直後は Step の入口で、処理中でない。
    assert_eq!(reloaded.run.busy, None);
    assert_eq!(reloaded.run.current, stopped.run.current);
    let mut expected = entry.run;
    expected.generation = reloaded.run.generation;
    assert_eq!(reloaded.run, expected);
    assert_eq!(reloaded.entry, stopped.entry);
    let after = assert_stale_ignored(&ir, reloaded.clone(), old);
    assert_eq!(after.run, reloaded.run);
}

#[test]
fn reload_right_after_move_restores_the_moved_state() {
    let ir = ir();
    for busy in [true, false] {
        let mut path = busy_path(&ir, 2);
        if !busy {
            while let Some(addr) = next_tick(&path.state) {
                path.op(Op::Tick(addr));
            }
        }
        let old = TickAddr {
            generation: path.state.run.generation,
            step: path.state.run.current.clone().expect("current"),
            index: path.state.run.busy.as_ref().map_or(99, |b| b.done),
        };
        let g0 = path.state.run.generation.0;
        path.op(Op::Control(Control::GoTo(StepUid(ST2.to_string()))));
        let moved = path.state.clone();
        assert_eq!(moved.run.current, Some(StepUid(ST2.to_string())));
        assert_eq!(moved.run.generation.0, g0 + 1, "busy={busy}");
        let reloaded = save_and_reload(&ir, path);
        assert_only_reload_differs(&moved, &reloaded);
        // 移動で +1、再読み込みで +1。
        assert_eq!(reloaded.run.generation.0, g0 + 2, "busy={busy}");
        assert_eq!(reloaded.entry, moved.entry);
        assert_eq!(reloaded.run.busy, None);
        let after = assert_stale_ignored(&ir, reloaded.clone(), old);
        assert_eq!(after.run, reloaded.run);
    }
}

#[test]
fn reload_right_after_restore_restores_the_restored_state() {
    let ir = ir();
    let mut path = RecordPath::new(&ir, &load().initial);
    // 処理に入る前に保存し、受け付けて処理中にする。
    path.op(Op::Control(Control::Save));
    path.op(first_note());
    for _ in 0..2 {
        let addr = next_tick(&path.state).expect("next tick");
        path.op(Op::Tick(addr));
    }
    assert!(path.state.run.busy.is_some());
    // 復帰の前に予約された宛先 2 件。
    let old0 = next_tick(&path.state).expect("next tick");
    let old1 = TickAddr {
        index: old0.index + 1,
        ..old0.clone()
    };
    path.op(Op::Control(Control::Restore(CheckpointId(1))));
    let restored = path.state.clone();
    assert_eq!(restored.run.busy, None);
    assert_eq!(restored.run.generation.0, old0.generation.0 + 1);
    assert!(!restored.checkpoints.is_empty());
    let reloaded = save_and_reload(&ir, path);
    assert_only_reload_differs(&restored, &reloaded);
    assert_eq!(reloaded.checkpoints, restored.checkpoints);
    assert_eq!(reloaded.next_checkpoint, restored.next_checkpoint);
    assert_eq!(reloaded.drafts, restored.drafts);
    assert_eq!(reloaded.hints_opened, restored.hints_opened);
    assert_eq!(reloaded.entry, restored.entry);
    let n = reloaded.log.len();
    let s = assert_stale_ignored(&ir, reloaded.clone(), old0);
    let s = assert_stale_ignored(&ir, s, old1);
    assert_eq!(s.log.len(), n + 2);
    assert_eq!(s.run, reloaded.run);
}

#[test]
fn session_entry_agrees_for_all_three_scenes() {
    let ir = ir();
    for scene in 0..3 {
        let mut path = busy_path(&ir, 2);
        let old = next_tick(&path.state).expect("next tick");
        path.op(Op::Control(match scene {
            0 => Control::Stop,
            1 => Control::GoTo(StepUid(ST2.to_string())),
            _ => Control::Save,
        }));
        let (pure_state, _) = reload(path.state.clone());
        let expected = assert_stale_ignored(&ir, pure_state, old.clone());
        let mut session = Session::from_state(ir.clone(), path.state);
        session.reload();
        let events = session.apply(Op::Tick(old)).to_vec();
        assert!(
            matches!(
                events.as_slice(),
                [Event::Ignored {
                    reason: IgnoreReason::Stale,
                    ..
                }]
            ),
            "scene={scene}"
        );
        assert_eq!(session.state(), &expected, "scene={scene}");
    }
}
