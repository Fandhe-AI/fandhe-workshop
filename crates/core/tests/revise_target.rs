//! 承認の「直す」の対象つき入力（TASK-264、JUDGE-10）の試験。
//!
//! 試験データ `tests/data/revise_target_1/ir.json` は、testdata の ST-2.6（`r-revise-expect`・`r-revise-other`・
//! `r-approve-early`）と ST-2.3（`r-hold`）の形に合わせた手書きの写しで、対象が 2 つの Step・矛盾した教材・
//! `on_reject` の上書き・壊れた文面を足してある。確かめるのは次の 3 点である。
//! - 規則に書かれた対象ごとに、対象つきの「直す」が受付になる
//! - 対象なしの「直す」は、対象つきの規則がある Step では拒否のままになる
//! - その拒否に、空でない次の行動が付く

mod common;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_core::rejection_text::{
    rejection_of, resolve_rejection, RejectionSource, RejectionTextError,
};
use fandhe_workshop_core::select_judge::{
    judge_select, revise_targets, SelectJudgeError, SelectVerdict,
};
use proptest::prelude::*;
use serde_json::{json, Value};

fn ir() -> Ir {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/revise_target_1/ir.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn step_of(ir: &Ir, id: &str) -> Step {
    ir.nodes
        .iter()
        .find(|n| n.step.id == id)
        .unwrap_or_else(|| panic!("{id}"))
        .step
        .clone()
}

fn never(_: &Value) -> Option<bool> {
    None
}

fn appr(decision: Decision, target: Option<&str>) -> Input {
    Input::Approval {
        decision,
        target: target.map(str::to_string),
    }
}

fn revise(target: Option<&str>) -> Input {
    appr(Decision::Revise, target)
}

fn step_with_rules(rules: Vec<Value>) -> Step {
    let mut s = step_of(&ir(), "st-26");
    s.rules = rules;
    s
}

#[test]
fn revise_targets_lists_in_order_without_duplicates() {
    let ir = ir();
    assert_eq!(
        revise_targets(&step_of(&ir, "st-two")),
        Ok(vec!["expect".to_string(), "reason".to_string()])
    );
    assert_eq!(
        revise_targets(&step_of(&ir, "st-26")),
        Ok(vec!["expect".to_string()])
    );
}

#[test]
fn revise_targets_ignores_bare_and_other_decisions() {
    let ir = ir();
    assert_eq!(revise_targets(&step_of(&ir, "st-23")), Ok(vec![]));
    let s = step_with_rules(vec![
        json!({ "id": "r", "on": { "approval": "approve", "target": "x" }, "outcome": { "kind": "accept" } }),
    ]);
    assert_eq!(revise_targets(&s), Ok(vec![]));
    let mut none = step_of(&ir, "st-26");
    none.rules.clear();
    assert_eq!(revise_targets(&none), Ok(vec![]));
}

#[test]
fn revise_targets_rejects_invalid_declarations() {
    for rule in [
        json!("not an object"),
        json!({ "id": "r" }),
        json!({ "id": "r", "on": { "approval": "revise", "target": 1 } }),
    ] {
        assert_eq!(
            revise_targets(&step_with_rules(vec![rule])),
            Err(SelectJudgeError::InvalidDeclaration)
        );
    }
}

#[test]
fn revise_targets_reads_compiled_wide_rules_with_null_target() {
    // コンパイル済みの教材の `on` は全キーを `null` 込みで持つ。対象なしの「直す」の `target: null` は欠落と同じ。
    let s = step_with_rules(vec![
        json!({ "id": "r-bare", "on": { "approval": "revise", "target": null, "text": null, "cancel": null },
                "outcome": { "kind": "reject" } }),
        json!({ "id": "r-empty", "on": { "approval": "revise", "target": [] }, "outcome": { "kind": "reject" } }),
        json!({ "id": "r-t", "on": { "approval": "revise", "target": "expect", "text": null },
                "outcome": { "kind": "accept" } }),
    ]);
    assert_eq!(revise_targets(&s), Ok(vec!["expect".to_string()]));
}

#[test]
fn each_declared_target_is_accepted() {
    let ir = ir();
    let s = step_of(&ir, "st-two");
    for (target, id) in [("expect", "r-expect"), ("reason", "r-reason")] {
        match judge_select(&s, &revise(Some(target)), &never) {
            Ok(SelectVerdict::Accepted { rule_id, .. }) => assert_eq!(rule_id, id),
            other => panic!("{target}: {other:?}"),
        }
    }
}

#[test]
fn bare_revise_in_st_2_6_is_rejected_by_rule_with_next_action() {
    let ir = ir();
    let s = step_of(&ir, "st-26");
    let v = judge_select(&s, &revise(None), &never).unwrap();
    assert!(matches!(
        &v,
        SelectVerdict::RejectedByRule { rule_id, .. } if rule_id == "r-revise-other"
    ));
    let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
    assert_eq!(r.category, RejectCategory::NotInThisStep);
    assert_eq!(r.text, "rj-2-6-wrong-target");
    assert!(!r.next_action.as_str().trim().is_empty());
}

#[test]
fn bare_revise_without_bare_reject_rule_falls_back_to_category() {
    let ir = ir();
    let s = step_of(&ir, "st-two");
    let v = judge_select(&s, &revise(None), &never).unwrap();
    assert_eq!(v, SelectVerdict::Rejected(RejectCategory::NotInThisStep));
    let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
    assert_eq!(r.text, "rj-default-not-in-this-step");
    assert!(!r.next_action.as_str().trim().is_empty());
}

#[test]
fn on_reject_overrides_default_text() {
    let ir = ir();
    let s = step_of(&ir, "st-override");
    let v = judge_select(&s, &revise(None), &never).unwrap();
    assert_eq!(v, SelectVerdict::Rejected(RejectCategory::NotInThisStep));
    let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
    assert_eq!(r.text, "rj-override");
}

#[test]
fn contradictory_pack_never_accepts_bare_revise() {
    let ir = ir();
    let s = step_of(&ir, "st-mixed");
    assert_eq!(
        judge_select(&s, &revise(None), &|_| Some(true)),
        Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep))
    );
    assert!(matches!(
        judge_select(&s, &revise(Some("expect")), &never),
        Ok(SelectVerdict::Accepted { .. })
    ));
}

