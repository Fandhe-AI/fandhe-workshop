//! チェックポイントの保存と復帰の試験（TASK-214 1/3、仕様 EVT-6 (5)(6)・EVT-12・EVT-14・性質テスト P-4）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1 を受け付けて進行を進め、途中で保存し、先へ進んだ後に復帰したとき、
//! 実行状態が保存時点に戻り、世代だけが復帰前の +1 になることを確かめる。保存は世代を変えない。
//! 復帰に失敗したとき（無い番号・世代や番号のあふれ・不整合な写し）は世代も状態も変わらない。
//! `Restored` による復習の作り直し（2/3）の試験は `restore_review.rs`、復帰前の進行の無視・処理中の再開（3/3）の試験は別件。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::property::{assert_replay_eq, check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::{Cause, ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::session::{Busy, SessionState};
use fandhe_workshop_core::{apply, load_ir};
use proptest::prelude::*;

const ST1: &str = "main/st-1";

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn restore(n: u32) -> Op {
    Op::Control(Control::Restore(CheckpointId(n)))
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    let state = apply(ir, load().initial, first_note()).0;
    tick_n(ir, state, ticks)
}

fn tick_n(ir: &Ir, mut state: SessionState, n: u32) -> SessionState {
    for _ in 0..n {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

fn only_warning(events: &[Event]) -> bool {
    matches!(events, [Event::Warning { .. }])
}

fn saved_log_len(events: &[Event]) -> Option<u32> {
    events.iter().find_map(|e| match e {
        Event::CheckpointSaved { log_len, .. } => Some(*log_len),
        _ => None,
    })
}

#[test]
fn restore_after_saving_at_three_ticks_returns_saved_run() {
    let ir = ir();
    let state = busy_state(&ir, 3);
    assert_eq!(state.run.busy, Some(Busy { done: 3, total: 5 }));
    let (saved, save_events) = apply(&ir, state, save());
    let saved_run = saved.run.clone();
    let advanced = tick_n(&ir, saved, 1);
    assert_eq!(advanced.run.busy, Some(Busy { done: 4, total: 5 }));
    let gen_before = advanced.run.generation;
    let (next, events) = apply(&ir, advanced, restore(1));
    let mut expected = saved_run;
    expected.generation = Generation(gen_before.0 + 1);
    assert_eq!(next.run, expected);
    assert_eq!(next.run.busy, Some(Busy { done: 3, total: 5 }));
    let restored: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Restored {
                at,
                id,
                new_generation,
                log_len,
                owner,
            } => Some((at.clone(), *id, *new_generation, *log_len, owner.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(restored.len(), 1);
    let (at, id, new_generation, log_len, owner) = &restored[0];
    assert_eq!(at.generation, gen_before, "at is the pre-restore stamp");
    assert_eq!(at.step, uid(ST1));
    assert_eq!(*id, CheckpointId(1));
    assert_eq!(*new_generation, next.run.generation);
    assert_eq!(Some(*log_len), saved_log_len(&save_events));
    assert_eq!(*owner, uid(ST1));
    for e in &events {
        if let Event::Changed { cause, .. } = e {
            assert_eq!(*cause, Cause::Restore);
        }
    }
}

#[test]
fn save_keeps_run_and_generation() {
    let ir = ir();
    let state = busy_state(&ir, 2);
    let before = state.clone();
    let (next, events) = apply(&ir, state, save());
    assert_eq!(next.run, before.run);
    assert_eq!(next.run.generation, before.run.generation);
    assert_eq!(next.checkpoints.len(), 1);
    assert_eq!(next.next_checkpoint, CheckpointId(2));
    let cp = next.checkpoints.get(&CheckpointId(1)).expect("checkpoint");
    assert_eq!(cp.run, before.run);
    assert_eq!(cp.entry, before.entry);
    assert_eq!(cp.owner, Some(uid(ST1).root()));
    assert_eq!(
        saved_log_len(&events),
        u32::try_from(before.log.len()).ok(),
        "log_len is the log length before the save"
    );
    assert_eq!(cp.log_len, u32::try_from(before.log.len()).expect("fits"));
    assert!(matches!(
        events.as_slice(),
        [Event::CheckpointSaved {
            id: CheckpointId(1),
            ..
        }]
    ));
    assert_eq!(next.log.len(), before.log.len() + 1);
}

#[test]
fn second_save_does_not_overwrite_first() {
    let ir = ir();
    let state = busy_state(&ir, 1);
    let once = apply(&ir, state, save()).0;
    let first = once.checkpoints.get(&CheckpointId(1)).cloned();
    let advanced = tick_n(&ir, once, 1);
    let twice = apply(&ir, advanced, save()).0;
    assert_eq!(twice.checkpoints.len(), 2);
    assert_eq!(twice.checkpoints.get(&CheckpointId(1)).cloned(), first);
    assert_eq!(twice.next_checkpoint, CheckpointId(3));
}

#[test]
fn save_and_restore_without_current_are_noop() {
    let ir = ir();
    for op in [save(), restore(1)] {
        let state = SessionState::default();
        let (next, events) = apply(&ir, state.clone(), op);
        assert!(events.is_empty());
        assert_eq!(next, state);
    }
}

#[test]
fn restore_unknown_checkpoint_is_refused_without_change() {
    let ir = ir();
    // 別の番号のチェックポイントがあっても、無い番号へは戻らない。
    let state = apply(&ir, busy_state(&ir, 2), save()).0;
    let (next, events) = apply(&ir, state.clone(), restore(7));
    assert_eq!(next.run, state.run);
    assert_eq!(next.entry, state.entry);
    assert_eq!(next.checkpoints, state.checkpoints);
    assert!(matches!(
        events.as_slice(),
        [Event::Refused {
            op: ControlKind::Restore,
            reason: RefuseReason::UnknownCheckpoint,
            rejection: None,
            ..
        }]
    ));
}

#[test]
fn restore_overflow_warns_only() {
    let ir = ir();
    let mut state = apply(&ir, busy_state(&ir, 1), save()).0;
    state.run.generation = Generation(u64::MAX);
    let (next, events) = apply(&ir, state.clone(), restore(1));
    assert!(only_warning(&events));
    assert_eq!(next.run, state.run);
    assert_eq!(next.entry, state.entry);
}

#[test]
fn save_id_overflow_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 1);
    state.next_checkpoint = CheckpointId(u32::MAX);
    let (next, events) = apply(&ir, state.clone(), save());
    assert!(only_warning(&events));
    assert_eq!(next.checkpoints, state.checkpoints);
    assert_eq!(next.next_checkpoint, state.next_checkpoint);
    assert_eq!(next.run, state.run);
}

#[test]
fn restore_checkpoint_without_current_warns_only() {
    let ir = ir();
    let mut state = apply(&ir, busy_state(&ir, 1), save()).0;
    if let Some(cp) = state.checkpoints.get_mut(&CheckpointId(1)) {
        cp.run.current = None;
    }
    let (next, events) = apply(&ir, state.clone(), restore(1));
    assert!(only_warning(&events));
    assert_eq!(next.run, state.run);
    assert_eq!(next.entry, state.entry);
}

#[test]
fn restore_keeps_log_drafts_hints_and_checkpoints() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.drafts.insert(uid(ST1).root(), "draft text".into());
    state.hints_opened.insert(uid(ST1), 2);
    let saved = apply(&ir, state, save()).0;
    let advanced = tick_n(&ir, saved, 1);
    let before = advanced.clone();
    let (next, events) = apply(&ir, advanced, restore(1));
    assert_eq!(next.drafts, before.drafts);
    assert_eq!(next.hints_opened, before.hints_opened);
    assert_eq!(next.checkpoints, before.checkpoints);
    assert_eq!(next.next_checkpoint, before.next_checkpoint);
    assert_eq!(
        next.log.get(..before.log.len()),
        Some(before.log.as_slice())
    );
    assert_eq!(
        next.log.get(before.log.len()..),
        Some(events.as_slice()),
        "log tail"
    );
    // 同じ ID へ何度でも戻れ、世代はそのたびに +1 になる。
    let again = apply(&ir, next.clone(), restore(1)).0;
    assert_eq!(
        again.run.generation.0,
        next.run.generation.0 + 1,
        "second restore"
    );
}

#[test]
fn restore_after_stop_brings_back_world() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 2), save()).0;
    assert!(saved.run.world.contains_key("notes/draft"));
    let stopped = apply(&ir, saved.clone(), Op::Control(Control::Stop)).0;
    assert!(!stopped.run.world.contains_key("notes/draft"));
    let (next, _) = apply(&ir, stopped, restore(1));
    assert_eq!(next.run.world, saved.run.world);
    assert_eq!(next.run.busy, saved.run.busy);
}

