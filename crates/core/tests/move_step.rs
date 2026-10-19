//! 別の Step への移動 `Control::GoTo` の試験（TASK-213 1/2、仕様 EVT-11・性質テスト P-4）。
//!
//! 3 Step の教材（`min-3step.json`）で、成功した移動が世代を +1 し、処理中を解き、移動先を `Entered` にし、
//! 入口の写しを無効化することを確かめる。無い Step・世代のあふれ・現在の Step 無しは状態を変えない
//! （fail-closed）。移動前の世代を宛先にした進行が移動の後に届いても、`run` は変わらず `Ignored(Stale)` が
//! 1 件だけ増えること、同じ Step へ入り直して受け付け直した後も効かないこと（TASK-213 2/2）も確かめる。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::property::{assert_replay_eq, check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{ControlKind, Event, IgnoreReason, RefuseReason};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::{Busy, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use proptest::prelude::*;

const ST1: &str = "main/st-1";
const ST2: &str = "main/st-2";

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn go_to(s: &str) -> Op {
    Op::Control(Control::GoTo(uid(s)))
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn stamp(g: u64, s: &str) -> Stamp {
    Stamp {
        generation: Generation(g),
        step: uid(s),
    }
}

fn status_of(state: &SessionState, s: &str) -> Option<StepStatus> {
    let u = uid(s);
    state
        .run
        .slices
        .get(&u.root())
        .and_then(|x| x.status.get(&u))
        .copied()
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    let mut state = apply(ir, load().initial, first_note()).0;
    for _ in 0..ticks {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

fn only_warning(events: &[Event]) -> bool {
    matches!(events, [Event::Warning { .. }])
}

#[test]
fn move_while_busy_increments_generation_and_clears_busy() {
    let ir = ir();
    let state = busy_state(&ir, 2);
    assert!(state.run.busy.is_some());
    let world = state.run.world.clone();
    let (next, events) = apply(&ir, state, go_to(ST2));
    assert_eq!(next.run.generation, Generation(2));
    assert_eq!(next.run.busy, None);
    assert_eq!(next.run.current, Some(uid(ST2)));
    assert_eq!(status_of(&next, ST2), Some(StepStatus::Entered));
    assert_eq!(status_of(&next, ST1), Some(StepStatus::Accepted));
    assert_eq!(next.run.world, world);
    assert!(next.run.world.contains_key("notes/draft"));
    assert_eq!(
        events,
        vec![
            Event::Moved {
                at: stamp(1, ST1),
                to: uid(ST2),
                new_generation: Generation(2)
            },
            Event::Entered { at: stamp(2, ST2) },
        ]
    );
    // 関門が黙って直さないよう、世代を運ぶイベントと状態の一致を明示して比べる。
    assert!(matches!(
        events.first(),
        Some(Event::Moved { new_generation, .. }) if *new_generation == next.run.generation
    ));
}

#[test]
fn idle_move_also_increments() {
    let ir = ir();
    let (next, events) = apply(&ir, load().initial, go_to(ST2));
    assert_eq!(next.run.generation, Generation(2));
    assert_eq!(next.run.current, Some(uid(ST2)));
    assert_eq!(events.len(), 2, "{events:?}");
}

#[test]
fn move_to_same_step_reenters() {
    let ir = ir();
    let state = busy_state(&ir, 1);
    let (next, events) = apply(&ir, state, go_to(ST1));
    assert_eq!(next.run.generation, Generation(2));
    assert_eq!(next.run.busy, None);
    assert_eq!(next.run.current, Some(uid(ST1)));
    assert_eq!(status_of(&next, ST1), Some(StepStatus::Entered));
    assert!(matches!(events.get(1), Some(Event::Entered { .. })));
}

#[test]
fn move_to_achieved_step_sets_entered() {
    let ir = ir();
    let mut state = load().initial;
    state
        .run
        .slices
        .entry(uid(ST2).root())
        .or_default()
        .status
        .insert(uid(ST2), StepStatus::Achieved);
    let (next, _) = apply(&ir, state, go_to(ST2));
    assert_eq!(status_of(&next, ST2), Some(StepStatus::Entered));
}

#[test]
fn move_to_unknown_step_is_refused() {
    let ir = ir();
    for state in [busy_state(&ir, 2), load().initial] {
        let before = state.clone();
        let (next, events) = apply(&ir, state, go_to("main/st-9"));
        assert_eq!(
            events,
            vec![Event::Refused {
                at: stamp(before.run.generation.0, ST1),
                op: ControlKind::GoTo,
                reason: RefuseReason::UnknownStep,
                rejection: None,
            }]
        );
        assert_eq!(next.run, before.run);
        assert_eq!(next.entry, before.entry);
    }
}

#[test]
fn move_overflow_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.run.generation = Generation(u64::MAX);
    let before = state.clone();
    let (next, events) = apply(&ir, state, go_to(ST2));
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(next.run, before.run);
    assert_eq!(next.entry, before.entry);
    if let Some(Event::Warning { message, .. }) = events.first() {
        assert!(!message.contains("st-2"));
    }
}

#[test]
fn move_without_current_does_nothing() {
    let ir = ir();
    let state = SessionState::default();
    let (next, events) = apply(&ir, state.clone(), go_to(ST2));
    assert!(events.is_empty());
    assert_eq!(next, state);
}

#[test]
fn move_keeps_drafts_hints_checkpoints_and_log_prefix() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.drafts.insert(uid(ST1).root(), "draft text".into());
    state.hints_opened.insert(uid(ST1), 2);
    let before = state.clone();
    let (next, events) = apply(&ir, state, go_to(ST2));
    assert_eq!(next.drafts, before.drafts);
    assert_eq!(next.hints_opened, before.hints_opened);
    assert_eq!(next.checkpoints, before.checkpoints);
    assert_eq!(
        next.log.get(..before.log.len()),
        Some(before.log.as_slice())
    );
    assert_eq!(
        next.log.get(before.log.len()..),
        Some(events.as_slice()),
        "log tail"
    );
}

#[test]
fn move_invalidates_entry_copy() {
    let ir = ir();
    let state = busy_state(&ir, 2);
    assert!(state.entry.is_some());
    let (next, _) = apply(&ir, state, go_to(ST1));
    assert!(next.entry.is_none());
    // 写しの Step へ戻った後に受け付けて停止しても、古い写しへ誤って巻き戻らない。受付の前に確保した新しい写し
    // （移動先の Step のもの）へ戻り、現在の Step を保ったまま処理中だけが解ける。
    let (accepted, _) = apply(&ir, next, first_note());
    assert!(accepted.run.busy.is_some());
    let (stopped, events) = apply(&ir, accepted, Op::Control(Control::Stop));
    assert!(
        events.iter().any(|e| matches!(e, Event::Stopped { .. })),
        "{events:?}"
    );
    assert!(stopped.run.busy.is_none());
    assert_eq!(stopped.run.current, Some(StepUid(ST1.to_string())));
}

#[derive(Debug, Clone)]
struct Plan {
    ticks: u32,
    resends: u32,
    target: usize,
}

fn plan_strategy() -> impl Strategy<Value = Plan> {
    (0u32..5, 0u32..3, 0usize..3).prop_map(|(ticks, resends, target)| Plan {
        ticks,
        resends,
        target,
    })
}

fn target_uid(plan: &Plan) -> &'static str {
    ["main/st-1", "main/st-2", "main/st-3"]
        .get(plan.target)
        .copied()
        .unwrap_or(ST1)
}

