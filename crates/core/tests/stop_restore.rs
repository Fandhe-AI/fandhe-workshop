//! 処理中の停止と入口の写しへの復帰の試験（TASK-211 1/2、仕様 EVT-9・性質テスト P-4）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1 を、サンプルの `initial`（入口の写しを持つ）から受け付け、
//! 進行を進めてから停止したとき、世代が +1 になり、処理中が解け、受付後に作られた世界状態が消えることを確かめる。
//! 状態に写しが無い・別の Step のものなら、ログから受付の時点を導いて戻る（`entry_derive`、SAFE-35）。戻り先が
//! 不確かなとき（ログからも導けない・世代があふれる）は `Warning` だけで状態を変えない。
//! 停止の前の世代を宛先にした進行が停止の後に届いても `run` は変わらず `Ignored(Stale)` が 1 件だけ増えること、
//! 停止の後に入力を受け付け直せること（TASK-211 2/2）も確かめる。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::property::{assert_replay_eq, check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Cause, Event, IgnoreReason};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::{Busy, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
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

fn stop() -> Op {
    Op::Control(Control::Stop)
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn stamp(g: u64) -> Stamp {
    Stamp {
        generation: Generation(g),
        step: uid(ST1),
    }
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
fn stop_after_two_ticks_removes_draft() {
    let ir = ir();
    let state = busy_state(&ir, 2);
    assert_eq!(state.run.busy, Some(Busy { done: 2, total: 5 }));
    assert!(state.run.world.contains_key("notes/draft"));
    let (next, events) = apply(&ir, state, stop());
    assert_eq!(next.run.generation, Generation(2));
    assert_eq!(next.run.busy, None);
    assert!(!next.run.world.contains_key("notes/draft"));
    assert!(next.run.world.is_empty());
    let u = uid(ST1);
    let status = next
        .run
        .slices
        .get(&u.root())
        .and_then(|s| s.status.get(&u))
        .copied();
    assert_eq!(status, Some(StepStatus::Entered));
    assert_eq!(events.len(), 2, "{events:?}");
    assert_eq!(
        events.first(),
        Some(&Event::Stopped {
            at: stamp(1),
            new_generation: Generation(2)
        })
    );
    match events.get(1) {
        Some(Event::Changed {
            at,
            path,
            after,
            cause,
            ..
        }) => {
            assert_eq!(at, &stamp(2));
            assert_eq!(path, "notes/draft");
            assert!(after.is_none());
            assert_eq!(*cause, Cause::Stop);
        }
        other => panic!("second event: {other:?}"),
    }
    // 関門が黙って直さないよう、世代を運ぶイベントと状態の一致を明示して比べる。
    assert!(matches!(
        events.first(),
        Some(Event::Stopped { new_generation, .. }) if *new_generation == next.run.generation
    ));
}

#[test]
fn stop_matches_sample_e06() {
    let s = load();
    let ir = ir();
    let mut state = s.initial.clone();
    for id in ["e02", "e04", "e05", "e06", "e07", "e08"] {
        let e = s.entries.iter().find(|e| e.id == id).expect("entry");
        assert_eq!(state.run, e.before, "{id}: before");
        let (next, events) = apply(&ir, state, e.op.clone());
        assert_eq!(events, e.events, "{id}: events");
        assert_eq!(next.run, e.after, "{id}: after");
        state = next;
    }
}

#[test]
fn idle_stop_is_cancel_rejection_without_state_change() {
    // 処理中でない停止は取り消しの入力として扱われる（TASK-212、EVT-10）。規則が無ければ拒否で、状態は変わらない。
    let ir = ir();
    let state = load().initial;
    let (next, events) = apply(&ir, state.clone(), stop());
    assert_eq!(events.len(), 2, "{events:?}");
    assert!(matches!(
        events[0],
        Event::InputReceived {
            input: Input::Cancel,
            ..
        }
    ));
    assert!(matches!(events[1], Event::Judged { .. }));
    assert_eq!(next.run, state.run);
    assert_eq!(next.run.generation, Generation(1));
    assert_eq!(next.log.len(), state.log.len() + 2);
}

/// 状態に入口の写しが無くても（通常の操作の形）、ログから受付の時点を導いて戻る。結果は、状態に写しがある
/// ときと同じになる（導いた写しが受付の直前の状態と一致する。SAFE-35・`entry_derive`）。
#[test]
fn stop_without_stored_entry_derives_the_same_entry_from_log() {
    let ir = ir();
    let with_copy = busy_state(&ir, 2);
    assert!(with_copy.entry.is_some());
    let mut without_copy = with_copy.clone();
    without_copy.entry = None;
    let (expected, expected_events) = apply(&ir, with_copy, stop());
    let (next, events) = apply(&ir, without_copy, stop());
    assert!(
        matches!(events.first(), Some(Event::Stopped { .. })),
        "{events:?}"
    );
    assert_eq!(events, expected_events);
    assert_eq!(next.run, expected.run);
    assert_eq!(next.entry, None);
}

/// 状態の写しも無く、ログに受付も無い（受付の時点を一意に決められない）ときは、`Warning` だけで状態を変えない。
#[test]
fn stop_without_entry_or_accept_in_log_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.entry = None;
    state.log.clear();
    let before = state.clone();
    let (next, events) = apply(&ir, state, stop());
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(next.run, before.run);
    assert_eq!(next.run.generation, Generation(1));
    assert_eq!(next.log.len(), before.log.len() + 1);
}

/// 状態の写しが別の Step のものなら使わず、ログから導く。ログからも導けなければ `Warning` だけで状態を変えない。
#[test]
fn stop_with_other_step_entry_falls_back_to_log() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    if let Some(e) = state.entry.as_mut() {
        e.run.current = Some(uid("main/st-2"));
    }
    let (next, events) = apply(&ir, state.clone(), stop());
    assert!(
        matches!(events.first(), Some(Event::Stopped { .. })),
        "{events:?}"
    );
    assert_eq!(next.run.current, Some(uid(ST1)));
    assert!(next.run.world.is_empty());

    state.log.clear();
    let before = state.run.clone();
    let (next, events) = apply(&ir, state, stop());
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(next.run, before);
}

