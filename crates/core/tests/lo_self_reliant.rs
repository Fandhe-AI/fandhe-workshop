//! 選び直しの手順を持つ確認と持たない確認の自力の数え方 `lo_self_reliant` の試験（TASK-304 2/3・3/3、GUIDE-11・GUIDE-12）。
//!
//! 期待の出所は TASK-3 の決定（#484、2026-10-31）。04 の GUIDE-11 は submodule の `d70ad10` ではまだ未反映のため、
//! 決定のコメントを正とする。AI の代理の結果は人の評価に数えない（D-34）。ここでの数え直しは規則の机上の照合である。
//! 呼び出し元は CI の `make test` のみ。
//!
//! 履歴からの試験は `min-3step.json` の st-2 を `answer` の確認にして使う（`lo_observe.rs` と同じ補助を写す。
//! `tests/` のファイル間で非公開の補助は共有できないため）。答えを含むヒントの段は 3 を渡す。
//! 確認の種類は IR からは決めず、呼び出し側が規則（`reselectable`・`no_reselection`）で選ぶ。

mod common;

use std::path::PathBuf;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::lo_check::{
    CheckMethod, CheckTarget, FirstAnswer, LoCheckError, LoCheckRecord, Reselection,
};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::lo_observe::records_from_history;
use fandhe_workshop_core::lo_self_reliant::{
    classify, count_participants, tally, CheckKind, ParticipantCount, SelfRelianceOutcome,
    SelfRelianceRule, SelfRelianceRuleError,
};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use proptest::prelude::*;
use serde_json::json;

const ST2: &str = "main/st-2";

fn rule(stage: u32) -> SelfRelianceRule {
    SelfRelianceRule::reselectable(stage).expect("non-zero stage")
}