/// 受付・進行・受付の再送（拒否）の後に移動した結果。(移動前, 移動後, 移動のイベント列)。
fn run_plan(plan: &Plan) -> (SessionState, SessionState, Vec<Event>) {
    let ir = ir();
    let mut state = busy_state(&ir, plan.ticks);
    for _ in 0..plan.resends {
        state = apply(&ir, state, first_note()).0;
    }
    let before = state.clone();
    let (next, events) = apply(&ir, state, go_to(target_uid(plan)));
    (before, next, events)
}

#[test]
fn p4_move_clears_busy_and_increments() {
    check(
        "p4-move-clears-busy-and-increments",
        DEFAULT_CASES,
        plan_strategy(),
        |plan| {
            let (before, next, events) = run_plan(&plan);
            let to = uid(target_uid(&plan));
            prop_assert_eq!(Some(next.run.generation), before.run.generation.next());
            prop_assert_eq!(next.run.busy.clone(), None);
            prop_assert_eq!(next.run.current.clone(), Some(to.clone()));
            prop_assert_eq!(&next.run.world, &before.run.world);
            // 切り身は移動先の状態以外が等しい。
            let mut expected = before.run.clone();
            expected.generation = next.run.generation;
            expected.busy = None;
            expected.current = Some(to.clone());
            expected
                .slices
                .entry(to.root())
                .or_default()
                .status
                .insert(to.clone(), StepStatus::Entered);
            prop_assert_eq!(&next.run, &expected);
            let moved: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Moved { new_generation, .. } => Some(*new_generation),
                    _ => None,
                })
                .collect();
            prop_assert_eq!(moved, vec![next.run.generation]);
            let changed = events
                .iter()
                .filter(|e| matches!(e, Event::Changed { .. }))
                .count();
            prop_assert_eq!(changed, 0);
            Ok(())
        },
    );
}

