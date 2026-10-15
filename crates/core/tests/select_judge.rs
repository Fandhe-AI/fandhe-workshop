//! 選択肢・画面操作・承認の判定（TASK-263、JUDGE-1）の試験。
//!
//! 試験データ `tests/data/select_judge_1/steps.json` の Step は、pack の golden
//! （`min-3step-pack.ir.json` の `st-2`・`st-3`、`ch1-pack.ir.json` の `st-1-6`）の形に合わせた手書きの写しで、
//! 否定ケース（宣言に無い ID を指す規則・余分なキーの規則など）を足してある。

mod common;

use std::collections::BTreeMap;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{JudgmentKind, RejectCategory};
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_core::select_judge::{
    judge_confirm, judge_select, ConfirmVerdict, SelectJudgeError, SelectVerdict,
};
use proptest::prelude::*;
use serde_json::Value;

fn steps() -> BTreeMap<String, Step> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/select_judge_1/steps.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn step(name: &str) -> Step {
    steps().remove(name).unwrap_or_else(|| panic!("{name}"))
}

fn never(_: &Value) -> Option<bool> {
    None
}

fn sel(name: &str, input: &Input) -> Result<SelectVerdict, SelectJudgeError> {
    judge_select(&step(name), input, &never)
}

fn conf(name: &str, input: &Input) -> Result<ConfirmVerdict, SelectJudgeError> {
    judge_confirm(&step(name), input)
}

fn choice(id: &str) -> Input {
    Input::Choice(id.to_string())
}

fn ui(view: &str, path: &str) -> Input {
    Input::Ui {
        view: view.to_string(),
        path: path.to_string(),
    }
}

fn appr(decision: Decision, target: Option<&str>) -> Input {
    Input::Approval {
        decision,
        target: target.map(str::to_string),
    }
}

fn assert_accepted(got: Result<SelectVerdict, SelectJudgeError>, id: &str) {
    match got {
        Ok(SelectVerdict::Accepted { rule_id, .. }) => assert_eq!(rule_id, id),
        other => panic!("expected accepted {id}, got {other:?}"),
    }
}

fn not_in_step() -> SelectVerdict {
    SelectVerdict::Rejected(RejectCategory::NotInThisStep)
}

fn confirm_rejected() -> Result<ConfirmVerdict, SelectJudgeError> {
    Ok(ConfirmVerdict::Rejected(RejectCategory::NotInThisStep))
}

#[test]
fn choice_accepts_declared_option() {
    assert_accepted(sel("choice", &choice("opt-memo")), "r-memo");
}

#[test]
fn choice_rejects_undeclared_option() {
    // `ghost` を指す受付の規則があっても、宣言に無い値は受付にならない。
    assert_eq!(sel("choice", &choice("ghost")), Ok(not_in_step()));
}

#[test]
fn choice_rule_outcomes_map_to_kinds() {
    let r = sel("choice", &choice("opt-todo")).unwrap();
    assert!(
        matches!(&r, SelectVerdict::RejectedByRule { rejection, .. } if rejection == "rj-todo")
    );
    assert_eq!(r.kind(), JudgmentKind::Rejected);
    let c = sel("choice", &choice("opt-ask")).unwrap();
    assert!(
        matches!(&c, SelectVerdict::Clarified { clarification, .. } if clarification == "cl-ask")
    );
    assert_eq!(c.kind(), JudgmentKind::Clarified);
}

#[test]
fn rule_with_extra_key_does_not_match() {
    // `r-extra` は `{option, approval}` で、先に書かれていても一致しない（次の規則が採られる）。
    let r = sel("choice", &choice("opt-todo")).unwrap();
    assert!(matches!(r, SelectVerdict::RejectedByRule { ref rule_id, .. } if rule_id == "r-todo"));
    // `r-b-extra` は `{ui, option}` で、宣言にある `notes/b` でも受付にならない。
    assert_eq!(sel("ui", &ui("tree", "notes/b")), Ok(not_in_step()));
}

#[test]
fn ui_action_accepts_declared_target() {
    assert_accepted(sel("ui", &ui("tree", "notes/a")), "r-a");
}