#[test]
fn restore_after_go_to_brings_back_entry() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 1), save()).0;
    let entry = saved.entry.clone();
    assert!(entry.is_some());
    let moved = apply(&ir, saved, Op::Control(Control::GoTo(uid("main/st-2")))).0;
    assert_eq!(moved.entry, None);
    let (next, _) = apply(&ir, moved, restore(1));
    assert_eq!(next.entry, entry, "entry copy is restored unchanged");
    assert_eq!(next.run.current, Some(uid(ST1)));
}

#[derive(Debug, Clone)]
enum After {
    Next,
    Resend,
    Stop,
    GoTo,
}

fn after_strategy() -> impl Strategy<Value = After> {
    prop_oneof![
        Just(After::Next),
        Just(After::Resend),
        Just(After::Stop),
        Just(After::GoTo),
    ]
}

fn scenario() -> impl Strategy<Value = (u32, Vec<After>)> {
    (0u32..5, op_sequence(after_strategy()))
}

/// 進行を進めて保存し、保存の後に操作列を流した状態。(保存した状態, 流した後の状態)。
fn run_scenario(ticks: u32, ops: &[After]) -> (SessionState, SessionState) {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, ticks), save()).0;
    let mut state = saved.clone();
    for op in ops {
        state = match op {
            After::Next => match next_tick(&state) {
                Some(addr) => apply(&ir, state, Op::Tick(addr)).0,
                None => state,
            },
            After::Resend => apply(&ir, state, first_note()).0,
            After::Stop => apply(&ir, state, Op::Control(Control::Stop)).0,
            After::GoTo => apply(&ir, state, Op::Control(Control::GoTo(uid("main/st-2")))).0,
        };
    }
    (saved, state)
}

