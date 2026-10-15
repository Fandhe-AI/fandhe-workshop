//! 受付の一回性と処理中の拒否 `busy` の試験（TASK-209 1/2、仕様 EVT-7 と性質テスト P-6）。
//!
//! 受付は（世代, Step）につき 1 回だけで、処理中の Step への同じ入力の連打は 2 回目以降が `busy` になり、
//! 状態が変わらない。`busy` はイベントに分類が載らない（契約の版 2 は `Judged` を含めない）ため、
//! 各送信の直前の状態で [`classify_busy`] が `Some` を返した回数で数え、あわせてその送信の結果が
//! `InputReceived` だけであることを確かめる。連打の場面は `processing` を持つ st-1 で行う
//! （st-2・st-3 は処理中にならず、2 回目の入力は確認の判定に回る）。
//!
//! 擬似操作 `FinishProcessing` は、処理中（`run.busy`）を畳む進行の照合（TASK-210）の代わりである。
//! `Op::Tick` が処理を進めるようになった後は、これを `Op::Tick` に置き換えてよい。
//! `Op::Control`・`Op::Tick` は列に入れない（世代を動かす経路を作らず、P-6 の前提を保つ）。
//! 残り続けるボタンへの連打とログの増え方は TASK-209 2/2 が扱う。呼び出し元は CI の `make test` のみである。

mod common;

use std::collections::BTreeSet;
use std::path::PathBuf;

