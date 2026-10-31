//! チェックポイントの保存の数の上限（200 件）と上限の後の扱いの試験（TASK-339 2/2、SAVE-31）。
//!
//! 3 Step の教材（`min-3step.json`、`main/st-3` が最後の Step）で、次を確かめる。
//! - 200 件目までは保存でき、201 件目の任意の保存は `Refused(CheckpointLimit)` で断る（状態を変えない）
//! - 上限の後の章の終わりは自動保存をせず、末尾に同じ `Refused` を 1 件足す
//! - 任意の保存と自動保存は同じ列・同じ上限を使い、`ops` の適用し直しで同じ状態になる

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::MAX_CHECKPOINTS;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseNextAction, RefuseReason};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{Checkpoint, RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// `main/st-3`（最後の Step）が受付済みの状態。
fn accepted_at_final() -> SessionState {
    let u = StepUid("main/st-3".to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Accepted);
    let mut run = RunState {
        generation: Generation(2),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn approve() -> Op {
    Op::Input(Input::Approval {
        decision: Decision::Approve,
        target: None,
    })
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn saved_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::CheckpointSaved { .. }))
        .count()
}

fn limit_refusals(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Refused {
                    op: ControlKind::Save,
                    reason: RefuseReason::CheckpointLimit,
                    rejection: None,
                    ..
                }
            )
        })
        .count()
}

/// 手動の保存を `n` 件になるまで送る。
fn fill_by_saves(ir: &Ir, mut s: SessionState, n: usize) -> SessionState {
    while s.checkpoints.len() < n {
        let (next, events) = apply(ir, s, save());
        assert_eq!(saved_count(&events), 1);
        s = next;
    }
    s
}

/// ログを除いた比較用の写し。
fn without_log(s: &SessionState) -> SessionState {
    let mut c = s.clone();
    c.log.clear();
    c
}

#[test]
fn the_limit_is_200() {
    assert_eq!(MAX_CHECKPOINTS, 200);
}

#[test]
fn the_200th_save_succeeds() {
    let ir = ir();
    let s = fill_by_saves(&ir, accepted_at_final(), MAX_CHECKPOINTS - 1);
    assert_eq!(s.checkpoints.len(), 199);
    let (s, events) = apply(&ir, s, save());
    assert_eq!(saved_count(&events), 1);
    assert_eq!(limit_refusals(&events), 0);
    assert_eq!(s.checkpoints.len(), MAX_CHECKPOINTS);
}

#[test]
fn the_201st_manual_save_is_refused_without_change() {
    let ir = ir();
    let s = fill_by_saves(&ir, accepted_at_final(), MAX_CHECKPOINTS);
    let before = s.clone();
    let (after, events) = apply(&ir, s, save());
    assert_eq!(events.len(), 1, "{events:?}");
    assert_eq!(limit_refusals(&events), 1);
    assert_eq!(saved_count(&events), 0);
    if let Some(Event::Refused { reason, .. }) = events.first() {
        assert_eq!(
            reason.next_action(),
            Some(RefuseNextAction::ContinueWithExistingCheckpoints)
        );
    }
    // ログに拒否の 1 件が足される以外は、状態が操作の前と等しい。
    assert_eq!(after.log.len(), before.log.len() + 1);
    assert_eq!(without_log(&after), without_log(&before));
    assert_eq!(
        after.log.get(..before.log.len()),
        Some(before.log.as_slice())
    );
    // 断り続けても件数は増えない。
    let (again, events) = apply(&ir, after, save());
    assert_eq!(limit_refusals(&events), 1);
    assert_eq!(again.checkpoints.len(), MAX_CHECKPOINTS);
}

#[test]
fn the_autosave_can_be_the_200th() {
    let ir = ir();
    let s = fill_by_saves(&ir, accepted_at_final(), MAX_CHECKPOINTS - 1);
    let (s, events) = apply(&ir, s, approve());
    assert_eq!(saved_count(&events), 1);
    assert_eq!(limit_refusals(&events), 0);
    assert!(matches!(events.last(), Some(Event::CheckpointSaved { .. })));
    assert_eq!(s.checkpoints.len(), MAX_CHECKPOINTS);
    assert_eq!(s.next_checkpoint, CheckpointId(201));
}

