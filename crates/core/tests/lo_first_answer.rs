//! LO の確認で最初の回答の正答だけを自力に数える規則の試験（TASK-305、GUIDE-18）。
//!
//! 入力は contract の `lo_check` の型だけである。契約サンプルは repo の中（`crates/contract/tests/data`）を読む。

mod common;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckMethod, CheckObservation, CheckTarget, FirstAnswer, LoCheckRecord,
    Reselection,
};
use fandhe_workshop_core::lo_first_answer::{
    classify, tally, FirstAnswerOutcome, FirstAnswerTally,
};
use proptest::prelude::*;

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
        method: CheckMethod::PointAt,
        first_answer: first,
        reselection: re,
        after_comparison: cmp,
        max_hint: hint,
    }
}

fn correct_first(step: &str, lo: &[&str]) -> LoCheckRecord {
    rec(step, lo, FirstAnswer::Correct, Reselection::None, false, 0)
}

fn reselected(step: &str, lo: &[&str], cmp: bool) -> LoCheckRecord {
    rec(step, lo, FirstAnswer::Wrong, Reselection::Correct, cmp, 0)
}

#[test]
fn first_answer_correct_counts_as_self_reliant() {
    let r = correct_first("s1", &["LO-2"]);
    let o = classify(&r).expect("valid");
    assert_eq!(o, FirstAnswerOutcome::SelfReliant);
    assert!(o.is_self_reliant());
}

#[test]
fn reselection_after_comparison_is_not_self_reliant_and_kept_apart() {
    let r = reselected("s2", &["LO-2"], true);
    let o = classify(&r).expect("valid");
    assert_eq!(
        o,
        FirstAnswerOutcome::Reselected {
            after_comparison: true
        }
    );
    assert!(!o.is_self_reliant());
    let t = tally(&[r], "LO-2").expect("valid");
    assert_eq!(t.self_reliant, 0);
    assert_eq!(t.reselected_after_comparison, 1);
}

#[test]
fn reselection_without_comparison_is_not_self_reliant() {
    let r = reselected("s2", &["LO-2"], false);
    let o = classify(&r).expect("valid");
    assert_eq!(
        o,
        FirstAnswerOutcome::Reselected {
            after_comparison: false
        }
    );
    assert!(!o.is_self_reliant());
    let t = tally(&[r], "LO-2").expect("valid");
    assert_eq!(t.self_reliant, 0);
    assert_eq!(t.reselected_without_comparison, 1);
}

#[test]
fn wrong_without_correct_and_unanswered() {
    let w = rec(
        "s1",
        &["LO-2"],
        FirstAnswer::Wrong,
        Reselection::NotCorrect,
        true,
        0,
    );
    assert_eq!(classify(&w), Ok(FirstAnswerOutcome::NotCorrect));
    let u = rec(
        "s1",
        &["LO-2"],
        FirstAnswer::Unanswered,
        Reselection::None,
        false,
        0,
    );
    assert_eq!(classify(&u), Ok(FirstAnswerOutcome::Unanswered));
    let t = tally(&[w, u], "LO-2").expect("valid");
    assert_eq!(
        t,
        FirstAnswerTally {
            not_correct: 1,
            unanswered: 1,
            ..FirstAnswerTally::default()
        }
    );
}

#[test]
fn hint_stage_does_not_change_outcome() {
    for hint in 0..=3 {
        let a = rec(
            "s",
            &["LO-2"],
            FirstAnswer::Correct,
            Reselection::None,
            false,
            hint,
        );
        assert_eq!(classify(&a), Ok(FirstAnswerOutcome::SelfReliant));
        let b = rec(
            "s",
            &["LO-2"],
            FirstAnswer::Wrong,
            Reselection::Correct,
            true,
            hint,
        );
        assert_eq!(
            classify(&b),
            Ok(FirstAnswerOutcome::Reselected {
                after_comparison: true
            })
        );
    }
}

#[test]
fn from_history_observations() {
    let target = |step: &str| CheckTarget {
        step: step.to_string(),
        lo: vec!["LO-2".to_string()],
        method: CheckMethod::PointAt,
    };
    let ans = |step: &str, correct: bool| CheckObservation::Answered {
        step: step.to_string(),
        correct,
    };
    let obs = vec![
        ans("a", true),
        ans("b", false),
        CheckObservation::ComparisonShown {
            step: "b".to_string(),
        },
        ans("b", true),
    ];
    let records = derive_records(&[target("a"), target("b")], &obs);
    assert_eq!(classify(&records[0]), Ok(FirstAnswerOutcome::SelfReliant));
    assert_eq!(
        classify(&records[1]),
        Ok(FirstAnswerOutcome::Reselected {
            after_comparison: true
        })
    );
    let t = tally(&records, "LO-2").expect("valid");
    assert_eq!(t.self_reliant, 1);
    assert_eq!(t.reselected_after_comparison, 1);
}

