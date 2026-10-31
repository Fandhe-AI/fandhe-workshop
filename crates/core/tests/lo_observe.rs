//! 履歴から確認ごとの最初の回答と開いたヒントの段を導く写像 `lo_observe` の試験（TASK-304 1/3、GUIDE-11）。
//!
//! 基の教材は `min-3step.json` で、st-2 を `answer` の確認にして使う。観測の規則（段階・入力の種類・上限での戻り）と、
//! `observe_history` が操作を 1 件ずつ流した結果と一致することを確かめる。呼び出し元は CI の `make test` のみ。

use std::path::PathBuf;

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::lo_check::{
    CheckMethod, CheckObservation, CheckTarget, FirstAnswer, Reselection,
};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::lo_observe::{
    answer_phase, observe_history, observe_op, records_from_history,
};
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

const ST2: &str = "main/st-2";

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn ir_with_st2(patch: Value) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == ST2 {
            let mut v = serde_json::to_value(&n.step).expect("to value");
            for (k, val) in patch.as_object().expect("object") {
                v[k] = val.clone();
            }
            n.step = serde_json::from_value::<Step>(v).expect("from value");
        }
    }
    ir
}

fn answer_patch() -> Value {
    json!({
        "hints": ["h-1-1", "h-1-2", "h-1-3"],
        "confirm": {
            "kind": "answer",
            "on_wrong": "rj-decline",
            "options": [
                { "id": "opt-memo", "correct": true },
                { "id": "opt-todo", "correct": false }
            ]
        }
    })
}

fn answer_ir() -> Ir {
    ir_with_st2(answer_patch())
}

/// st-2 を、st-3 と同じ承認の入力・規則にして確認を approve にした教材。
fn approve_ir() -> Ir {
    let ir = base_ir();
    let st3 = ir
        .nodes
        .iter()
        .find(|n| n.uid == "main/st-3")
        .expect("st-3");
    let v = serde_json::to_value(&st3.step).expect("to value");
    ir_with_st2(json!({
        "input": v["input"].clone(),
        "rules": v["rules"].clone(),
        "confirm": { "kind": "approve" }
    }))
}

fn entered_state() -> SessionState {
    let u = StepUid(ST2.to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn choice(id: &str) -> Op {
    Op::Input(Input::Choice(id.into()))
}

fn approval(decision: Decision) -> Op {
    Op::Input(Input::Approval {
        decision,
        target: None,
    })
}

/// 1 件流して、その操作の観測を返す。
fn step(s: &mut Session, op: Op) -> Vec<CheckObservation> {
    let p = answer_phase(s.ir(), s.state(), &op);
    let ev = s.apply(op).to_vec();
    observe_op(p, &ev)
}

fn answered(correct: bool) -> CheckObservation {
    CheckObservation::Answered {
        step: ST2.into(),
        correct,
    }
}

fn hint(opened: u32) -> CheckObservation {
    CheckObservation::HintOpened {
        step: ST2.into(),
        opened,
    }
}

fn targets(method: CheckMethod) -> Vec<CheckTarget> {
    vec![CheckTarget {
        step: ST2.into(),
        lo: vec!["LO-2".into()],
        method,
    }]
}

/// 受付済み（確認の段階）の st-2。
fn confirming(ir: Ir) -> Session {
    let mut s = Session::from_state(ir, entered_state());
    assert!(step(&mut s, choice("opt-memo")).is_empty());
    s
}

fn engine(op: Op) -> SessionOp {
    HistoryOp::Engine(op)
}

#[test]
fn hints_then_correct_first_answer() {
    let mut s = confirming(answer_ir());
    let mut obs = Vec::new();
    obs.extend(step(&mut s, Op::Control(Control::OpenHint)));
    obs.extend(step(&mut s, Op::Control(Control::OpenHint)));
    obs.extend(step(&mut s, choice("opt-memo")));
    assert_eq!(obs, vec![hint(1), hint(2), answered(true)]);
}

#[test]
fn max_hint_reaches_three_in_record() {
    let ir = answer_ir();
    let ops: Vec<SessionOp> = vec![
        engine(choice("opt-memo")),
        engine(Op::Control(Control::OpenHint)),
        engine(Op::Control(Control::OpenHint)),
        engine(Op::Control(Control::OpenHint)),
        engine(choice("opt-memo")),
    ];
    let r = records_from_history(&ir, entered_state(), &ops, &targets(CheckMethod::Answer))
        .expect("replay");
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].max_hint, 3);
    assert_eq!(r[0].first_answer, FirstAnswer::Correct);
}

