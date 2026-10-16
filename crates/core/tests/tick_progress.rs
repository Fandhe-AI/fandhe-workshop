//! 宛先が一致する進行 `Op::Tick` の試験（TASK-210 2/3、仕様 EVT-8 と EVT-16・性質テスト P-16・P-2）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1（進行 5 つ）で、宛先が一致した進行が 1 つずつ進み、
//! 食い違う宛先が状態を変えず、進行の宣言が誤っていれば `Warning` だけで倒れることを確かめる。
//! 食い違う宛先は状態を変えず `Ignored` を 1 件だけ記録する（TASK-210 3/3）。呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::property::{assert_replay_eq, check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Cause, Event, IgnoreReason};
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::judge::judge_tick;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{apply, load_ir, Session};
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

fn first_note() -> Input {
    Input::Command("/make-note first-note".into())
}

/// 受付の直後（処理中、done 0 / total 5）のセッション。
fn busy_session(ir: Ir) -> Session {
    let mut s = Session::from_state(ir, entered());
    s.apply(Op::Input(first_note()));
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 5 }));
    s
}

fn tick(s: &mut Session) -> Vec<Event> {
    let addr = next_tick(s.state()).expect("next tick");
    s.apply(Op::Tick(addr)).to_vec()
}

fn progress_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

fn ignored_reasons(events: &[Event]) -> Vec<IgnoreReason> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Ignored { reason, .. } => Some(*reason),
            _ => None,
        })
        .collect()
}

/// 操作前の状態の世代と現在の Step の `Stamp`。
fn stamp_of(run: &RunState) -> Stamp {
    Stamp {
        generation: run.generation,
        step: run.current.clone().expect("current"),
    }
}