#[test]
fn stop_overflow_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.run.generation = Generation(u64::MAX);
    let before = state.run.clone();
    let (next, events) = apply(&ir, state, stop());
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(next.run, before);
    if let Some(Event::Warning { message, .. }) = events.first() {
        assert!(!message.contains("notes"));
    }
}

#[test]
fn stop_keeps_drafts_hints_and_entry() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.drafts.insert(uid(ST1).root(), "draft text".into());
    state.hints_opened.insert(uid(ST1), 2);
    let before = state.clone();
    let (next, events) = apply(&ir, state, stop());
    assert_eq!(next.drafts, before.drafts);
    assert_eq!(next.hints_opened, before.hints_opened);
    assert_eq!(next.entry, before.entry);
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

#[derive(Debug, Clone)]
struct Plan {
    ticks: u32,
    resends: u32,
}

fn plan_strategy() -> impl Strategy<Value = Plan> {
    (0u32..5, 0u32..3).prop_map(|(ticks, resends)| Plan { ticks, resends })
}

/// 受付・進行・受付の再送（拒否）の後に停止した結果。(停止前, 停止後, 停止のイベント列)。
fn run_plan(plan: &Plan) -> (SessionState, SessionState, Vec<Event>) {
    let ir = ir();
    let mut state = busy_state(&ir, plan.ticks);
    for _ in 0..plan.resends {
        state = apply(&ir, state, first_note()).0;
    }
    let before = state.clone();
    let (next, events) = apply(&ir, state, stop());
    (before, next, events)
}

#[test]
fn p4_stop_returns_to_entry() {
    check(
        "p4-stop-returns-to-entry",
        DEFAULT_CASES,
        plan_strategy(),
        |plan| {
            let (before, next, events) = run_plan(&plan);
            let entry = before.entry.clone().expect("entry");
            let mut expected = entry.run;
            expected.generation = next.run.generation;
            expected.busy = None;
            prop_assert_eq!(&next.run, &expected);
            prop_assert_eq!(Some(next.run.generation), before.run.generation.next());
            let stopped: Vec<_> = events
                .iter()
                .filter_map(|e| match e {
                    Event::Stopped { new_generation, .. } => Some(*new_generation),
                    _ => None,
                })
                .collect();
            prop_assert_eq!(stopped, vec![next.run.generation]);
            for e in &events {
                if let Event::Changed { cause, .. } = e {
                    prop_assert_eq!(*cause, Cause::Stop);
                }
            }
            Ok(())
        },
    );
}