#[test]
fn move_is_deterministic() {
    check(
        "move-is-deterministic",
        DEFAULT_CASES,
        plan_strategy(),
        |plan| {
            assert_replay_eq(
                |p: &[Plan]| {
                    let (_, next, events) = run_plan(&p[0]);
                    (next, events)
                },
                std::slice::from_ref(&plan),
            )
        },
    );
}

/// 移動前の世代（1）を宛先にした進行。
fn old_tick(step: &str, index: u32) -> Op {
    Op::Tick(TickAddr {
        generation: Generation(1),
        step: uid(step),
        index,
    })
}

fn progress_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

/// 処理中の状態に残っている予約（`done..total` の番号、現在の世代・現在の Step）。
fn reserved_ticks(state: &SessionState) -> Vec<TickAddr> {
    let (Some(step), Some(busy)) = (state.run.current.clone(), state.run.busy.as_ref()) else {
        return Vec::new();
    };
    (busy.done..busy.total)
        .map(|index| TickAddr {
            generation: state.run.generation,
            step: step.clone(),
            index,
        })
        .collect()
}

#[test]
fn reserved_ticks_after_move_are_ignored() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let reserved = reserved_ticks(&busy);
    assert_eq!(reserved.len(), 3);
    let mut state = apply(&ir, busy, go_to(ST2)).0;
    assert_eq!(state.run.generation, Generation(2));
    for addr in reserved {
        let before = state.clone();
        let (next, events) = apply(&ir, state, Op::Tick(addr));
        assert_eq!(next.run, before.run);
        assert_eq!(
            events,
            vec![Event::Ignored {
                at: stamp(2, ST2),
                reason: IgnoreReason::Stale
            }]
        );
        assert_eq!(next.log.len(), before.log.len() + 1);
        state = next;
    }
}

#[test]
fn reserved_ticks_after_reenter_are_ignored() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let reserved = reserved_ticks(&busy);
    let moved = apply(&ir, busy, go_to(ST1)).0;
    assert_eq!(moved.run.busy, None);
    for addr in reserved {
        let (next, events) = apply(&ir, moved.clone(), Op::Tick(addr));
        assert_eq!(next.run, moved.run);
        assert_eq!(
            events,
            vec![Event::Ignored {
                at: stamp(2, ST1),
                reason: IgnoreReason::Stale
            }]
        );
    }
}

#[test]
fn stale_tick_after_reenter_and_reinput_is_ignored() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let moved = apply(&ir, busy, go_to(ST1)).0;
    let mut state = apply(&ir, moved, first_note()).0;
    assert_eq!(state.run.generation, Generation(2));
    assert_eq!(state.run.busy, Some(Busy { done: 0, total: 5 }));
    let before = state.run.clone();
    let (next, events) = apply(&ir, state.clone(), old_tick(ST1, 0));
    assert_eq!(next.run, before);
    assert_eq!(progress_count(&events), 0);
    assert_eq!(
        events,
        vec![Event::Ignored {
            at: stamp(2, ST1),
            reason: IgnoreReason::Stale
        }]
    );
    let addr = next_tick(&state).expect("next tick");
    state = apply(&ir, state, Op::Tick(addr)).0;
    assert_eq!(state.run.busy, Some(Busy { done: 1, total: 5 }));
    for n in 1..5 {
        if n == 3 {
            let before = state.run.clone();
            state = apply(&ir, state, old_tick(ST1, 3)).0;
            assert_eq!(state.run, before);
        }
        let addr = next_tick(&state).expect("next tick");
        state = apply(&ir, state, Op::Tick(addr)).0;
    }
    assert_eq!(state.run.busy, None);
}

