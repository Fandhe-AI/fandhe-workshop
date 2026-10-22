//! 承認の「直す」の対象の候補と再承認の判定（TASK-265、JUDGE-11・D-69）の試験。
//!
//! 試験データ `tests/data/revise_candidates_1/ir.json` は、testdata の ST-2.6 の形（正しい対象 `expect`・対象なしの
//! 拒否の規則・早すぎる承認の拒否）に、`approval.revise.candidates`（PACK-42）で候補 3 件を宣言した手書きの写しで、
//! 矛盾した教材・`when` つき・`on_reject` の上書き・候補を宣言しない v1.4 の形・コンパイル済みの形を足してある。
//! 確かめるのは次の点である。
//! - 候補と表示名が宣言の順で返り、宣言が無ければ規則の対象が表示名なしで返る
//! - 誤った候補の「直す」は、対象なしの拒否の規則（教材が誤った対象向けに書いた文面）で拒否され、受付・問い返しにならない
//! - 宣言に無い対象は、規則に関わらず分類による拒否になる
//! - 再承認を別の Step に分けた形で、再承認が別の Step の入力として判定される
//!
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::input_spec::ReviseCandidate;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::rejection_text::rejection_of;
use fandhe_workshop_core::select_judge::{
    judge_select, revise_candidates, revise_targets, SelectJudgeError, SelectVerdict,
};
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use proptest::prelude::*;
use serde_json::{json, Value};

fn ir() -> Ir {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/revise_candidates_1/ir.json"
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

fn always(_: &Value) -> Option<bool> {
    Some(true)
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

fn cand(id: &str, label: Option<&str>) -> ReviseCandidate {
    ReviseCandidate {
        id: id.to_string(),
        label: label.map(str::to_string),
    }
}

fn with_input(id: &str, input: Value) -> Step {
    let mut s = step_of(&ir(), id);
    s.input = Some(input);
    s
}

#[test]
fn judge11_candidates_in_declared_order_with_labels() {
    let c = revise_candidates(&step_of(&ir(), "st-cand")).unwrap();
    assert_eq!(
        c,
        vec![
            cand("expect", Some("期待する結果")),
            cand("reason", Some("理由")),
            cand("scope", None)
        ]
    );
    let shown: Vec<&str> = c.iter().map(ReviseCandidate::display).collect();
    assert_eq!(shown, ["期待する結果", "理由", "scope"]);
}

#[test]
fn judge11_candidates_fall_back_to_rule_targets_without_declaration() {
    let c = revise_candidates(&step_of(&ir(), "st-legacy")).unwrap();
    assert_eq!(c, vec![cand("expect", None), cand("reason", None)]);
    assert_eq!(c[0].display(), "expect");
}

#[test]
fn judge11_candidates_are_empty_for_other_input_kinds() {
    let ir = ir();
    for id in ["st-choice", "st-noinput", "st-end"] {
        assert_eq!(revise_candidates(&step_of(&ir, id)), Ok(vec![]), "{id}");
    }
}

#[test]
fn judge11_invalid_candidate_declaration_is_error() {
    let bad = [
        json!({ "kind": "approval", "revise": { "candidates": [{ "id": "a", "extra": 1 }] } }),
        json!({ "kind": "approval", "revise": { "candidates": [] } }),
        json!({ "kind": "approval", "revise": { "candidates": [{ "id": "a" }, { "id": "a" }] } }),
        json!({ "kind": "approval", "revise": { "candidates": [{ "id": "a", "label": "  " }] } }),
        json!({ "kind": "approval", "unknown": true }),
    ];
    for decl in bad {
        let s = with_input("st-cand", decl);
        assert_eq!(
            revise_candidates(&s),
            Err(SelectJudgeError::InvalidDeclaration)
        );
        // 宣言が壊れた Step は、どの入力も受付にしない。
        for input in [
            revise(Some("expect")),
            revise(None),
            appr(Decision::Approve, None),
        ] {
            assert_eq!(
                judge_select(&s, &input, &always),
                Err(SelectJudgeError::InvalidDeclaration)
            );
        }
    }
    let text = SelectJudgeError::InvalidDeclaration.to_string();
    assert!(
        !text.contains("expect") && !text.contains("candidates"),
        "{text}"
    );
}

#[test]
fn judge11_correct_candidate_is_accepted() {
    let s = step_of(&ir(), "st-cand");
    assert!(matches!(
        judge_select(&s, &revise(Some("expect")), &never),
        Ok(SelectVerdict::Accepted { rule_id, .. }) if rule_id == "r-revise-expect"
    ));
}

#[test]
fn judge11_wrong_candidate_takes_targetless_reject_rule() {
    let ir = ir();
    let s = step_of(&ir, "st-cand");
    for t in ["reason", "scope"] {
        let v = judge_select(&s, &revise(Some(t)), &never).unwrap();
        assert!(
            matches!(
                &v,
                SelectVerdict::RejectedByRule { rule_id, rejection, .. }
                    if rule_id == "r-revise-other" && rejection == "rj-wrong-target"
            ),
            "{t}: {v:?}"
        );
        let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
        assert_eq!(r.category, RejectCategory::NotInThisStep);
        assert_eq!(r.text, "rj-wrong-target");
        assert!(!r.next_action.as_str().trim().is_empty());
    }
}

#[test]
fn judge11_wrong_candidate_without_reject_rule_falls_to_category() {
    let ir = ir();
    let s = step_of(&ir, "st-cand-norule");
    let v = judge_select(&s, &revise(Some("scope")), &never).unwrap();
    assert_eq!(v, SelectVerdict::Rejected(RejectCategory::NotInThisStep));
    let r = rejection_of(&ir, &s, &v).unwrap().unwrap();
    assert_eq!(r.text, "rj-default");
    assert!(!r.next_action.as_str().trim().is_empty());
}

#[test]
fn judge11_wrong_candidate_respects_on_reject_override() {
    let ir = ir();
    let s = step_of(&ir, "st-cand-override");
    let v = judge_select(&s, &revise(Some("scope")), &never).unwrap();
    assert_eq!(v, SelectVerdict::Rejected(RejectCategory::NotInThisStep));
    assert_eq!(
        rejection_of(&ir, &s, &v).unwrap().unwrap().text,
        "rj-override"
    );
}

#[test]
fn judge11_wrong_candidate_never_accepts_or_clarifies() {
    let s = step_of(&ir(), "st-cand-trap");
    for t in ["reason", "scope"] {
        let v = judge_select(&s, &revise(Some(t)), &always).unwrap();
        assert!(
            matches!(&v, SelectVerdict::RejectedByRule { rule_id, .. } if rule_id == "r-revise-other"),
            "{t}: {v:?}"
        );
    }
    // 対象なしでも同じ（JUDGE-10 の不変条件）。
    assert!(matches!(
        judge_select(&s, &revise(None), &always),
        Ok(SelectVerdict::RejectedByRule { .. })
    ));
}

#[test]
fn judge11_when_false_skips_reject_rule() {
    let ir = ir();
    let s = step_of(&ir, "st-cand-when");
    assert_eq!(
        judge_select(&s, &revise(Some("scope")), &never),
        Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep))
    );
    assert_eq!(
        judge_select(&s, &revise(Some("scope")), &|_| Some(false)),
        Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep))
    );
    assert!(matches!(
        judge_select(&s, &revise(Some("scope")), &always),
        Ok(SelectVerdict::RejectedByRule { .. })
    ));
}