#[test]
fn bare_revise_without_targeted_rules_keeps_accepting() {
    let ir = ir();
    let s = step_of(&ir, "st-23");
    assert!(matches!(
        judge_select(&s, &revise(None), &never),
        Ok(SelectVerdict::Accepted { rule_id, .. }) if rule_id == "r-hold"
    ));
}

#[test]
fn undeclared_target_is_rejected_with_next_action() {
    let ir = ir();
    for name in ["st-26", "st-two", "st-23"] {
        let s = step_of(&ir, name);
        let v = judge_select(&s, &revise(Some("other")), &never).unwrap();
        assert_eq!(v, SelectVerdict::Rejected(RejectCategory::NotInThisStep));
        let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
        assert!(!r.next_action.as_str().trim().is_empty());
    }
}

#[test]
fn unreadable_rule_after_match_still_fails_closed_for_bare_revise() {
    let mut s = step_of(&ir(), "st-two");
    s.rules.push(json!("broken"));
    assert_eq!(
        judge_select(&s, &revise(None), &never),
        Err(SelectJudgeError::InvalidDeclaration)
    );
}

#[test]
fn resolution_failures_are_errors_not_empty_actions() {
    let ir = ir();
    let s = step_of(&ir, "st-26");
    let cat = RejectionSource::Category(RejectCategory::NotInThisStep);

    let mut no_defaults = ir.clone();
    no_defaults.defaults.clear();
    assert_eq!(
        resolve_rejection(&no_defaults, &s, cat),
        Err(RejectionTextError::MissingText)
    );
    assert_eq!(
        resolve_rejection(&ir, &s, RejectionSource::Rule("ghost")),
        Err(RejectionTextError::MissingText)
    );
    assert_eq!(
        resolve_rejection(&ir, &s, RejectionSource::Rule("rj-blank")),
        Err(RejectionTextError::EmptyNextAction)
    );
    assert_eq!(
        resolve_rejection(&ir, &step_of(&ir, "st-broken-text"), cat),
        Err(RejectionTextError::EmptyNextAction)
    );
    assert_eq!(
        resolve_rejection(&ir, &s, RejectionSource::Rule("rj-free")),
        Err(RejectionTextError::InvalidText)
    );
    assert_eq!(
        resolve_rejection(&ir, &s, RejectionSource::Rule("rj-no-next")),
        Err(RejectionTextError::InvalidText)
    );
    let mut not_object = ir.clone();
    not_object
        .rejections
        .insert("rj-early".into(), json!("text"));
    assert_eq!(
        resolve_rejection(&not_object, &s, RejectionSource::Rule("rj-early")),
        Err(RejectionTextError::InvalidText)
    );
}

