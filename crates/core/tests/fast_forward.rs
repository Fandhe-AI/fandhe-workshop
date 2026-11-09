//! 進行の連続適用 `fast_forward` の試験（TASK-227 1/2、EVT-17）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1（進行 5 つ）で、早送りの入（連続適用）と切（タイマー相当の 1 件ずつの適用）が、
//! 最終の状態・イベント列・履歴の `ops`・`Progress` の数で一致することを固定する。性質テストは、任意の操作列の
//! 各操作の後に両側へ同じ進行の扱いをして一致を確かめる。早送りの途中の停止と場面を削らないことの試験は TASK-227 2/2。
//! 呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{guided_op_specs, op_specs, Harness, OpSpec, Which};
use common::property::{check, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::fast_forward::{fast_forward, FastForwardEnd};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;
use serde_json::json;

const ST1: &str = "main/st-1";

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// st-1 が `Entered`・処理中でない・世代 1 の状態。
fn entered() -> SessionState {
    let u = uid(ST1);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "created".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn busy_session(ir: Ir) -> Session {
    let mut s = Session::from_state(ir, entered());
    s.apply(first_note());
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 5 }));
    s
}

fn busy_recorded(ir: Ir) -> RecordedSession {
    let mut r = RecordedSession::new(Session::from_state(ir, entered()), "s-1");
    r.apply(first_note());
    assert_eq!(
        r.session().state().run.busy,
        Some(Busy { done: 0, total: 5 })
    );
    r
}

/// 宛先が進まない（世代・Step が変わった、または番号が増えない）か。
fn is_stuck(state: &SessionState, prev: &TickAddr) -> bool {
    next_tick(state).is_some_and(|n| {
        n.generation != prev.generation || n.step != prev.step || n.index <= prev.index
    })
}

/// 切の基準（タイマー相当）。処理中が解けるか番号が進まなくなるまで 1 件ずつ適用する。
fn tick_by_tick(s: &mut Session) -> (Vec<TickAddr>, Vec<Event>) {
    let (mut ticks, mut events) = (Vec::new(), Vec::new());
    while let Some(addr) = next_tick(s.state()) {
        events.extend(s.apply(Op::Tick(addr.clone())).to_vec());
        let stuck = is_stuck(s.state(), &addr);
        ticks.push(addr);
        if stuck {
            break;
        }
    }
    (ticks, events)
}

fn tick_by_tick_recorded(r: &mut RecordedSession) {
    while let Some(addr) = next_tick(r.session().state()) {
        r.apply(Op::Tick(addr.clone()));
        if is_stuck(r.session().state(), &addr) {
            break;
        }
    }
}

fn progress_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

#[test]
fn fast_forward_matches_tick_by_tick() {
    let mut off = busy_session(base_ir());
    let mut on = off.clone();
    let (off_ticks, off_events) = tick_by_tick(&mut off);
    let ff = fast_forward(&mut on, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.ticks.len(), 5);
    assert_eq!(ff.ticks, off_ticks);
    assert_eq!(ff.events, off_events);
    assert_eq!(progress_count(&ff.events), 5);
    assert_eq!(on.state(), off.state());
    assert!(on.state().run.busy.is_none());
}

#[test]
fn fast_forward_records_same_ops() {
    let mut off = busy_recorded(base_ir());
    let mut on = off.clone();
    tick_by_tick_recorded(&mut off);
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(on.ops(), off.ops());
    assert_eq!(on.ops().len(), 6, "受付 1 + 進行 5");
    assert_eq!(on.envelope_for_save(), off.envelope_for_save());
    assert_eq!(on.session().state(), off.session().state());
    let replayed = replay(on.session().ir(), entered(), on.ops()).expect("replay");
    assert_eq!(&replayed, on.session().state());
}

#[test]
fn fast_forward_not_busy_is_noop() {
    let mut s = Session::from_state(base_ir(), entered());
    let before = s.state().clone();
    let ff = fast_forward(&mut s, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::NotBusy);
    assert!(ff.ticks.is_empty() && ff.events.is_empty());
    assert_eq!(s.state(), &before);

    let mut r = RecordedSession::new(Session::from_state(base_ir(), entered()), "s-1");
    let ff = r.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::NotBusy);
    assert!(r.ops().is_empty());
    assert_eq!(r.session().state(), &before);
}

