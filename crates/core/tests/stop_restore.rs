//! 処理中の停止と入口の写しへの復帰の試験（TASK-211 1/2、仕様 EVT-9・性質テスト P-4）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1 を、サンプルの `initial`（入口の写しを持つ）から受け付け、
//! 進行を進めてから停止したとき、世代が +1 になり、処理中が解け、受付後に作られた世界状態が消えることを確かめる。
//! 戻り先が不確かなとき（写しが無い・別の Step のもの・世代があふれる）は `Warning` だけで状態を変えない。
//! 停止前の進行の無視と停止後の再入力は TASK-211 2/2 の担当で、ここでは扱わない。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::property::{assert_replay_eq, check, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
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
    for id in ["e02", "e04", "e05", "e06"] {
        let e = s.entries.iter().find(|e| e.id == id).expect("entry");
        assert_eq!(state.run, e.before, "{id}: before");
        let (next, events) = apply(&ir, state, e.op.clone());
        assert_eq!(events, e.events, "{id}: events");
        assert_eq!(next.run, e.after, "{id}: after");
        state = next;
    }
}

#[test]
fn idle_stop_changes_nothing() {
    let ir = ir();
    let state = load().initial;
    let (next, events) = apply(&ir, state.clone(), stop());
    assert!(events.is_empty());
    assert_eq!(next, state);
    assert_eq!(next.run.generation, Generation(1));
}

#[test]
fn stop_without_entry_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    state.entry = None;
    let before = state.clone();
    let (next, events) = apply(&ir, state, stop());
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(next.run, before.run);
    assert_eq!(next.run.generation, Generation(1));
    assert_eq!(next.log.len(), before.log.len() + 1);
}

#[test]
fn stop_with_other_step_entry_warns_only() {
    let ir = ir();
    let mut state = busy_state(&ir, 2);
    if let Some(e) = state.entry.as_mut() {
        e.run.current = Some(uid("main/st-2"));
    }
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