fn nr(stage: u32) -> SelfRelianceRule {
    SelfRelianceRule::no_reselection(stage).expect("non-zero stage")
}

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn answer_ir() -> Ir {
    let patch = json!({
        "hints": ["h-1-1", "h-1-2", "h-1-3"],
        "confirm": {
            "kind": "answer",
            "on_wrong": "rj-decline",
            "options": [
                { "id": "opt-memo", "correct": true },
                { "id": "opt-todo", "correct": false }
            ]
        }
    });
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

fn pick(id: &str) -> SessionOp {
    HistoryOp::Engine(Op::Input(Input::Choice(id.into())))
}

fn hint() -> SessionOp {
    HistoryOp::Engine(Op::Control(Control::OpenHint))
}

/// 操作の列から st-2 の記録を 1 件導く。
fn from_history(ops: &[SessionOp]) -> LoCheckRecord {
    let targets = vec![CheckTarget {
        step: ST2.into(),
        lo: vec!["LO-X".into()],
        method: CheckMethod::Answer,
    }];
    let mut r = records_from_history(&answer_ir(), entered_state(), ops, &targets).expect("replay");
    assert_eq!(r.len(), 1);
    r.remove(0)
}

fn rec(
    step: &str,
    lo: &[&str],
    first: FirstAnswer,
    re: Reselection,
    cmp: bool,
    hint: u32,
) -> LoCheckRecord {
    LoCheckRecord {
        step: step.to_string(),
        lo: lo.iter().map(|s| s.to_string()).collect(),
        method: CheckMethod::Answer,
        first_answer: first,
        reselection: re,
        after_comparison: cmp,
        max_hint: hint,
    }
}

fn first_correct(step: &str, hint: u32) -> LoCheckRecord {
    rec(
        step,
        &["LO-1"],
        FirstAnswer::Correct,
        Reselection::None,
        false,
        hint,
    )
}

// --- 履歴から ---

#[test]
fn first_and_second_stage_hints_then_correct_are_self_reliant() {
    for n in 1..=2usize {
        let mut ops = vec![pick("opt-memo")];
        ops.extend((0..n).map(|_| hint()));
        ops.push(pick("opt-memo"));
        let r = from_history(&ops);
        assert_eq!(r.max_hint, n as u32);
        assert_eq!(
            classify(&r, &rule(3)).expect("valid"),
            SelfRelianceOutcome::SelfReliant { reselected: false },
            "n={n}"
        );
    }
}

#[test]
fn third_stage_hint_then_correct_is_not_self_reliant() {
    let ops = vec![pick("opt-memo"), hint(), hint(), hint(), pick("opt-memo")];
    let r = from_history(&ops);
    assert_eq!(r.max_hint, 3);
    assert_eq!(
        classify(&r, &rule(3)).expect("valid"),
        SelfRelianceOutcome::AfterAnswerHint { reselected: false }
    );
    let t = tally(&[r], "LO-X", &rule(3)).expect("valid");
    assert_eq!(t.self_reliant(), 0);
    assert_eq!(t.after_answer_hint, 1);
    assert_eq!(t.max_hint, 3);
}

#[test]
fn wrong_then_reselect_counts_as_self_reliant() {
    let r = from_history(&[pick("opt-memo"), pick("opt-todo"), pick("opt-memo")]);
    assert_eq!(
        classify(&r, &rule(3)).expect("valid"),
        SelfRelianceOutcome::SelfReliant { reselected: true }
    );
    let t = tally(&[r], "LO-X", &rule(3)).expect("valid");
    assert_eq!(t.self_reliant_reselected, 1);
    assert_eq!(t.self_reliant_first, 0);
}

#[test]
fn third_stage_hint_before_reselection_is_not_self_reliant() {
    let r = from_history(&[
        pick("opt-memo"),
        pick("opt-todo"),
        hint(),
        hint(),
        hint(),
        pick("opt-memo"),
    ]);
    assert_eq!(
        classify(&r, &rule(3)).expect("valid"),
        SelfRelianceOutcome::AfterAnswerHint { reselected: true }
    );
}

#[test]
fn wrong_only_and_unanswered_are_not_self_reliant() {
    let w = from_history(&[pick("opt-memo"), pick("opt-todo")]);
    assert_eq!(
        classify(&w, &rule(3)).expect("valid"),
        SelfRelianceOutcome::NotCorrect
    );
    let u = from_history(&[pick("opt-memo")]);
    assert_eq!(
        classify(&u, &rule(3)).expect("valid"),
        SelfRelianceOutcome::Unanswered
    );
}

// --- 記録から ---

#[test]
fn answer_stage_is_given_by_caller() {
    let r = first_correct("s1", 2);
    assert!(classify(&r, &rule(3)).expect("valid").is_self_reliant());
    assert_eq!(
        classify(&r, &rule(2)).expect("valid"),
        SelfRelianceOutcome::AfterAnswerHint { reselected: false }
    );
}

#[test]
fn zero_answer_stage_is_rejected() {
    assert_eq!(
        SelfRelianceRule::reselectable(0),
        Err(SelfRelianceRuleError::ZeroAnswerStage)
    );
    assert_eq!(
        SelfRelianceRuleError::ZeroAnswerStage.to_string(),
        "answer hint stage must be at least 1"
    );
}

#[test]
fn reselection_after_comparison_is_not_self_reliant() {
    let r = rec(
        "s1",
        &["LO-1"],
        FirstAnswer::Wrong,
        Reselection::Correct,
        true,
        0,
    );
    let o = classify(&r, &rule(3)).expect("valid");
    assert_eq!(o, SelfRelianceOutcome::ReselectedAfterComparison);
    assert!(!o.is_self_reliant());
    let t = tally(&[r], "LO-1", &rule(3)).expect("valid");
    assert_eq!((t.self_reliant(), t.reselected_after_comparison), (0, 1));
}

#[test]
fn invalid_record_is_rejected() {
    // 正答なのに選び直しがある、は不変条件違反。指定の LO 以外の記録でも読み飛ばさない。
    let bad = rec(
        "s2",
        &["LO-9"],
        FirstAnswer::Correct,
        Reselection::Correct,
        false,
        0,
    );
    assert_eq!(
        classify(&bad, &rule(3)),
        Err(LoCheckError::ReselectionWithoutWrong)
    );
    let good = first_correct("s1", 0);
    assert!(tally(&[good.clone(), bad.clone()], "LO-1", &rule(3)).is_err());
    assert!(count_participants(&[vec![good], vec![bad]], "LO-1", &rule(3)).is_err());
}

#[test]
fn tally_counts_only_the_designated_lo() {
    let other = rec(
        "s2",
        &["LO-2"],
        FirstAnswer::Unanswered,
        Reselection::None,
        false,
        0,
    );
    let multi = rec(
        "s3",
        &["LO-1", "LO-2"],
        FirstAnswer::Correct,
        Reselection::None,
        false,
        0,
    );
    let rs = [first_correct("s1", 0), other, multi];
    let t1 = tally(&rs, "LO-1", &rule(3)).expect("valid");
    assert_eq!((t1.checks(), t1.self_reliant()), (2, 2));
    let t2 = tally(&rs, "LO-2", &rule(3)).expect("valid");
    assert_eq!((t2.checks(), t2.self_reliant(), t2.unanswered), (2, 1, 1));
}

#[test]
fn max_hint_is_kept_separately() {
    let rs = [first_correct("s1", 2), first_correct("s2", 1)];
    let t = tally(&rs, "LO-1", &rule(3)).expect("valid");
    assert_eq!(t.self_reliant(), 2);
    assert_eq!(t.max_hint, 2);
}

#[test]
fn participant_needs_all_checks_self_reliant() {
    let mut rs: Vec<_> = (0..4).map(|i| first_correct(&format!("s{i}"), 0)).collect();
    rs.push(first_correct("s4", 3));
    let t = tally(&rs, "LO-1", &rule(3)).expect("valid");
    assert_eq!((t.checks(), t.self_reliant()), (5, 4));
    assert!(!t.all_self_reliant());
}

#[test]
fn participant_without_checks_is_not_counted() {
    let none: Vec<LoCheckRecord> = vec![];
    let t = tally(&none, "LO-1", &rule(3)).expect("valid");
    assert!(!t.all_self_reliant());
    let c = count_participants(&[none], "LO-1", &rule(3)).expect("valid");
    assert_eq!(
        c,
        ParticipantCount {
            participants: 1,
            all_self_reliant: 0
        }
    );
}

// --- AI の代理の run の数え直し ---

/// PoC-13 の集計（`docs/spec/03-poc/learner-trial-intro/ai-proxy/runs/summary.md` の 4・5 節、
/// `docs/spec/03-poc/README.md` の 9 節）の LO-1 の確認 5 件 × 4 run を記録の水準で写す。
/// PoC の run は本エンジンの履歴の形ではなく、CI では `docs/spec` が無い（`check-sibling-free`）ため、
/// 履歴の再生ではなく記録で照合する。r3 は 2 件に到達していない（未回答）。
#[test]
fn ai_proxy_recount_lo1_is_three_of_four() {
    let steps = ["st-0-2a", "st-0-2b", "st-0-2c", "st-3-1", "st-3-2"];
    let run = |reached: usize| -> Vec<LoCheckRecord> {
        steps
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let first = if i < reached {
                    FirstAnswer::Correct
                } else {
                    FirstAnswer::Unanswered
                };
                rec(
                    &format!("main/{s}"),
                    &["LO-1"],
                    first,
                    Reselection::None,
                    false,
                    0,
                )
            })
            .collect()
    };
    let runs = vec![run(5), run(5), run(3), run(5)];
    let c = count_participants(&runs, "LO-1", &rule(3)).expect("valid");
    assert_eq!(
        c,
        ParticipantCount {
            participants: 4,
            all_self_reliant: 3
        }
    );
}