#[test]
fn wrong_then_reselect_is_recorded_without_comparison() {
    let ir = answer_ir();
    let ops: Vec<SessionOp> = vec![
        engine(choice("opt-memo")),
        engine(choice("opt-todo")),
        engine(choice("opt-memo")),
    ];
    let r = records_from_history(&ir, entered_state(), &ops, &targets(CheckMethod::Answer))
        .expect("replay");
    assert_eq!(r[0].first_answer, FirstAnswer::Wrong);
    assert_eq!(r[0].reselection, Reselection::Correct);
    assert!(!r[0].after_comparison);
}

#[test]
fn wrong_only_is_not_correct() {
    let ir = answer_ir();
    let ops: Vec<SessionOp> = vec![engine(choice("opt-memo")), engine(choice("opt-todo"))];
    let r = records_from_history(&ir, entered_state(), &ops, &targets(CheckMethod::Answer))
        .expect("replay");
    assert_eq!(r[0].first_answer, FirstAnswer::Wrong);
    assert_eq!(r[0].reselection, Reselection::NotCorrect);
}

#[test]
fn entering_stage_choice_is_not_an_answer() {
    let mut s = Session::from_state(answer_ir(), entered_state());
    // 受付の段階の選択は、受け付けられても拒否されても回答に数えない。
    assert!(step(&mut s, choice("unknown-option")).is_empty());
    assert!(step(&mut s, choice("opt-memo")).is_empty());
}

#[test]
fn non_matching_inputs_in_confirm_stage_are_not_answers() {
    let mut s = confirming(answer_ir());
    assert!(step(&mut s, Op::Input(Input::Command("make-note x".into()))).is_empty());
    assert!(step(&mut s, Op::Input(Input::Text("hello".into()))).is_empty());
    assert!(step(&mut s, Op::Input(Input::Cancel)).is_empty());
    // 方式に合う選択は誤答として数える。未知の選択肢の ID も同じ。
    assert_eq!(step(&mut s, choice("nope")), vec![answered(false)]);
}

#[test]
fn approve_step_counts_approve_but_not_revise() {
    let mut s = Session::from_state(approve_ir(), entered_state());
    assert!(step(&mut s, approval(Decision::Approve)).is_empty());
    assert!(step(&mut s, approval(Decision::Revise)).is_empty());
    assert_eq!(
        step(&mut s, approval(Decision::Approve)),
        vec![answered(true)]
    );
}

#[test]
fn restore_decides_phase_from_real_state() {
    let mut s = confirming(answer_ir());
    let id = s.state().next_checkpoint;
    step(&mut s, Op::Control(Control::Save));
    assert_eq!(step(&mut s, choice("opt-memo")), vec![answered(true)]);
    step(&mut s, Op::Control(Control::Restore(id)));
    // 復帰後の st-2 は確認の段階に戻っているので、誤答が回答になる。
    assert_eq!(step(&mut s, choice("opt-todo")), vec![answered(false)]);
}

#[test]
fn unavailable_hint_makes_no_observation() {
    let mut s = Session::from_state(ir_with_st2(json!({ "hints": [] })), entered_state());
    assert!(step(&mut s, Op::Control(Control::OpenHint)).is_empty());
}

#[test]
fn confirmed_judgment_always_comes_with_achieved() {
    let mut s = confirming(answer_ir());
    for id in ["opt-todo", "opt-memo"] {
        let before = s.log().len();
        s.apply(choice(id));
        let ev = &s.log()[before..];
        let confirmed = ev.iter().any(|e| {
            matches!(e, Event::Judged { judgment, .. } if *judgment == fandhe_workshop_contract::judgment::Judgment::Confirmed)
        });
        let achieved = ev.iter().any(|e| matches!(e, Event::Achieved { .. }));
        assert_eq!(confirmed, achieved, "{id}");
    }
}

#[test]
fn busy_step_input_is_not_an_answer() {
    let mut st = entered_state();
    let u = StepUid(ST2.to_string());
    st.run
        .slices
        .get_mut(&u.root())
        .expect("slice")
        .status
        .insert(u, StepStatus::Accepted);
    let ir = answer_ir();
    assert!(answer_phase(&ir, &st, &choice("opt-memo")).is_some());
    st.run.busy = Some(Busy { done: 0, total: 2 });
    assert!(answer_phase(&ir, &st, &choice("opt-memo")).is_none());
}

#[test]
fn observe_history_matches_stepwise_session() {
    let ir = answer_ir();
    let ops_raw = vec![
        choice("opt-memo"),
        Op::Control(Control::OpenHint),
        choice("opt-todo"),
        Op::Control(Control::OpenHint),
        choice("opt-memo"),
        choice("opt-todo"),
    ];
    let mut s = Session::from_state(ir.clone(), entered_state());
    let mut expected = Vec::new();
    for op in ops_raw.clone() {
        expected.extend(step(&mut s, op));
    }
    let ops: Vec<SessionOp> = ops_raw.into_iter().map(engine).collect();
    let got = observe_history(&ir, entered_state(), &ops).expect("replay");
    assert_eq!(got, expected);
    assert!(!got.is_empty());
}