#[test]
fn stop_is_deterministic() {
    check(
        "stop-is-deterministic",
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

/// 停止の前の世代（1）を宛先にした進行。
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

/// 処理中の状態を停止した後の状態。
fn stopped_state(ir: &Ir, ticks: u32) -> SessionState {
    apply(ir, busy_state(ir, ticks), stop()).0
}

#[test]
fn stale_tick_after_stop_is_ignored() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let addr = next_tick(&busy).expect("next tick");
    assert_eq!(addr.generation, Generation(1));
    let stopped = apply(&ir, busy, stop()).0;
    let (next, events) = apply(&ir, stopped.clone(), Op::Tick(addr));
    assert_eq!(next.run, stopped.run);
    assert_eq!(next.run.generation, Generation(2));
    assert_eq!(
        events,
        vec![Event::Ignored {
            at: stamp(2),
            reason: IgnoreReason::Stale
        }]
    );
    assert_eq!(next.log.len(), stopped.log.len() + 1);
}

#[test]
fn stale_tick_after_reinput_is_ignored() {
    let ir = ir();
    let stopped = stopped_state(&ir, 2);
    let (state, _) = apply(&ir, stopped, first_note());
    assert_eq!(state.run.busy, Some(Busy { done: 0, total: 5 }));
    assert!(state.run.world.contains_key("notes/draft"));
    let (after, events) = apply(&ir, state.clone(), old_tick(ST1, 0));
    assert_eq!(after.run, state.run);
    assert_eq!(after.run.busy, Some(Busy { done: 0, total: 5 }));
    assert_eq!(progress_count(&events), 0);
    assert_eq!(
        events,
        vec![Event::Ignored {
            at: stamp(2),
            reason: IgnoreReason::Stale
        }]
    );
    let addr = next_tick(&after).expect("next tick");
    assert_eq!(addr.generation, Generation(2));
    let (done, events) = apply(&ir, after, Op::Tick(addr));
    assert_eq!(progress_count(&events), 1);
    assert_eq!(done.run.busy, Some(Busy { done: 1, total: 5 }));
}

#[test]
fn input_after_stop_is_accepted_and_completes() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let (refused, events) = apply(&ir, busy.clone(), first_note());
    assert_eq!(refused.run, busy.run, "busy refuses re-input");
    assert!(!events.is_empty());
    let stale = next_tick(&busy).expect("next tick");
    let stopped = apply(&ir, busy, stop()).0;
    let (mut state, events) = apply(&ir, stopped, first_note());
    assert!(matches!(events.first(), Some(Event::InputReceived { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::Judged { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::Accepted { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::Changed { .. })));
    let pos = |f: fn(&Event) -> bool| events.iter().position(f);
    let i_recv = pos(|e| matches!(e, Event::InputReceived { .. }));
    let i_judged = pos(|e| matches!(e, Event::Judged { .. }));
    let i_acc = pos(|e| matches!(e, Event::Accepted { .. }));
    let i_chg = pos(|e| matches!(e, Event::Changed { .. }));
    assert!(i_recv < i_judged && i_judged < i_acc && i_acc < i_chg);
    assert_eq!(state.run.generation, Generation(2));
    assert_eq!(state.run.busy, Some(Busy { done: 0, total: 5 }));
    for n in 0..5 {
        if n == 2 {
            let before = state.run.clone();
            state = apply(&ir, state, Op::Tick(stale.clone())).0;
            assert_eq!(state.run, before);
        }
        let addr = next_tick(&state).expect("next tick");
        state = apply(&ir, state, Op::Tick(addr)).0;
    }
    assert_eq!(state.run.busy, None);
}

#[derive(Debug, Clone)]
enum PostOp {
    Old { second_step: bool, index: u32 },
    Reinput,
    Next,
}

fn post_op_strategy() -> impl Strategy<Value = PostOp> {
    prop_oneof![
        (any::<bool>(), 0u32..7)
            .prop_map(|(second_step, index)| PostOp::Old { second_step, index }),
        Just(PostOp::Reinput),
        Just(PostOp::Next),
    ]
}

fn post_stop_scenario() -> impl Strategy<Value = (Plan, Vec<PostOp>)> {
    (plan_strategy(), op_sequence(post_op_strategy()))
}

/// 停止の後に操作列を流す。古い世代の宛先の進行ごとに (操作前, 操作後, イベント列) を返す。
fn run_post_stop(
    plan: &Plan,
    ops: &[PostOp],
) -> (SessionState, Vec<(SessionState, SessionState, Vec<Event>)>) {
    let ir = ir();
    let mut state = busy_state(&ir, plan.ticks);
    for _ in 0..plan.resends {
        state = apply(&ir, state, first_note()).0;
    }
    state = apply(&ir, state, stop()).0;
    let mut stale = Vec::new();
    for op in ops {
        match op {
            PostOp::Old { second_step, index } => {
                let step = if *second_step { "main/st-2" } else { ST1 };
                let (next, events) = apply(&ir, state.clone(), old_tick(step, *index));
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
fn p2_pre_stop_ticks_never_change_run() {
    check(
        "p2-pre-stop-ticks-never-change-run",
        DEFAULT_CASES,
        post_stop_scenario(),
        |(plan, ops)| {
            let (last, stale) = run_post_stop(&plan, &ops);
            // 正解は judge を使わず、宛先の世代が操作前の世代より小さいことだけで決める。
            prop_assert_eq!(last.run.generation, Generation(2));
            for (before, after, events) in &stale {
                prop_assert!(Generation(1) < before.run.generation);
                prop_assert_eq!(&after.run, &before.run);
                prop_assert_eq!(
                    events,
                    &vec![Event::Ignored {
                        at: Stamp {
                            generation: before.run.generation,
                            step: uid(ST1)
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
fn post_stop_sequence_is_deterministic() {
    check(
        "post-stop-sequence-is-deterministic",
        DEFAULT_CASES,
        post_stop_scenario(),
        |(plan, ops)| {
            assert_replay_eq(
                |p: &[(Plan, Vec<PostOp>)]| {
                    let (plan, ops) = &p[0];
                    let (last, stale) = run_post_stop(plan, ops);
                    let events: Vec<Vec<Event>> = stale.into_iter().map(|t| t.2).collect();
                    (last, events)
                },
                std::slice::from_ref(&(plan, ops)),
            )
        },
    );
}