#[test]
fn ui_action_rejects_other_path() {
    // view は一致し path が違う。`notes/c` を指す受付の規則があっても宣言に無い。
    assert_eq!(sel("ui", &ui("tree", "notes/c")), Ok(not_in_step()));
    assert_eq!(sel("ui", &ui("other", "notes/a")), Ok(not_in_step()));
}

#[test]
fn approval_accepts_approve() {
    assert_accepted(sel("approval", &appr(Decision::Approve, None)), "r-approve");
}

#[test]
fn approval_reject_rule_rejects() {
    let r = sel("approval", &appr(Decision::Reject, None)).unwrap();
    assert!(
        matches!(&r, SelectVerdict::RejectedByRule { rejection, .. } if rejection == "rj-decline")
    );
}

#[test]
fn approval_revise_without_rule_rejects() {
    // 対象つきの規則しか無い「直す」に、対象なしの入力は一致しない。
    assert_eq!(
        sel("approval", &appr(Decision::Revise, None)),
        Ok(not_in_step())
    );
    assert_eq!(
        sel("approval", &appr(Decision::Revise, Some("Y"))),
        Ok(not_in_step())
    );
    assert_accepted(
        sel("approval", &appr(Decision::Revise, Some("X"))),
        "r-revise-x",
    );
    // 対象つきの「承認」は対象なしの規則に一致しない。
    assert_eq!(
        sel("approval", &appr(Decision::Approve, Some("X"))),
        Ok(not_in_step())
    );
}

#[test]
fn mismatched_kind_and_missing_input_are_not_in_this_step() {
    assert_eq!(sel("choice", &ui("tree", "notes/a")), Ok(not_in_step()));
    assert_eq!(sel("ui", &choice("opt-memo")), Ok(not_in_step()));
    assert_eq!(sel("text_step", &choice("x")), Ok(not_in_step()));
    assert_eq!(
        sel("no_input", &appr(Decision::Approve, None)),
        Ok(not_in_step())
    );
}

#[test]
fn non_select_inputs_are_errors() {
    for i in [
        Input::Command("/x".into()),
        Input::Text("t".into()),
        Input::Cancel,
    ] {
        assert_eq!(sel("choice", &i), Err(SelectJudgeError::NotSelectInput));
        assert_eq!(
            conf("confirm_approve", &i),
            Err(SelectJudgeError::NotSelectInput)
        );
    }
}

#[test]
fn invalid_declarations_never_accept() {
    let i = appr(Decision::Approve, None);
    let invalid = Err(SelectJudgeError::InvalidDeclaration);
    assert_eq!(sel("bad_outcome", &i), invalid);
    assert_eq!(sel("bad_reject", &i), invalid);
    assert_eq!(sel("bad_input", &choice("a")), invalid);
    // approval も未知の欄を含む宣言を読めないものとして扱い、一致する規則があっても受付にしない。
    assert_eq!(sel("bad_approval_input", &i), invalid);
    assert_eq!(
        conf("confirm_bad", &i),
        Err(SelectJudgeError::InvalidDeclaration)
    );
    assert!(!SelectJudgeError::InvalidDeclaration
        .to_string()
        .contains("approve"));
}

#[test]
fn when_none_and_false_skip_rule() {
    let s = step("choice");
    let i = choice("opt-when");
    let none = judge_select(&s, &i, &|_| None).unwrap();
    assert!(
        matches!(none, SelectVerdict::RejectedByRule { ref rule_id, .. } if rule_id == "r-when-fallback")
    );
    let no = judge_select(&s, &i, &|_| Some(false)).unwrap();
    assert_eq!(none, no);
    assert_accepted(judge_select(&s, &i, &|_| Some(true)), "r-when");
}

#[test]
fn confirm_answer() {
    assert_eq!(
        conf("confirm_answer", &choice("right")),
        Ok(ConfirmVerdict::Confirmed)
    );
    assert_eq!(
        conf("confirm_answer", &choice("wrong-own")),
        Ok(ConfirmVerdict::WrongAnswer {
            option: "wrong-own".into(),
            text: "w-own".into()
        })
    );
    assert_eq!(
        conf("confirm_answer", &choice("wrong-default")),
        Ok(ConfirmVerdict::WrongAnswer {
            option: "wrong-default".into(),
            text: "w-default".into()
        })
    );
    assert_eq!(conf("confirm_answer", &choice("ghost")), confirm_rejected());
    assert_eq!(
        conf("confirm_answer", &ui("tree", "notes/a")),
        confirm_rejected()
    );
}