#[test]
fn judge11_undeclared_target_is_rejected_by_category() {
    let ir = ir();
    for id in ["st-cand", "st-cand-trap", "st-cand-wide"] {
        let s = step_of(&ir, id);
        let v = judge_select(&s, &revise(Some("unlisted")), &always).unwrap();
        assert_eq!(
            v,
            SelectVerdict::Rejected(RejectCategory::NotInThisStep),
            "{id}"
        );
    }
}

#[test]
fn judge11_bare_revise_with_declared_candidates_is_never_accepted() {
    let ir = ir();
    for id in ["st-cand", "st-cand-norule", "st-cand-trap", "st-cand-only"] {
        let s = step_of(&ir, id);
        let v = judge_select(&s, &revise(None), &always).unwrap();
        assert!(
            !matches!(
                v,
                SelectVerdict::Accepted { .. } | SelectVerdict::Clarified { .. }
            ),
            "{id}: {v:?}"
        );
    }
    // 候補を宣言しない v1.4 以前の形は従来のまま（対象つきの規則が無ければ受付）。
    let mut legacy = step_of(&ir, "st-cand-only");
    legacy.input = Some(json!({ "kind": "approval" }));
    assert!(matches!(
        judge_select(&legacy, &revise(None), &never),
        Ok(SelectVerdict::Accepted { .. })
    ));
}

#[test]
fn judge11_legacy_without_declaration_keeps_old_behavior() {
    let s = step_of(&ir(), "st-legacy");
    // 宣言が無い Step では、規則に無い対象は従来どおり分類による拒否で、対象なしの拒否の規則は当たらない。
    assert_eq!(
        judge_select(&s, &revise(Some("scope")), &never),
        Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep))
    );
}

#[test]
fn judge11_compiled_wide_rules() {
    let ir = ir();
    let s = step_of(&ir, "st-cand-wide");
    assert_eq!(revise_targets(&s), Ok(vec!["expect".to_string()]));
    assert_eq!(revise_candidates(&s).unwrap().len(), 3);

    // Session は normalized 済みの Step で判定する。
    let mut sess = session_at(ir, "st-cand-wide");
    let ev = send(&mut sess, revise(Some("scope")));
    assert!(rejected_by(&ev, "rj-wrong-target"), "{ev:?}");
    assert_eq!(achieved(&ev), 0);
    let ev = send(&mut sess, revise(Some("unlisted")));
    assert!(
        is_not_in_this_step(&ev) && !rejected_by(&ev, "rj-wrong-target"),
        "{ev:?}"
    );
}

