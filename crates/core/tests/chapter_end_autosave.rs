//! 章の終わりのチェックポイントの自動保存の試験（TASK-339 1/2、SAVE-31）。
//!
//! 3 Step の教材（`min-3step.json`、`main/st-3` が最後の Step）で、章の終わりを含む操作の末尾に
//! `CheckpointSaved` が 1 件だけ足され、手動の保存と同じ通し番号を使うことを確かめる。
//! 上限（200 件）と上限の後の扱いは TASK-339 2/2 が持つため、ここでは扱わない。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::chapter_end::{chapter_end_in, is_main_final};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// 指定の Step が受付済み（処理中でない）にある状態。
fn accepted_at(step: &str) -> SessionState {
    let u = StepUid(step.to_string());
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

fn saved(events: &[Event]) -> Vec<(CheckpointId, u32)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::CheckpointSaved { id, log_len, .. } => Some((*id, *log_len)),
            _ => None,
        })
        .collect()
}

#[test]
fn the_final_step_is_the_chapter_end() {
    let ir = ir();
    assert!(is_main_final(&ir, "main/st-3"));
    assert!(!is_main_final(&ir, "main/st-2"));
    assert!(!is_main_final(&ir, "main/unknown"));
    assert!(!chapter_end_in(&ir, &[]));
}

#[test]
fn a_non_final_achievement_does_not_save() {
    let ir = ir();
    let mut s = accepted_at("main/st-1");
    s.run.busy = None;
    let (s, events) = apply(
        &ir,
        s,
        Op::Input(Input::Ui {
            view: "v-files".into(),
            path: "notes/draft".into(),
        }),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    assert!(saved(&events).is_empty());
    assert!(s.checkpoints.is_empty());
    assert_eq!(s.next_checkpoint, CheckpointId(1));
}

#[test]
fn the_chapter_end_appends_one_checkpoint_at_the_tail() {
    let ir = ir();
    let s = accepted_at("main/st-3");
    let len_before = s.log.len();
    let gen_before = s.run.generation;
    let (s, events) = apply(&ir, s, approve());
    assert!(matches!(events.last(), Some(Event::CheckpointSaved { .. })));
    let marks = saved(&events);
    assert_eq!(marks.len(), 1);
    let (id, log_len) = marks[0];
    assert_eq!(id, CheckpointId(1));
    assert_eq!(s.next_checkpoint, CheckpointId(2));
    // `log_len` は `CheckpointSaved` 自身のログ上の位置。
    let index = usize::try_from(log_len).expect("fits");
    assert_eq!(index, len_before + events.len() - 1);
    assert!(matches!(
        s.log.get(index),
        Some(Event::CheckpointSaved { .. })
    ));
    // 写しは操作の後の状態で、世代は変わらない。
    let cp = &s.checkpoints[&id];
    assert_eq!(cp.run, s.run);
    assert_eq!(s.run.generation, gen_before);
    // 返したイベント列は、新しいログの末尾と一致する。
    assert_eq!(s.log.get(len_before..), Some(events.as_slice()));
}

#[test]
fn the_autosave_shares_the_id_sequence_with_manual_saves() {
    let ir = ir();
    let (s, events) = apply(&ir, accepted_at("main/st-3"), Op::Control(Control::Save));
    assert_eq!(saved(&events).len(), 1);
    assert_eq!(saved(&events)[0].0, CheckpointId(1));
    // 手動の保存は章の終わりではないので、自動保存は足されない。
    assert_eq!(s.checkpoints.len(), 1);
    let (s, events) = apply(&ir, s, approve());
    assert_eq!(saved(&events)[0].0, CheckpointId(2));
    assert_eq!(s.checkpoints.len(), 2);
    assert_eq!(s.next_checkpoint, CheckpointId(3));
}

#[test]
fn an_id_overflow_warns_and_keeps_the_achievement() {
    let ir = ir();
    let mut s = accepted_at("main/st-3");
    s.next_checkpoint = CheckpointId(u32::MAX);
    let (s, events) = apply(&ir, s, approve());
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    assert!(saved(&events).is_empty());
    assert!(matches!(events.last(), Some(Event::Warning { .. })));
    assert!(s.checkpoints.is_empty());
    assert_eq!(s.next_checkpoint, CheckpointId(u32::MAX));
}

#[test]
fn reload_and_restore_do_not_autosave() {
    let ir = ir();
    let (s, _) = apply(&ir, accepted_at("main/st-3"), approve());
    assert_eq!(s.checkpoints.len(), 1);
    let (s, events) = apply(&ir, s, Op::Control(Control::Restore(CheckpointId(1))));
    assert!(saved(&events).is_empty());
    assert_eq!(s.checkpoints.len(), 1);
    let (s, events) = fandhe_workshop_core::session::reload(s);
    assert!(saved(&events).is_empty());
    assert_eq!(s.checkpoints.len(), 1);
}