// --- 性質 ---

fn any_record() -> impl Strategy<Value = LoCheckRecord> {
    (0u8..5, 0u32..6, any::<bool>()).prop_map(|(kind, hint, cmp)| match kind {
        0 => rec(
            "s",
            &["LO-1"],
            FirstAnswer::Correct,
            Reselection::None,
            false,
            hint,
        ),
        1 => rec(
            "s",
            &["LO-1"],
            FirstAnswer::Unanswered,
            Reselection::None,
            false,
            hint,
        ),
        2 => rec(
            "s",
            &["LO-1"],
            FirstAnswer::Wrong,
            Reselection::NotCorrect,
            cmp,
            hint,
        ),
        _ => rec(
            "s",
            &["LO-1"],
            FirstAnswer::Wrong,
            Reselection::Correct,
            cmp,
            hint,
        ),
    })
}

#[test]
fn prop_all_self_reliant_implies_every_record_self_reliant() {
    let strat = proptest::collection::vec(any_record(), 0..12);
    check("lo-self-reliant-all", DEFAULT_CASES, strat, |rs| {
        let r = rule(3);
        let t = tally(&rs, "LO-1", &r).expect("valid");
        let sum = t.self_reliant_first
            + t.self_reliant_reselected
            + t.after_answer_hint
            + t.reselected_after_comparison
            + t.correct_after_wrong
            + t.not_correct
            + t.unanswered;
        prop_assert_eq!(sum, t.checks());
        prop_assert_eq!(t.checks() as usize, rs.len());
        if t.all_self_reliant() {
            for x in &rs {
                prop_assert!(classify(x, &r).expect("valid").is_self_reliant());
            }
        }
        Ok(())
    });
}