fn status(s: &Session, step: &str) -> StepStatus {
    let u = uid(step);
    s.state()
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

#[test]
fn fixed_sequence_matches_sample_shape() {
    let mut s = busy_session(base_ir());
    let names = ["p1-prepare", "p2-create", "p3-write", "p4-check", "p5-save"];
    for (i, name) in names.iter().enumerate() {
        let events = tick(&mut s);
        match events.first() {
            Some(Event::Progress {
                at,
                name: n,
                index,
                total,
            }) => {
                assert_eq!(n, name);
                assert_eq!(usize::try_from(*index).ok(), Some(i));
                assert_eq!(*total, 5);
                assert_eq!(at.generation, Generation(1));
                assert_eq!(at.step, uid(ST1));
            }
            other => panic!("tick {i}: first event is {other:?}"),
        }
        let changed: Vec<(&str, Cause)> = events
            .iter()
            .filter_map(|e| match e {
                Event::Changed { path, cause, .. } => Some((path.as_str(), *cause)),
                _ => None,
            })
            .collect();
        if i == 2 || i == 4 {
            assert_eq!(changed, vec![("notes/draft", Cause::Input)], "tick {i}");
        } else {
            assert!(changed.is_empty(), "tick {i}: {events:?}");
        }
        assert!(!events
            .iter()
            .any(|e| matches!(e, Event::InputReceived { .. } | Event::Judged { .. })));
    }
    let state = s.state();
    assert_eq!(state.run.busy, None);
    assert_eq!(status(&s, ST1), StepStatus::Accepted);
    assert_eq!(state.run.generation, Generation(1));
    assert_eq!(state.run.current, Some(uid(ST1)));
}

#[test]
fn matching_tick_advances_once() {
    let mut s = busy_session(base_ir());
    let events = tick(&mut s);
    assert_eq!(progress_count(&events), 1);
    assert_eq!(s.state().run.busy, Some(Busy { done: 1, total: 5 }));
}

#[test]
fn duplicate_index_advances_once() {
    let mut s = busy_session(base_ir());
    let addr = next_tick(s.state()).expect("next");
    s.apply(Op::Tick(addr.clone()));
    let before = s.state().run.clone();
    let log_len = s.state().log.len();
    let events = s.apply(Op::Tick(addr)).to_vec();
    assert_eq!(progress_count(&events), 0);
    assert_eq!(ignored_reasons(&events), vec![IgnoreReason::Stale]);
    assert_eq!(events.len(), 1);
    assert_eq!(s.state().run, before);
    assert_eq!(s.state().log.len(), log_len + 1);
    assert_eq!(s.state().run.busy, Some(Busy { done: 1, total: 5 }));
}

#[test]
fn duplicate_of_last_tick_is_no_processing() {
    let mut s = busy_session(base_ir());
    let mut last = None;
    for _ in 0..5 {
        last = next_tick(s.state());
        tick(&mut s);
    }
    assert_eq!(s.state().run.busy, None);
    let before = s.state().run.clone();
    let events = s.apply(Op::Tick(last.expect("last"))).to_vec();
    assert_eq!(ignored_reasons(&events), vec![IgnoreReason::NoProcessing]);
    assert_eq!(events.len(), 1);
    assert_eq!(s.state().run, before);
}

fn mismatched_variants(good: &TickAddr) -> Vec<TickAddr> {
    vec![
        // 古い世代・新しすぎる世代。
        TickAddr {
            generation: Generation(0),
            ..good.clone()
        },
        TickAddr {
            generation: Generation(2),
            ..good.clone()
        },
        // 移動前の Step 宛て（移動の操作は TASK-213。ここでは宛先の Step を変えて代替する）。
        TickAddr {
            step: uid("main/st-2"),
            ..good.clone()
        },
        // 同じ番号の二重の届き相当（番号が合わない）。
        TickAddr {
            index: 1,
            ..good.clone()
        },
    ]
}

#[test]
fn mismatched_tick_records_one_ignored() {
    let good = next_tick(busy_session(base_ir()).state()).expect("next");
    for addr in mismatched_variants(&good) {
        let mut s = busy_session(base_ir());
        let before = s.state().run.clone();
        let log_len = s.state().log.len();
        let events = s.apply(Op::Tick(addr.clone())).to_vec();
        assert_eq!(
            events,
            vec![Event::Ignored {
                at: stamp_of(&before),
                reason: IgnoreReason::Stale
            }],
            "{addr:?}"
        );
        assert_eq!(progress_count(&events), 0, "{addr:?}");
        assert_eq!(s.state().run, before, "{addr:?}");
        assert_eq!(s.state().log.len(), log_len + 1, "{addr:?}");
    }
    // 処理中でない。
    let mut s = Session::from_state(base_ir(), entered());
    let before = s.state().run.clone();
    let events = s.apply(Op::Tick(good.clone())).to_vec();
    assert_eq!(
        events,
        vec![Event::Ignored {
            at: stamp_of(&before),
            reason: IgnoreReason::NoProcessing
        }]
    );
    assert_eq!(s.state().run, before);
    // 現在の Step が無い（`Stamp` を作れないので記録しない）。
    let mut none = SessionState::default();
    none.run.busy = Some(Busy { done: 0, total: 5 });
    let mut s = Session::from_state(base_ir(), none);
    let before = s.state().run.clone();
    let events = s.apply(Op::Tick(good)).to_vec();
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(s.state().run, before);
}

#[test]
fn mismatch_is_checked_before_processing_declaration() {
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == ST1) {
        n.step.processing = vec![json!({ "label": "SECRET-MARK" }); 5];
    }
    let mut s = busy_session(ir);
    let good = next_tick(s.state()).expect("next");
    let before = s.state().run.clone();
    for addr in mismatched_variants(&good) {
        let events = s.apply(Op::Tick(addr)).to_vec();
        assert_eq!(ignored_reasons(&events), vec![IgnoreReason::Stale]);
        assert_eq!(events.len(), 1);
        assert_eq!(s.state().run, before);
    }
}

fn only_warning(events: &[Event]) -> bool {
    matches!(events, [Event::Warning { .. }])
}