#[test]
fn the_autosave_after_the_limit_is_skipped_with_one_line() {
    let ir = ir();
    let s = fill_by_saves(&ir, accepted_at_final(), MAX_CHECKPOINTS);
    let ids_before: Vec<_> = s.checkpoints.keys().copied().collect();
    let next_before = s.next_checkpoint;
    let (s, events) = apply(&ir, s, approve());
    // 達成などは通常どおり起きる。
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    assert_eq!(saved_count(&events), 0);
    assert_eq!(limit_refusals(&events), 1);
    assert!(
        matches!(
            events.last(),
            Some(Event::Refused {
                reason: RefuseReason::CheckpointLimit,
                ..
            })
        ),
        "{events:?}"
    );
    let ids_after: Vec<_> = s.checkpoints.keys().copied().collect();
    assert_eq!(ids_after, ids_before);
    assert_eq!(s.next_checkpoint, next_before);
}

#[test]
fn over_the_limit_is_also_refused() {
    let ir = ir();
    let mut s = fill_by_saves(&ir, accepted_at_final(), MAX_CHECKPOINTS);
    // 手で組んだ 200 件を超える状態（履歴由来の不整合を想定）。
    let extra: Checkpoint = s.checkpoints.values().next().expect("one").clone();
    s.checkpoints.insert(CheckpointId(9_999), extra);
    assert_eq!(s.checkpoints.len(), MAX_CHECKPOINTS + 1);
    let (after, events) = apply(&ir, s.clone(), save());
    assert_eq!(limit_refusals(&events), 1);
    assert_eq!(after.checkpoints.len(), MAX_CHECKPOINTS + 1);
    assert_eq!(after.next_checkpoint, s.next_checkpoint);
}

#[test]
fn id_overflow_still_warns() {
    let ir = ir();
    let mut s = accepted_at_final();
    s.next_checkpoint = CheckpointId(u32::MAX);
    let (s, events) = apply(&ir, s, save());
    assert_eq!(limit_refusals(&events), 0);
    assert!(matches!(events.last(), Some(Event::Warning { .. })));
    assert!(s.checkpoints.is_empty());
}

#[test]
fn manual_and_auto_saves_share_the_limit_and_replay() {
    let ir = ir();
    let initial = accepted_at_final();
    let mut rec = RecordedSession::new(Session::from_state(ir.clone(), initial.clone()), "s-1");
    // 1 周 = 手動の保存 1 件 + 章の終わりの自動保存 1 件 + 復帰（受付済みへ戻る）。
    let mut cycles = 0;
    while rec.session().state().checkpoints.len() < MAX_CHECKPOINTS {
        let first = rec.session().state().next_checkpoint;
        rec.apply(save());
        rec.apply(approve());
        rec.apply(Op::Control(Control::Restore(first)));
        cycles += 1;
        assert!(cycles <= MAX_CHECKPOINTS, "no progress");
    }
    let s = rec.session().state();
    assert_eq!(s.checkpoints.len(), MAX_CHECKPOINTS);
    // 手動と自動の両方が同じ列に入っている（1 周で 2 件ずつ増える）。
    assert_eq!(cycles * 2, MAX_CHECKPOINTS);
    // 201 件目は手動でも自動でも増えない。
    let before_len = rec.session().state().log.len();
    let events = rec.apply(save()).to_vec();
    assert_eq!(limit_refusals(&events), 1);
    let events = rec.apply(approve()).to_vec();
    assert_eq!(saved_count(&events), 0);
    assert!(rec.session().state().log.len() > before_len);
    assert_eq!(rec.session().state().checkpoints.len(), MAX_CHECKPOINTS);
    // `ops` の適用し直しで同じ状態になる（自動保存・拒否も再現される）。
    let replayed = replay(&ir, initial, rec.ops()).expect("replay");
    assert_eq!(&replayed, rec.session().state());
}