#[test]
fn tally_counts_only_the_designated_lo() {
    let records = vec![
        correct_first("s1", &["LO-1"]),
        correct_first("s2", &["LO-2"]),
        correct_first("s3", &["LO-3"]),
        correct_first("s4", &["LO-1", "LO-2"]),
        reselected("s5", &["LO-2"], true),
    ];
    let t = tally(&records, "LO-2").expect("valid");
    assert_eq!(t.self_reliant, 2);
    assert_eq!(t.reselected_after_comparison, 1);
    let t1 = tally(&records, "LO-1").expect("valid");
    assert_eq!(t1.self_reliant, 2);
    assert_eq!(tally(&records, "LO-9"), Ok(FirstAnswerTally::default()));
}

#[test]
fn invalid_record_is_rejected() {
    let bad = rec(
        "s",
        &["LO-2"],
        FirstAnswer::Correct,
        Reselection::Correct,
        false,
        0,
    );
    assert!(classify(&bad).is_err());
    let empty_lo = rec("s", &[], FirstAnswer::Correct, Reselection::None, false, 0);
    assert!(classify(&empty_lo).is_err());
    let missing = rec(
        "s",
        &["LO-2"],
        FirstAnswer::Wrong,
        Reselection::None,
        false,
        0,
    );
    assert!(classify(&missing).is_err());
    // 指定の LO と無関係な不正な記録も読み飛ばさない。
    let ok = correct_first("s1", &["LO-2"]);
    assert!(tally(&[ok, empty_lo], "LO-2").is_err());
    assert!(tally(&[bad], "LO-2").is_err());
}

#[test]
fn contract_samples_classify_as_expected() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../contract/tests/data/lo_check_1/records.json"
    );
    let text = std::fs::read_to_string(path).expect("contract sample must exist");
    let records: Vec<LoCheckRecord> = serde_json::from_str(&text).expect("valid records");
    let s2 = records.iter().find(|r| r.step == "main/s2").expect("s2");
    assert_eq!(
        classify(s2),
        Ok(FirstAnswerOutcome::Reselected {
            after_comparison: true
        })
    );
    let s1 = records.iter().find(|r| r.step == "main/s1").expect("s1");
    assert_eq!(classify(s1), Ok(FirstAnswerOutcome::SelfReliant));
}

fn valid_record() -> impl Strategy<Value = LoCheckRecord> {
    let los = proptest::sample::subsequence(vec!["LO-1", "LO-2", "LO-3"], 1..=3);
    let state = prop_oneof![
        Just((FirstAnswer::Correct, Reselection::None, false)),
        Just((FirstAnswer::Unanswered, Reselection::None, false)),
        any::<bool>().prop_map(|c| (FirstAnswer::Wrong, Reselection::Correct, c)),
        any::<bool>().prop_map(|c| (FirstAnswer::Wrong, Reselection::NotCorrect, c)),
    ];
    (los, state, 0u32..4).prop_map(|(lo, (f, r, c), h)| rec("s", &lo, f, r, c, h))
}

#[test]
fn self_reliant_iff_first_answer_correct() {
    check(
        "lo-first-answer-class",
        DEFAULT_CASES,
        valid_record(),
        |r| {
            let o = classify(&r).expect("valid");
            prop_assert_eq!(o.is_self_reliant(), r.first_answer == FirstAnswer::Correct);
            Ok(())
        },
    );
}

#[test]
fn tally_partitions_matching_records() {
    let strat = proptest::collection::vec(valid_record(), 0..12);
    check("lo-first-answer-tally", DEFAULT_CASES, strat, |rs| {
        let t = tally(&rs, "LO-2").expect("valid");
        let matching: Vec<_> = rs
            .iter()
            .filter(|r| r.lo.iter().any(|l| l == "LO-2"))
            .collect();
        let sum = t.self_reliant
            + t.reselected_after_comparison
            + t.reselected_without_comparison
            + t.not_correct
            + t.unanswered;
        prop_assert_eq!(sum as usize, matching.len());
        let firsts = matching
            .iter()
            .filter(|r| r.first_answer == FirstAnswer::Correct)
            .count();
        prop_assert_eq!(t.self_reliant as usize, firsts);
        Ok(())
    });
}