#[test]
fn invalid_processing_is_fail_closed() {
    // 進行の形が誤っている。
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == ST1) {
        n.step.processing = vec![json!({ "label": "SECRET-MARK" }); 5];
    }
    let mut s = busy_session(ir);
    let before = s.state().run.clone();
    let events = tick(&mut s);
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(s.state().run, before);
    if let Some(Event::Warning { message, .. }) = events.first() {
        assert!(!message.contains("SECRET-MARK"));
    }
    // 総数の食い違い。
    let mut state = busy_session(base_ir()).into_state();
    state.run.busy = Some(Busy { done: 0, total: 4 });
    let mut s = Session::from_state(base_ir(), state);
    let before = s.state().run.clone();
    let events = tick(&mut s);
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(s.state().run, before);
    // 範囲外の done。
    let mut state = busy_session(base_ir()).into_state();
    state.run.busy = Some(Busy { done: 5, total: 5 });
    let mut s = Session::from_state(base_ir(), state);
    let before = s.state().run.clone();
    let events = tick(&mut s);
    assert!(only_warning(&events), "{events:?}");
    assert_eq!(s.state().run, before);
}

#[derive(Debug, Clone)]
enum TestOp {
    Apply(Input),
    /// 一致する宛先の進行（無ければ何もしない）。
    Next,
    /// 任意の宛先の進行（世代・番号・別の Step か）。
    Any(u64, u32, bool),
}

fn op_strategy() -> impl Strategy<Value = TestOp> {
    prop_oneof![
        4 => Just(TestOp::Apply(first_note())),
        1 => Just(TestOp::Apply(Input::Cancel)),
        1 => Just(TestOp::Apply(Input::Command("/nothing".into()))),
        2 => Just(TestOp::Apply(Input::Approval { decision: Decision::Approve, target: None })),
        6 => Just(TestOp::Next),
        3 => (0u64..3, 0u32..7, any::<bool>()).prop_map(|(g, i, o)| TestOp::Any(g, i, o)),
    ]
}

fn to_op(state: &SessionState, op: &TestOp) -> Option<Op> {
    match op {
        TestOp::Apply(i) => Some(Op::Input(i.clone())),
        TestOp::Next => next_tick(state).map(Op::Tick),
        TestOp::Any(g, i, other) => Some(Op::Tick(TickAddr {
            generation: Generation(*g),
            step: uid(if *other { "main/st-2" } else { ST1 }),
            index: *i,
        })),
    }
}

fn run_all(ops: &[TestOp]) -> SessionState {
    let ir = base_ir();
    let mut state = entered();
    for op in ops {
        if let Some(o) = to_op(&state, op) {
            state = apply(&ir, state, o).0;
        }
    }
    state
}

#[test]
fn p16_replay_is_deterministic() {
    check(
        "p16-replay-is-deterministic",
        DEFAULT_CASES,
        op_sequence(op_strategy()),
        |ops| assert_replay_eq(run_all, &ops),
    );
}

#[test]
fn p2_mismatched_tick_keeps_run() {
    check(
        "p2-mismatched-tick-keeps-run",
        DEFAULT_CASES,
        op_sequence(op_strategy()),
        |ops| {
            let ir = base_ir();
            let mut state = entered();
            for op in &ops {
                let Some(o) = to_op(&state, op) else { continue };
                let ignored = matches!(
                    &o,
                    Op::Tick(a) if judge_tick(&state.run, a).judgment == Judgment::Ignored
                );
                let before = state.run.clone();
                let log_len = state.log.len();
                let (next, events) = apply(&ir, state, o);
                if ignored {
                    prop_assert_eq!(&next.run, &before);
                    prop_assert_eq!(progress_count(&events), 0);
                    if before.current.is_some() {
                        prop_assert_eq!(events.len(), 1);
                        prop_assert_eq!(ignored_reasons(&events).len(), 1);
                        prop_assert_eq!(next.log.len(), log_len + 1);
                    }
                }
                state = next;
            }
            Ok(())
        },
    );
}
