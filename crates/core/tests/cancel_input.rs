//! 処理中でない Esc（`Control::Stop`）の取り消しの入力としての扱いの試験（TASK-212、仕様 EVT-10）。
//!
//! 処理中でない停止は世代を変えず、`Input::Cancel` と同じ結果になる。取り消しの規則（`on.cancel`）が無い Step は
//! `not_in_this_step` の拒否、受付の規則がある Step はその規則の意味に従う。呼び出し元は CI の `make test` のみである。

mod common;

use common::contract_sample::{load, testdata_dir};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use serde_json::{json, Value};

const ST1: &str = "main/st-1";

fn base_ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

/// st-1 の先頭に、取り消しの規則を足した教材。
fn ir_with_rule(rule: Value) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == ST1 {
            n.step.rules.insert(0, rule.clone());
        }
    }
    ir
}

fn accept_rule() -> Value {
    json!({
        "id": "r-cancel",
        "on": { "cancel": true },
        "outcome": { "kind": "accept", "effects": [
            { "set_attr": { "path": "notes/draft", "attr": "state", "value": "cancelled" } }
        ] }
    })
}

fn stop() -> Op {
    Op::Control(Control::Stop)
}

fn cancel() -> Op {
    Op::Input(Input::Cancel)
}

fn initial() -> SessionState {
    load().initial
}

fn set_status(state: &mut SessionState, status: StepStatus) {
    for sl in state.run.slices.values_mut() {
        sl.status.insert(StepUid(ST1.into()), status);
    }
}

/// 世界状態に `notes/draft` があり、処理中でなく、st-1 が指定の段階の状態。
fn state_with_draft(status: StepStatus) -> SessionState {
    let mut state = apply(
        &base_ir(),
        initial(),
        Op::Input(Input::Command("/make-note first-note".into())),
    )
    .0;
    state.run.busy = None;
    set_status(&mut state, status);
    state
}

fn assert_rejected_only(before: &SessionState, next: &SessionState, events: &[Event]) {
    match events {
        [Event::InputReceived {
            input: Input::Cancel,
            ..
        }, Event::Judged {
            judgment: Judgment::Rejected(r),
            ..
        }] => {
            assert_eq!(r.category, RejectCategory::NotInThisStep);
        }
        other => panic!("取り消しの拒否だけのはず: {other:?}"),
    }
    assert_eq!(next.run, before.run);
    assert_eq!(next.run.generation, before.run.generation);
    assert_eq!(next.log.len(), before.log.len() + 2);
}

#[test]
fn idle_stop_without_cancel_rule_is_rejected_in_entered_step() {
    let ir = base_ir();
    let before = initial();
    let (next, events) = apply(&ir, before.clone(), stop());
    assert_rejected_only(&before, &next, &events);
    assert_eq!(next.run.generation, Generation(1));
}

#[test]
fn idle_stop_without_cancel_rule_is_rejected_in_accepted_step() {
    let ir = base_ir();
    let before = state_with_draft(StepStatus::Accepted);
    let (next, events) = apply(&ir, before.clone(), stop());
    assert_rejected_only(&before, &next, &events);
}

#[test]
fn idle_stop_with_accept_rule_accepts_and_applies_effects() {
    let ir = ir_with_rule(accept_rule());
    let before = state_with_draft(StepStatus::Entered);
    let (next, events) = apply(&ir, before.clone(), stop());
    assert!(
        matches!(
            events.as_slice(),
            [
                Event::InputReceived {
                    input: Input::Cancel,
                    ..
                },
                Event::Judged {
                    judgment: Judgment::Accepted(_),
                    ..
                },
                Event::Accepted { .. },
                ..
            ]
        ),
        "{events:?}"
    );
    let status = next
        .run
        .slices
        .values()
        .find_map(|s| s.status.get(&StepUid(ST1.into())))
        .copied();
    assert_eq!(status, Some(StepStatus::Accepted));
    assert_eq!(
        next.run
            .world
            .get("notes/draft")
            .and_then(|e| e.attrs.get("state"))
            .map(String::as_str),
        Some("cancelled")
    );
    assert_eq!(next.run.generation, before.run.generation);
    let causes: Vec<Cause> = events
        .iter()
        .filter_map(|e| match e {
            Event::Changed { cause, .. } => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(causes, [Cause::Input], "{events:?}");
}

#[test]
fn idle_stop_with_accept_rule_in_accepted_step_warns_only() {
    let ir = ir_with_rule(accept_rule());
    let before = state_with_draft(StepStatus::Accepted);
    let (next, events) = apply(&ir, before.clone(), stop());
    assert!(
        matches!(
            events.as_slice(),
            [Event::InputReceived { .. }, Event::Warning { message, .. }]
                if !message.contains("r-cancel")
        ),
        "{events:?}"
    );
    assert_eq!(next.run, before.run);
}

#[test]
fn idle_stop_with_reject_rule_judges_only() {
    let rule = json!({
        "id": "r-cancel",
        "on": { "cancel": true },
        "outcome": { "kind": "reject", "rejection": "rj-decline" }
    });
    let ir = ir_with_rule(rule);
    let before = initial();
    let (next, events) = apply(&ir, before.clone(), stop());
    assert!(
        matches!(
            events.as_slice(),
            [
                Event::InputReceived { .. },
                Event::Judged {
                    judgment: Judgment::Rejected(_),
                    ..
                }
            ]
        ),
        "{events:?}"
    );
    assert_eq!(next.run, before.run);
}

#[test]
fn idle_stop_equals_cancel_input() {
    for ir in [base_ir(), ir_with_rule(accept_rule())] {
        let before = state_with_draft(StepStatus::Entered);
        let (a, ea) = apply(&ir, before.clone(), stop());
        let (b, eb) = apply(&ir, before, cancel());
        assert_eq!(ea.len(), eb.len());
        assert_eq!(a.run, b.run);
    }
}

#[test]
fn idle_stop_is_deterministic() {
    let ir = ir_with_rule(accept_rule());
    let first = apply(&ir, state_with_draft(StepStatus::Entered), stop());
    let second = apply(&ir, state_with_draft(StepStatus::Entered), stop());
    assert_eq!(first, second);
}

#[test]
fn stop_without_current_step_is_empty() {
    let ir = base_ir();
    let mut state = initial();
    state.run.current = None;
    let (next, events) = apply(&ir, state.clone(), stop());
    assert!(events.is_empty());
    assert_eq!(next, state);
}