#[test]
fn refused_move_keeps_reservation() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let addr = next_tick(&busy).expect("next tick");
    let (refused, _) = apply(&ir, busy, go_to("main/st-9"));
    assert_eq!(refused.run.generation, Generation(1));
    let (done, events) = apply(&ir, refused, Op::Tick(addr));
    assert_eq!(progress_count(&events), 1);
    assert_eq!(done.run.busy, Some(Busy { done: 3, total: 5 }));
}

#[derive(Debug, Clone)]
enum PostOp {
    Old { step: usize, index: u32 },
    Reinput,
    Next,
}

fn post_op_strategy() -> impl Strategy<Value = PostOp> {
    prop_oneof![
        (0usize..3, 0u32..7).prop_map(|(step, index)| PostOp::Old { step, index }),
        Just(PostOp::Reinput),
        Just(PostOp::Next),
    ]
}

fn post_move_scenario() -> impl Strategy<Value = (Plan, Vec<PostOp>)> {
    (plan_strategy(), op_sequence(post_op_strategy()))
}

/// 移動の後に操作列を流す。古い世代の宛先の進行ごとに (操作前, 操作後, イベント列) を返す。
fn run_post_move(
    plan: &Plan,
    ops: &[PostOp],
) -> (SessionState, Vec<(SessionState, SessionState, Vec<Event>)>) {
    let ir = ir();
    let mut state = run_plan(plan).1;
    let mut stale = Vec::new();
    for op in ops {
        match op {
            PostOp::Old { step, index } => {
                let name = ["main/st-1", "main/st-2", "main/st-3"]
                    .get(*step)
                    .copied()
                    .unwrap_or(ST1);
                let (next, events) = apply(&ir, state.clone(), old_tick(name, *index));
                stale.push((state.clone(), next.clone(), events));
                state = next;
            }
            PostOp::Reinput => state = apply(&ir, state, first_note()).0,
            PostOp::Next => {
                if let Some(addr) = next_tick(&state) {
                    state = apply(&ir, state, Op::Tick(addr)).0;
                }
            }
        }
    }
    (state, stale)
}

#[test]
fn p2_pre_move_ticks_never_change_run() {
    check(
        "p2-pre-move-ticks-never-change-run",
        DEFAULT_CASES,
        post_move_scenario(),
        |(plan, ops)| {
            let (_, stale) = run_post_move(&plan, &ops);
            // 正解は judge を使わず、宛先の世代が操作前の世代より小さいことだけで決める。
            for (before, after, events) in &stale {
                prop_assert!(Generation(1) < before.run.generation);
                prop_assert_eq!(&after.run, &before.run);
                let Some(current) = before.run.current.clone() else {
                    return Err(TestCaseError::fail("current is None after move"));
                };
                prop_assert_eq!(
                    events,
                    &vec![Event::Ignored {
                        at: Stamp {
                            generation: before.run.generation,
                            step: current
                        },
                        reason: IgnoreReason::Stale
                    }]
                );
                prop_assert_eq!(after.log.len(), before.log.len() + 1);
            }
            Ok(())
        },
    );
}

#[test]
fn post_move_sequence_is_deterministic() {
    check(
        "post-move-sequence-is-deterministic",
        DEFAULT_CASES,
        post_move_scenario(),
        |(plan, ops)| {
            assert_replay_eq(
                |p: &[(Plan, Vec<PostOp>)]| {
                    let (plan, ops) = &p[0];
                    let (last, stale) = run_post_move(plan, ops);
                    let events: Vec<Vec<Event>> = stale.into_iter().map(|t| t.2).collect();
                    (last, events)
                },
                std::slice::from_ref(&(plan, ops)),
            )
        },
    );
}