#[test]
fn confirm_point_at() {
    assert_eq!(
        conf("confirm_point", &ui("tree", "notes/a")),
        Ok(ConfirmVerdict::Confirmed)
    );
    assert_eq!(
        conf("confirm_point", &ui("tree", "notes/b")),
        Ok(ConfirmVerdict::Missed {
            text: "m-miss".into()
        })
    );
    assert_eq!(
        conf("confirm_point_nomiss", &ui("tree", "notes/b")),
        confirm_rejected()
    );
}

#[test]
fn confirm_approve_and_none() {
    for name in ["confirm_approve", "confirm_missing"] {
        assert_eq!(
            conf(name, &appr(Decision::Approve, None)),
            Ok(ConfirmVerdict::Confirmed)
        );
        for i in [
            appr(Decision::Reject, None),
            appr(Decision::Revise, None),
            appr(Decision::Approve, Some("X")),
            choice("a"),
        ] {
            assert_eq!(conf(name, &i), confirm_rejected());
        }
    }
    assert_eq!(
        conf("confirm_none", &appr(Decision::Approve, None)),
        confirm_rejected()
    );
}

#[test]
fn verdict_kinds() {
    assert_eq!(ConfirmVerdict::Confirmed.kind(), JudgmentKind::Confirmed);
    assert_eq!(
        ConfirmVerdict::Missed {
            text: String::new()
        }
        .kind(),
        JudgmentKind::Rejected
    );
}

fn input_strategy() -> impl Strategy<Value = Input> {
    let word = prop::sample::select(vec![
        "opt-memo",
        "opt-todo",
        "opt-ask",
        "opt-when",
        "ghost",
        "right",
        "wrong-own",
        "tree",
        "notes/a",
        "notes/b",
        "X",
        "",
    ]);
    let decision =
        prop::sample::select(vec![Decision::Approve, Decision::Reject, Decision::Revise]);
    prop_oneof![
        word.clone().prop_map(|w| Input::Choice(w.to_string())),
        (word.clone(), word.clone()).prop_map(|(v, p)| Input::Ui {
            view: v.to_string(),
            path: p.to_string()
        }),
        (decision, prop::option::of(word)).prop_map(|(d, t)| Input::Approval {
            decision: d,
            target: t.map(str::to_string)
        }),
        Just(Input::Cancel),
        Just(Input::Text("t".into())),
        Just(Input::Command("/c".into())),
    ]
}

#[test]
fn judging_is_deterministic_and_total() {
    let all = steps();
    check(
        "select_judge_total",
        DEFAULT_CASES,
        input_strategy(),
        |input| {
            for step in all.values() {
                let a = judge_select(step, &input, &never);
                prop_assert_eq!(&a, &judge_select(step, &input, &never));
                if let Ok(v) = &a {
                    prop_assert!(matches!(
                        v.kind(),
                        JudgmentKind::Accepted | JudgmentKind::Rejected | JudgmentKind::Clarified
                    ));
                }
                let c = judge_confirm(step, &input);
                prop_assert_eq!(&c, &judge_confirm(step, &input));
                if let Ok(v) = &c {
                    prop_assert!(matches!(
                        v.kind(),
                        JudgmentKind::Confirmed | JudgmentKind::Rejected
                    ));
                }
            }
            Ok(())
        },
    );
}

#[test]
fn accepted_only_for_declared_choice_values() {
    // 受付は、宣言にある値だけに返る（宣言に無い ID は、規則が何であっても受付にならない）。
    let s = step("choice");
    check(
        "select_judge_declared_only",
        DEFAULT_CASES,
        input_strategy(),
        |input| {
            if let Ok(SelectVerdict::Accepted { .. }) = judge_select(&s, &input, &|_| Some(true)) {
                prop_assert!(matches!(&input, Input::Choice(id) if id != "ghost"));
            }
            Ok(())
        },
    );
}

#[test]
fn source_has_no_external_calls() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/src/select_judge.rs");
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
        assert!(!code.contains(banned), "{banned}");
    }
}