#[test]
fn fast_forward_limit_splits_without_difference() {
    let mut whole = busy_recorded(base_ir());
    let mut split = whole.clone();
    let mut off = whole.clone();
    whole.fast_forward(u32::MAX);
    tick_by_tick_recorded(&mut off);

    let zero = split.fast_forward(0);
    assert_eq!(zero.end, FastForwardEnd::LimitReached);
    assert!(zero.ticks.is_empty());
    assert_eq!(split.ops().len(), 1);

    let part = split.fast_forward(2);
    assert_eq!(part.end, FastForwardEnd::LimitReached);
    assert_eq!(part.ticks.len(), 2);
    assert_eq!(
        split.session().state().run.busy,
        Some(Busy { done: 2, total: 5 })
    );
    let rest = split.fast_forward(u32::MAX);
    assert_eq!(rest.end, FastForwardEnd::Finished);
    assert_eq!(rest.ticks.len(), 3);

    assert_eq!(split.session().state(), whole.session().state());
    assert_eq!(split.session().state(), off.session().state());
    assert_eq!(split.ops(), whole.ops());
    assert_eq!(split.ops(), off.ops());
}

#[test]
fn fast_forward_confirm_none_achieves_same_as_ticks() {
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == ST1) {
        n.step.confirm = Some(json!({ "kind": "none", "reason": "memo only" }));
    }
    let mut off = busy_session(ir);
    let mut on = off.clone();
    let (_, off_events) = tick_by_tick(&mut off);
    let ff = fast_forward(&mut on, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.events, off_events);
    assert_eq!(on.state(), off.state());
    assert!(ff
        .events
        .iter()
        .any(|e| matches!(e, Event::Achieved { .. })));
    assert!(ff
        .events
        .iter()
        .any(|e| matches!(e, Event::Transitioned { .. })));
    assert!(ff.events.iter().any(|e| matches!(e, Event::Entered { .. })));
    assert!(on.state().run.busy.is_none());
    assert_ne!(on.state().run.current, Some(uid(ST1)));
}

#[test]
fn fast_forward_stalls_on_unreadable_processing() {
    let mut state = busy_session(base_ir()).into_state();
    state.run.busy = Some(Busy { done: 0, total: 4 });
    let mut off = Session::from_state(base_ir(), state);
    let mut on = off.clone();
    let (off_ticks, off_events) = tick_by_tick(&mut off);
    assert_eq!(off_ticks.len(), 1);
    let ff = fast_forward(&mut on, 100);
    assert_eq!(ff.end, FastForwardEnd::Stalled);
    assert_eq!(ff.ticks.len(), 1);
    assert!(matches!(ff.events.as_slice(), [Event::Warning { .. }]));
    assert_eq!(ff.events, off_events);
    assert_eq!(on.state(), off.state());
}

/// 任意の操作列の各操作の後に、早送りの入と切で最終の状態・ログ・`ops`・`Progress` の数が一致する。
///
/// 操作は切の側の `Harness` が解決し、同じ `Op` を両側へ渡す。切の側は各操作の後に進行を 1 件ずつ、入の側は連続適用で進める。
fn on_off_property(specs: &[OpSpec]) -> Result<(), TestCaseError> {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    let ir = load_ir(&bytes).expect("load ir");
    let initial = sample.initial;
    let mut harness = Harness::via_session(&ir, initial.clone());
    let mut off = RecordedSession::new(Session::from_state(ir.clone(), initial), "s-1");
    let mut on = off.clone();
    let (mut off_progress, mut on_progress) = (0usize, 0usize);
    for spec in specs {
        let applied = harness.step(spec);
        match &applied.op {
            Some(op) => {
                off_progress += progress_count(off.apply(op.clone()));
                on_progress += progress_count(on.apply(op.clone()));
            }
            None => {
                off_progress += progress_count(off.reload());
                on_progress += progress_count(on.reload());
            }
        }
        // 切: 処理中が解けるか番号が進まなくなるまで、タイマー相当で 1 件ずつ。
        let mut guard = 0u32;
        while let Some(addr) = next_tick(harness.state()) {
            let a = harness.step(&OpSpec::Tick(Which::Current));
            if let Some(op) = a.op {
                off_progress += progress_count(off.apply(op));
            }
            guard += 1;
            if is_stuck(harness.state(), &addr) || guard > 10_000 {
                break;
            }
        }
        // 入: 連続適用。
        on_progress += progress_count(&on.fast_forward(u32::MAX).events);
        prop_assert_eq!(on.session().state(), off.session().state());
        prop_assert_eq!(on.ops(), off.ops());
        prop_assert_eq!(on_progress, off_progress);
    }
    prop_assert_eq!(on.session().state(), harness.state());
    Ok(())
}

#[test]
fn fast_forward_on_off_same_final_state() {
    check("fast-forward-on-off", DEFAULT_CASES, op_specs(), |specs| {
        on_off_property(&specs)
    });
}

#[test]
fn fast_forward_on_off_same_final_state_guided() {
    check(
        "fast-forward-on-off-guided",
        DEFAULT_CASES,
        guided_op_specs(),
        |specs| on_off_property(&specs),
    );
}