// --- 選び直しの手順を持たない確認（GUIDE-12） ---

fn wrong_then_correct(cmp: bool, hint: u32) -> LoCheckRecord {
    rec(
        "s",
        &["LO-1"],
        FirstAnswer::Wrong,
        Reselection::Correct,
        cmp,
        hint,
    )
}

#[test]
fn no_reselection_first_correct_is_self_reliant() {
    let r = from_history(&[pick("opt-memo"), pick("opt-memo")]);
    assert_eq!(
        classify(&r, &nr(3)).expect("valid"),
        SelfRelianceOutcome::SelfReliant { reselected: false }
    );
    let t = tally(&[r], "LO-X", &nr(3)).expect("valid");
    assert_eq!((t.self_reliant_first, t.self_reliant()), (1, 1));
}

#[test]
fn no_reselection_first_and_second_stage_hints_then_correct_are_self_reliant() {
    for n in 1..=2usize {
        let mut ops = vec![pick("opt-memo")];
        ops.extend((0..n).map(|_| hint()));
        ops.push(pick("opt-memo"));
        let r = from_history(&ops);
        assert_eq!(r.max_hint, n as u32);
        assert_eq!(
            classify(&r, &nr(3)).expect("valid"),
            SelfRelianceOutcome::SelfReliant { reselected: false },
            "n={n}"
        );
    }
}

#[test]
fn no_reselection_third_stage_hint_then_correct_is_not_self_reliant() {
    let r = from_history(&[pick("opt-memo"), hint(), hint(), hint(), pick("opt-memo")]);
    assert_eq!(
        classify(&r, &nr(3)).expect("valid"),
        SelfRelianceOutcome::AfterAnswerHint { reselected: false }
    );
    let t = tally(&[r], "LO-X", &nr(3)).expect("valid");
    assert_eq!(
        (t.self_reliant(), t.after_answer_hint, t.max_hint),
        (0, 1, 3)
    );
}

#[test]
fn no_reselection_wrong_then_correct_is_not_self_reliant() {
    let r = from_history(&[pick("opt-memo"), pick("opt-todo"), pick("opt-memo")]);
    // 同じ記録でも、選び直しの手順を持つ確認では自力、持たない確認では自力でない。
    assert_eq!(
        classify(&r, &rule(3)).expect("valid"),
        SelfRelianceOutcome::SelfReliant { reselected: true }
    );
    let o = classify(&r, &nr(3)).expect("valid");
    assert_eq!(o, SelfRelianceOutcome::CorrectAfterWrong);
    assert!(!o.is_self_reliant());
    let t = tally(&[r], "LO-X", &nr(3)).expect("valid");
    assert_eq!(
        (
            t.correct_after_wrong,
            t.self_reliant_reselected,
            t.self_reliant()
        ),
        (1, 0, 0)
    );
    assert_eq!(t.checks(), 1);
}

#[test]
fn no_reselection_third_stage_before_correction_is_after_answer_hint() {
    // 答えの段の判定が、誤答の後の正答の判定より先に勝つ。
    let r = from_history(&[
        pick("opt-memo"),
        pick("opt-todo"),
        hint(),
        hint(),
        hint(),
        pick("opt-memo"),
    ]);
    assert_eq!(
        classify(&r, &nr(3)).expect("valid"),
        SelfRelianceOutcome::AfterAnswerHint { reselected: true }
    );
}

#[test]
fn no_reselection_wrong_only_and_unanswered_are_not_self_reliant() {
    let w = from_history(&[pick("opt-memo"), pick("opt-todo")]);
    assert_eq!(
        classify(&w, &nr(3)).expect("valid"),
        SelfRelianceOutcome::NotCorrect
    );
    let u = from_history(&[pick("opt-memo")]);
    assert_eq!(
        classify(&u, &nr(3)).expect("valid"),
        SelfRelianceOutcome::Unanswered
    );
    let none = rec(
        "s",
        &["LO-1"],
        FirstAnswer::Unanswered,
        Reselection::None,
        false,
        0,
    );
    assert_eq!(
        classify(&none, &nr(3)).expect("valid"),
        SelfRelianceOutcome::Unanswered
    );
}

#[test]
fn no_reselection_after_comparison_is_correct_after_wrong() {
    for cmp in [false, true] {
        assert_eq!(
            classify(&wrong_then_correct(cmp, 0), &nr(3)).expect("valid"),
            SelfRelianceOutcome::CorrectAfterWrong,
            "cmp={cmp}"
        );
    }
}

