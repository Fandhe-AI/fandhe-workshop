//! 途中開始の省略の件数と保持の試験（TASK-217 2/2、EVT-21・MOD-6・SAVE-4）。
//!
//! 1/2（`begin_skip.rs`）が定めた省略の規則を、次の 2 点で確定させる。
//! - 件数: 飛ばした Step の数と、イベント・ログ・状態の 3 つの `Skipped` の件数がちょうど一致する。
//!   同じ開始点への再度の開始では重複せず、開始の後に進めても飛ばした Step は達成にならない
//! - 保持: 再読み込み・書き出し→読み込み・`ops` の適用し直しの前後で、件数も状態も変わらない
//!
//! 保存の経路（記録 → 包み → 書き出し → 読み込みの段 1〜8 → 起動の分岐）は本番と同じ関数を通す。
//! ブラウザでの再読み込みの確認は TASK-247、web の起動を `Control::Begin` へ置き換える作業は TASK-254 が担う。
//! 変異 `EVT21-5`〜`EVT21-7` の検出先になる。

mod common;

use common::min_ir;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::instance::status_of;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{apply, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{Projection, Session};

/// 5 Step の章。`main/st-1`〜`main/st-5`、開始点 `s-<n>` は `main/st-<n>`（`begin_skip.rs` と同じ形）。
fn ir5() -> Ir {
    let mut ir = min_ir();
    let proto = ir.nodes[2].clone();
    for n in 4..=5 {
        let mut node = proto.clone();
        node.uid = format!("main/st-{n}");
        node.step.id = format!("st-{n}");
        ir.nodes.push(node);
    }
    ir.starts = (1..=5)
        .map(|n| Start {
            id: format!("s-{n}"),
            node: format!("main/st-{n}"),
            effects: vec![],
            guidance: None,
            requires: None,
            unmet: None,
        })
        .collect();
    ir
}

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

fn approve() -> Op {
    Op::Input(Input::Approval {
        decision: Decision::Approve,
        target: None,
    })
}

fn uid(n: usize) -> StepUid {
    StepUid(format!("main/st-{n}"))
}

fn skipped_in(events: &[Event]) -> Vec<StepUid> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect()
}

fn skipped_by_status(ir: &Ir, state: &SessionState) -> Vec<StepUid> {
    (1..=5)
        .filter(|n| status_of(ir, &state.run, &uid(*n)) == StepStatus::Skipped)
        .map(uid)
        .collect()
}

/// 状態の `Skipped` が `expected` の Step だけで、ログの件数と一致し、`Achieved` にならないことを確かめる。
fn assert_skips(ir: &Ir, state: &SessionState, expected: &[usize]) {
    let want: Vec<StepUid> = expected.iter().map(|n| uid(*n)).collect();
    assert_eq!(skipped_by_status(ir, state), want, "status");
    assert_eq!(skipped_in(&state.log), want, "log");
    for n in expected {
        assert_ne!(status_of(ir, &state.run, &uid(*n)), StepStatus::Achieved);
    }
}