#[test]
fn accepted_and_clarified_have_no_rejection() {
    let ir = ir();
    let s = step_of(&ir, "st-two");
    let v = judge_select(&s, &revise(Some("expect")), &never).unwrap();
    assert_eq!(rejection_of(&ir, &s, &v), Ok(None));
    let c = SelectVerdict::Clarified {
        rule_index: 0,
        rule_id: "r".into(),
        clarification: "cl".into(),
    };
    assert_eq!(rejection_of(&ir, &s, &c), Ok(None));
}

#[test]
fn error_display_embeds_no_values() {
    for e in [
        RejectionTextError::MissingText,
        RejectionTextError::InvalidText,
        RejectionTextError::EmptyNextAction,
    ] {
        let text = e.to_string();
        assert!(!text.contains("rj-") && !text.contains("expect"), "{text}");
    }
}

fn approval_strategy() -> impl Strategy<Value = Input> {
    (
        prop_oneof![
            Just(Decision::Approve),
            Just(Decision::Reject),
            Just(Decision::Revise)
        ],
        prop_oneof![
            Just(None),
            Just(Some("expect")),
            Just(Some("reason")),
            Just(Some("other")),
            Just(Some("")),
        ],
    )
        .prop_map(|(d, t)| appr(d, t))
}

#[test]
fn targeted_steps_never_accept_bare_revise_and_always_give_next_action() {
    let ir = ir();
    for name in ["st-26", "st-two", "st-mixed", "st-override"] {
        let s = step_of(&ir, name);
        check(
            "revise_target_bare_revise",
            DEFAULT_CASES,
            approval_strategy(),
            |input| {
                let a = judge_select(&s, &input, &|_| Some(true));
                prop_assert_eq!(&a, &judge_select(&s, &input, &|_| Some(true)));
                let v = a.unwrap();
                if input == revise(None) {
                    let accepted_or_clarified = matches!(
                        v,
                        SelectVerdict::Accepted { .. } | SelectVerdict::Clarified { .. }
                    );
                    prop_assert!(!accepted_or_clarified);
                }
                if let Some(r) = rejection_of(&ir, &s, &v).unwrap() {
                    prop_assert!(!r.next_action.as_str().trim().is_empty());
                }
                Ok(())
            },
        );
    }
}

#[test]
fn source_has_no_external_calls() {
    for file in ["rejection_text.rs", "select_judge.rs"] {
        let path = format!("{}/src/{file}", env!("CARGO_MANIFEST_DIR"));
        let src = std::fs::read_to_string(path).unwrap();
        let code: String = src
            .lines()
            .filter(|l| !l.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n");
        for banned in [
            "std::net",
            "std::fs",
            "std::process",
            "std::time",
            "std::env",
            "web_sys",
            "js_sys",
            "extern crate",
        ] {
            assert!(!code.contains(banned), "{file}: {banned}");
        }
    }
}