#[test]
fn p4_restore_returns_saved_run() {
    check(
        "p4-restore-returns-saved-run",
        DEFAULT_CASES,
        scenario(),
        |(ticks, ops)| {
            let ir = ir();
            let (saved, state) = run_scenario(ticks, &ops);
            let before = state.run.generation;
            let (next, events) = apply(&ir, state, restore(1));
            let mut expected = saved.run.clone();
            expected.generation = next.run.generation;
            prop_assert_eq!(&next.run, &expected);
            prop_assert_eq!(Some(next.run.generation), before.next());
            let restored: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Restored { new_generation, .. } => Some(*new_generation),
                    _ => None,
                })
                .collect();
            prop_assert_eq!(restored, vec![next.run.generation]);
            for e in &events {
                if let Event::Changed { cause, .. } = e {
                    prop_assert_eq!(*cause, Cause::Restore);
                }
            }
            Ok(())
        },
    );
}

#[test]
fn p4_failed_restore_keeps_generation() {
    check(
        "p4-failed-restore-keeps-generation",
        DEFAULT_CASES,
        scenario(),
        |(ticks, ops)| {
            let ir = ir();
            let (_, state) = run_scenario(ticks, &ops);
            let (next, events) = apply(&ir, state.clone(), restore(99));
            prop_assert_eq!(&next.run, &state.run);
            prop_assert_eq!(&next.entry, &state.entry);
            prop_assert_eq!(events.len(), 1);
            Ok(())
        },
    );
}

#[test]
fn restore_is_deterministic() {
    check(
        "restore-is-deterministic",
        DEFAULT_CASES,
        scenario(),
        |(ticks, ops)| {
            assert_replay_eq(
                |p: &[(u32, Vec<After>)]| {
                    let (_, state) = run_scenario(p[0].0, &p[0].1);
                    apply(&ir(), state, restore(1))
                },
                &[(ticks, ops)],
            )
        },
    );
}