fn versions(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 書き出して、読み込みの段 1〜8 を通して開く。書き出したバイト列と開いた状態を返す。
fn export_and_open(ir: &Ir, s: &RecordedSession) -> (Vec<u8>, SessionState) {
    let env = s.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    let opened = load_history(
        &bytes,
        ir,
        &versions(ir),
        &OpeningCaps::current(),
        SessionState::default(),
    );
    let Opened::Open { state } = opened else {
        panic!("history must open: {:?}", opened.verdict());
    };
    (bytes, state)
}

/// 起動の分岐で復元した実行環境。
fn boot(ir: &Ir, bytes: &[u8]) -> RecordedSession {
    let outcome = boot_session(
        Some(bytes),
        ir.clone(),
        &versions(ir),
        &OpeningCaps::current(),
        SessionState::default(),
        "s-1",
    );
    let BootOutcome::Restored { session, .. } = outcome else {
        panic!("must restore");
    };
    session
}

fn recorded(ir: &Ir) -> RecordedSession {
    RecordedSession::new(
        Session::from_state(ir.clone(), SessionState::default()),
        "s-1",
    )
}

/// 世代の +1 とログ末尾の `Reloaded` 1 件を除いて、`before` と `after` が等しい。
fn assert_only_reload_differs(before: &SessionState, after: &SessionState) {
    let (mut a, mut b) = (after.clone(), before.clone());
    a.run.generation = Generation(0);
    b.run.generation = Generation(0);
    a.log.clear();
    b.log.clear();
    assert_eq!(a, b);
    assert_eq!(after.run.generation.0, before.run.generation.0 + 1);
    let n = before.log.len();
    assert_eq!(after.log.get(..n), Some(before.log.as_slice()));
    assert_eq!(after.log.len(), n + 1);
    assert!(matches!(after.log.last(), Some(Event::Reloaded { .. })));
}

#[test]
fn skipped_count_equals_skipped_steps_in_events_log_and_status() {
    let ir = ir5();
    let (state, events) = apply(&ir, SessionState::default(), begin("s-4"));
    assert_eq!(skipped_in(&events).len(), 3);
    assert_eq!(skipped_in(&state.log).len(), 3);
    assert_eq!(skipped_by_status(&ir, &state).len(), 3);
    assert_skips(&ir, &state, &[1, 2, 3]);
}

#[test]
fn begin_at_the_same_start_again_does_not_duplicate() {
    let ir = ir5();
    let (state, _) = apply(&ir, SessionState::default(), begin("s-4"));
    let (state, events) = apply(&ir, state, begin("s-4"));
    assert!(skipped_in(&events).is_empty(), "{events:?}");
    assert_skips(&ir, &state, &[1, 2, 3]);
}

#[test]
fn begin_at_another_start_skips_only_not_started_steps() {
    let ir = ir5();
    let (state, first) = apply(&ir, SessionState::default(), begin("s-2"));
    assert_eq!(skipped_in(&first), vec![uid(1)]);
    let (state, second) = apply(&ir, state, begin("s-4"));
    // st-2 は `Entered` なので省略にしない。新しく省略になるのは st-3 だけ。
    assert_eq!(skipped_in(&second), vec![uid(3)]);
    assert_skips(&ir, &state, &[1, 3]);
    assert_ne!(status_of(&ir, &state.run, &uid(2)), StepStatus::Skipped);
}

#[test]
fn progress_after_begin_keeps_skips_and_never_achieves_them() {
    let ir = ir5();
    let (mut state, _) = apply(&ir, SessionState::default(), begin("s-4"));
    for _ in 0..4 {
        state = apply(&ir, state, approve()).0;
        assert_skips(&ir, &state, &[1, 2, 3]);
    }
    let projection = Projection::from_events(&state.log);
    let items: Vec<_> = projection
        .review()
        .iter()
        .filter(|r| r.skipped && !r.achieved)
        .map(|r| r.step.clone())
        .collect();
    assert_eq!(items, vec![uid(1), uid(2), uid(3)]);
}

#[test]
fn skips_survive_export_import_replay_and_reload() {
    let ir = ir5();
    let mut s = recorded(&ir);
    s.apply(begin("s-4"));
    s.apply(approve());
    let before = s.session().state().clone();
    assert_skips(&ir, &before, &[1, 2, 3]);

    // 書き出し→読み込み。
    let (bytes, opened) = export_and_open(&ir, &s);
    assert_eq!(opened, before);
    assert_skips(&ir, &opened, &[1, 2, 3]);

    // 適用し直し。
    let replayed = replay(&ir, SessionState::default(), s.ops()).expect("replay");
    assert_eq!(replayed, before);
    assert_skips(&ir, &replayed, &[1, 2, 3]);

    // 再読み込み（起動の分岐）。
    let mut restored = boot(&ir, &bytes);
    let after = restored.session().state().clone();
    assert_only_reload_differs(&before, &after);
    assert_skips(&ir, &after, &[1, 2, 3]);

    // 再読み込みを重ねても件数も状態も変わらない。
    for _ in 0..3 {
        let (bytes, _) = export_and_open(&ir, &restored);
        restored = boot(&ir, &bytes);
        assert_skips(&ir, restored.session().state(), &[1, 2, 3]);
    }

    // 復元の後に進めても増えず、達成にもならない。包みは開けて件数は同じ。
    for _ in 0..3 {
        restored.apply(approve());
        assert_skips(&ir, restored.session().state(), &[1, 2, 3]);
    }
    let (_, reopened) = export_and_open(&ir, &restored);
    assert_eq!(&reopened, restored.session().state());
    assert_skips(&ir, &reopened, &[1, 2, 3]);
}

#[test]
fn begin_twice_survives_export_import() {
    let ir = ir5();
    let mut s = recorded(&ir);
    s.apply(begin("s-4"));
    s.apply(begin("s-4"));
    let (_, opened) = export_and_open(&ir, &s);
    assert_eq!(&opened, s.session().state());
    assert_skips(&ir, &opened, &[1, 2, 3]);
}