fn uid(step: &str) -> StepUid {
    StepUid(format!("main/{step}"))
}

fn session_at(ir: Ir, step: &str) -> Session {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(u.root(), slice);
    let run = RunState {
        generation: Generation(1),
        current: Some(u),
        slices,
        ..RunState::default()
    };
    Session::from_state(
        ir,
        SessionState {
            run,
            ..SessionState::default()
        },
    )
}

fn send(s: &mut Session, input: Input) -> Vec<Event> {
    s.apply(Op::Input(input)).to_vec()
}

fn achieved(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

fn is_not_in_this_step(events: &[Event]) -> bool {
    events.iter().any(|e| {
        matches!(e, Event::Judged { judgment: Judgment::Rejected(r), .. }
            if r.category == RejectCategory::NotInThisStep)
    })
}

fn rejected_by(events: &[Event], text: &str) -> bool {
    events.iter().any(
        |e| matches!(e, Event::Judged { judgment: Judgment::Rejected(r), .. } if r.text == text),
    )
}

#[test]
fn judge11_reapproval_is_judged_in_a_separate_step() {
    let mut s = session_at(ir(), "st-cand");

    // 早すぎる承認と誤った候補は拒否で、Step は変わらない。
    let ev = send(&mut s, appr(Decision::Approve, None));
    assert!(rejected_by(&ev, "rj-unfixed"), "{ev:?}");
    let ev = send(&mut s, revise(Some("reason")));
    assert!(rejected_by(&ev, "rj-wrong-target"), "{ev:?}");
    assert_eq!(achieved(&ev), 0);
    assert_eq!(s.state().run.current, Some(uid("st-cand")));

    // 正しい対象は受付で、再承認の Step へ進む。
    let ev = send(&mut s, revise(Some("expect")));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(s.state().run.current, Some(uid("st-reapprove")));

    // 再承認の Step では、もう一度の「直す」は受付にならず、承認で達成になる。
    let ev = send(&mut s, revise(None));
    assert_eq!(achieved(&ev), 0, "{ev:?}");
    assert!(rejected_by(&ev, "rj-unfixed"), "{ev:?}");
    assert_eq!(s.state().run.current, Some(uid("st-reapprove")));
    let ev = send(&mut s, appr(Decision::Approve, None));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(s.state().run.current, Some(uid("st-end")));
}

fn target_strategy() -> impl Strategy<Value = Option<String>> {
    prop_oneof![
        Just(None),
        Just(Some("expect".to_string())),
        Just(Some("reason".to_string())),
        Just(Some("scope".to_string())),
        "[a-z]{0,6}".prop_map(Some),
    ]
}

fn when_strategy() -> impl Strategy<Value = Option<bool>> {
    prop_oneof![Just(Some(true)), Just(Some(false)), Just(None)]
}

#[test]
fn judge11_targeted_revise_accepted_iff_exact_accept_rule() {
    let ir = ir();
    let ids = [
        "st-cand",
        "st-cand-norule",
        "st-cand-trap",
        "st-cand-when",
        "st-cand-override",
        "st-cand-only",
        "st-legacy",
        "st-cand-wide",
    ];
    let steps: Vec<Step> = ids.iter().map(|i| step_of(&ir, i)).collect();
    check(
        "judge11_targeted_revise",
        DEFAULT_CASES,
        (
            0..steps.len(),
            prop_oneof![
                Just(Decision::Approve),
                Just(Decision::Reject),
                Just(Decision::Revise)
            ],
            target_strategy(),
            when_strategy(),
        ),
        |(i, decision, target, w)| {
            let s = &steps[i];
            let input = Input::Approval {
                decision,
                target: target.clone(),
            };
            let eval = |_: &Value| w;
            let a = judge_select(s, &input, &eval);
            prop_assert_eq!(&a, &judge_select(s, &input, &eval));
            let v = a.unwrap();
            if let (Decision::Revise, Some(t)) = (decision, &target) {
                if matches!(
                    v,
                    SelectVerdict::Accepted { .. } | SelectVerdict::Clarified { .. }
                ) {
                    // 対象つきの規則（完全一致）が受付を返したときに限る。
                    let hit = s
                        .rules
                        .iter()
                        .any(|r| r["on"]["target"] == json!(t) && r["outcome"]["kind"] == "accept");
                    prop_assert!(hit, "{t}: {v:?}");
                    let is_accept = matches!(v, SelectVerdict::Accepted { .. });
                    prop_assert!(is_accept);
                }
            }
            if let Some(r) = rejection_of(&ir, s, &v).unwrap() {
                prop_assert!(!r.next_action.as_str().trim().is_empty());
            }
            Ok(())
        },
    );
}