#[test]
fn no_reselection_rule_kind_and_zero_stage() {
    assert_eq!(
        SelfRelianceRule::no_reselection(0),
        Err(SelfRelianceRuleError::ZeroAnswerStage)
    );
    assert_eq!(nr(3).kind(), CheckKind::NoReselection);
    assert_eq!(nr(3).answer_stage(), 3);
    assert_eq!(rule(3).kind(), CheckKind::Reselectable);
}

#[test]
fn no_reselection_invalid_record_is_rejected() {
    let bad = rec(
        "s2",
        &["LO-9"],
        FirstAnswer::Correct,
        Reselection::Correct,
        false,
        0,
    );
    assert_eq!(
        classify(&bad, &nr(3)),
        Err(LoCheckError::ReselectionWithoutWrong)
    );
    let good = first_correct("s1", 0);
    assert!(tally(&[good.clone(), bad.clone()], "LO-1", &nr(3)).is_err());
    assert!(count_participants(&[vec![good], vec![bad]], "LO-1", &nr(3)).is_err());
}

/// LO-4 の 4 確認の束ね（TASK-3 の決定）。Step の ID は教材の側の約束で、ここでは記録の写しとして置く。
fn lo4_steps(recs: [LoCheckRecord; 4]) -> Vec<LoCheckRecord> {
    let ids = [
        "main/st-8-2a",
        "main/st-8-2b",
        "main/st-8-2c",
        "main/st-14-3",
    ];
    recs.into_iter()
        .zip(ids)
        .map(|(mut r, id)| {
            r.step = id.to_string();
            r.lo = vec!["LO-4".into()];
            r
        })
        .collect()
}

#[test]
fn lo4_bundle_counts_only_all_first_correct_participants() {
    let ok = || first_correct("x", 0);
    let wrong_then = || wrong_then_correct(false, 0);
    let third = || first_correct("x", 3);
    let unanswered = || {
        rec(
            "x",
            &["LO-4"],
            FirstAnswer::Unanswered,
            Reselection::None,
            false,
            0,
        )
    };
    let people = vec![
        lo4_steps([ok(), ok(), ok(), ok()]),
        lo4_steps([ok(), wrong_then(), ok(), ok()]),
        lo4_steps([ok(), ok(), third(), ok()]),
        lo4_steps([ok(), ok(), ok(), unanswered()]),
    ];
    assert_eq!(
        count_participants(&people, "LO-4", &nr(3)).expect("valid"),
        ParticipantCount {
            participants: 4,
            all_self_reliant: 1
        }
    );
    // 選び直しの手順を持つ確認として数えると、誤答の後の正答も自力になる。
    assert_eq!(
        count_participants(&people, "LO-4", &rule(3))
            .expect("valid")
            .all_self_reliant,
        2
    );
}

#[test]
fn lo3_four_scenes_bundle() {
    let scenes = |last: LoCheckRecord| {
        let mut v: Vec<_> = ["main/s-a", "main/s-b", "main/s-c"]
            .iter()
            .map(|id| first_correct(id, 0))
            .collect();
        v.push(LoCheckRecord {
            step: "main/s-d".into(),
            ..last
        });
        v
    };
    let all = scenes(first_correct("x", 0));
    assert!(tally(&all, "LO-1", &nr(3))
        .expect("valid")
        .all_self_reliant());
    let one_wrong = scenes(wrong_then_correct(false, 0));
    assert!(!tally(&one_wrong, "LO-1", &nr(3))
        .expect("valid")
        .all_self_reliant());
}

#[test]
fn prop_no_reselection_never_counts_reselected_as_self_reliant() {
    let strat = proptest::collection::vec(any_record(), 0..12);
    check(
        "lo-self-reliant-no-reselection",
        DEFAULT_CASES,
        strat,
        |rs| {
            let r = nr(3);
            let t = tally(&rs, "LO-1", &r).expect("valid");
            prop_assert_eq!(t.self_reliant_reselected, 0);
            let sum = t.self_reliant_first
                + t.after_answer_hint
                + t.reselected_after_comparison
                + t.correct_after_wrong
                + t.not_correct
                + t.unanswered;
            prop_assert_eq!(sum, t.checks());
            prop_assert_eq!(t.checks() as usize, rs.len());
            if t.all_self_reliant() {
                for x in &rs {
                    prop_assert_eq!(x.first_answer, FirstAnswer::Correct);
                    prop_assert!(x.max_hint < 3);
                }
            }
            Ok(())
        },
    );
}