use common::property::{check, op_sequence, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::busy_class::classify_busy;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;

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

/// 現在の Step の状態を書き換えた写し。
fn with_status(status: StepStatus) -> SessionState {
    let mut s = entered();
    for slice in s.run.slices.values_mut() {
        for v in slice.status.values_mut() {
            *v = status;
        }
    }
    s
}

fn ui(view: &str, path: &str) -> Input {
    Input::Ui {
        view: view.into(),
        path: path.into(),
    }
}

fn approval(decision: Decision, target: Option<&str>) -> Input {
    Input::Approval {
        decision,
        target: target.map(str::to_string),
    }
}

fn first_note() -> Input {
    Input::Command("/make-note first-note".into())
}

fn is_input_received_only(events: &[Event]) -> bool {
    matches!(events, [Event::InputReceived { .. }])
}

fn accepted_count(state: &SessionState) -> usize {
    state
        .log
        .iter()
        .filter(|e| matches!(e, Event::Accepted { .. }))
        .count()
}

/// 1 回送り、送信の直前の分類が `busy` だったか・戻りのイベントを返す。
fn send(session: &mut Session, input: &Input) -> (bool, Vec<Event>) {
    let busy = classify_busy(&session.state().run, input);
    if let Some(b) = &busy {
        assert_eq!(b.category(), RejectCategory::Busy);
    }
    let events = session.apply(Op::Input(input.clone())).to_vec();
    (busy.is_some(), events)
}

/// 連打した結果。
struct Burst {
    session: Session,
    busy_flags: Vec<bool>,
    after_first: SessionState,
    results: Vec<Vec<Event>>,
}

fn burst(input: &Input, n: usize) -> Burst {
    let mut session = Session::from_state(base_ir(), entered());
    let mut busy_flags = Vec::new();
    let mut results = Vec::new();
    let mut after_first = session.state().clone();
    for i in 0..n {
        let (b, ev) = send(&mut session, input);
        busy_flags.push(b);
        results.push(ev);
        if i == 0 {
            after_first = session.state().clone();
        }
    }
    Burst {
        session,
        busy_flags,
        after_first,
        results,
    }
}

#[test]
fn burst_of_20_accepts_once_and_rejects_19_as_busy() {
    let b = burst(&first_note(), 20);
    let state = b.session.state();
    assert_eq!(accepted_count(state), 1);
    assert!(!b.busy_flags[0], "first send is not busy");
    assert_eq!(b.busy_flags.iter().filter(|f| **f).count(), 19);
    assert!(b.busy_flags[1..].iter().all(|f| *f));
    assert!(b.results[0]
        .iter()
        .any(|e| matches!(e, Event::Accepted { .. })));
    for r in &b.results[1..] {
        assert!(is_input_received_only(r), "{r:?}");
    }
    assert_eq!(b.after_first.run.busy, Some(Busy { done: 0, total: 5 }));
    assert_eq!(state.run, b.after_first.run);
    assert_eq!(state.log.len(), b.after_first.log.len() + 19);
    assert_eq!(state.run.generation, Generation(1));
    assert_eq!(b.after_first.run.generation, Generation(1));
}

#[test]
fn burst_is_deterministic() {
    let a = burst(&first_note(), 20);
    let b = burst(&first_note(), 20);
    assert_eq!(a.session.state(), b.session.state());
    assert_eq!(a.results, b.results);
}

#[test]
fn busy_applies_to_every_input_kind() {
    let inputs = [
        first_note(),
        Input::Command("/make-note Bad_Name".into()),
        Input::Command("/nothing".into()),
        Input::Choice("opt-memo".into()),
        ui("v-files", "notes/draft"),
        ui("v-other", "notes/other"),
        approval(Decision::Approve, None),
        approval(Decision::Reject, None),
        Input::Text("文章".into()),
    ];
    let mut session = Session::from_state(base_ir(), entered());
    send(&mut session, &first_note());
    assert!(session.state().run.busy.is_some());
    for input in &inputs {
        let before = session.state().clone();
        let (busy, events) = send(&mut session, input);
        assert!(busy, "{input:?}");
        assert!(is_input_received_only(&events), "{input:?}: {events:?}");
        assert_eq!(session.state().run, before.run, "{input:?}");
        assert_eq!(session.state().log.len(), before.log.len() + 1);
    }
}

#[test]
fn cancel_is_never_busy() {
    let mut session = Session::from_state(base_ir(), entered());
    send(&mut session, &first_note());
    let before = session.state().run.clone();
    let (busy, events) = send(&mut session, &Input::Cancel);
    assert!(!busy);
    assert!(is_input_received_only(&events));
    assert_eq!(session.state().run, before);
}

#[test]
fn not_busy_when_idle_or_awaiting_confirm() {
    let input = first_note();
    assert!(classify_busy(&entered().run, &input).is_none());
    assert!(classify_busy(&with_status(StepStatus::Accepted).run, &input).is_none());
    assert!(classify_busy(&with_status(StepStatus::Achieved).run, &input).is_none());
    // 現在の Step が無ければ、処理中でも判定しない。
    let mut none = entered();
    none.run.current = None;
    none.run.busy = Some(Busy { done: 0, total: 5 });
    assert!(classify_busy(&none.run, &input).is_none());
}

#[test]
fn busy_rejection_carries_progress() {
    let mut s = entered();
    s.run.busy = Some(Busy { done: 2, total: 5 });
    let r = classify_busy(&s.run, &first_note()).expect("busy");
    assert_eq!(r.busy, Busy { done: 2, total: 5 });
    assert_eq!(r.category(), RejectCategory::Busy);
}

#[derive(Debug, Clone)]
enum TestOp {
    Apply(Input),
    /// 進行の照合（TASK-210）の代わりに処理中を畳む擬似操作。
    FinishProcessing,
}

fn input_strategy() -> impl Strategy<Value = Input> {
    prop_oneof![
        4 => Just(first_note()),
        1 => Just(Input::Command("/make-note Bad_Name".into())),
        1 => Just(Input::Command("/nothing".into())),
        2 => Just(Input::Choice("opt-memo".into())),
        3 => Just(ui("v-files", "notes/draft")),
        1 => Just(ui("v-files", "notes/other")),
        3 => Just(approval(Decision::Approve, None)),
        1 => Just(approval(Decision::Reject, None)),
        1 => Just(Input::Text("文章".into())),
        2 => Just(Input::Cancel),
    ]
}

fn op_strategy() -> impl Strategy<Value = TestOp> {
    prop_oneof![
        10 => input_strategy().prop_map(TestOp::Apply),
        2 => Just(TestOp::FinishProcessing),
    ]
}

#[test]
fn p6_burst_accepts_once() {
    check("p6-burst-accepts-once", DEFAULT_CASES, 2..=MAX_OPS, |n| {
        let b = burst(&first_note(), n);
        let state = b.session.state();
        prop_assert_eq!(accepted_count(state), 1);
        prop_assert_eq!(b.busy_flags.iter().filter(|f| **f).count(), n - 1);
        prop_assert_eq!(&state.run, &b.after_first.run);
        prop_assert_eq!(state.run.generation, Generation(1));
        Ok(())
    });
}

#[test]
fn p6_random_inputs_keep_invariants() {
    check(
        "p6-random-inputs-keep-invariants",
        DEFAULT_CASES,
        op_sequence(op_strategy()),
        |ops| {
            let mut session = Session::from_state(base_ir(), entered());
            for (n, op) in ops.iter().enumerate() {
                match op {
                    TestOp::FinishProcessing => {
                        let ir = session.ir().clone();
                        let mut state = session.into_state();
                        state.run.busy = None;
                        session = Session::from_state(ir, state);
                    }
                    TestOp::Apply(input) => {
                        let before = session.state().clone();
                        let (busy, events) = send(&mut session, input);
                        let after = session.state();
                        let expect = before.run.busy.is_some()
                            && before.run.current.is_some()
                            && !matches!(input, Input::Cancel);
                        // I3: 分類は処理中かつ現在の Step があり、取り消しでないときだけ。
                        prop_assert_eq!(busy, expect, "I3 op#{}", n);
                        if busy {
                            // I2: 処理中の拒否は状態を変えず、ログは 1 件だけ増える。
                            prop_assert!(is_input_received_only(&events), "I2 op#{}", n);
                            prop_assert_eq!(&after.run, &before.run, "I2 op#{}", n);
                            prop_assert_eq!(after.log.len(), before.log.len() + 1);
                        }
                    }
                }
                // I4: 世代は列の全体で変わらない。
                prop_assert_eq!(session.state().run.generation, Generation(1), "I4 op#{}", n);
            }
            // I1: 受付は（世代, Step）につき高々 1 件。
            let mut seen = BTreeSet::new();
            for e in &session.state().log {
                if let Event::Accepted { at } = e {
                    prop_assert!(
                        seen.insert((at.generation, at.step.0.clone())),
                        "I1 duplicate accept"
                    );
                }
            }
            Ok(())
        },
    );
}
