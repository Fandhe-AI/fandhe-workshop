//! 学習目標の確認の記録（TASK-291、版 1）の適合試験（GUIDE-11・GUIDE-12・GUIDE-18 の型の水準）。
//!
//! 直列化の欄の順、不変条件、導出の入出力を `tests/data/lo_check_1/` のサンプルで固定する。
//! サンプルの書き換えは契約の変更であり、`LO_CHECK_CONTRACT_VERSION` の版上げの判断が要る（spec を先に直す）。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::path::Path;

use fandhe_workshop_contract::lo_check::*;
use serde::Deserialize;
use serde_json::{json, Value};

fn data(name: &str) -> String {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/lo_check_1")
        .join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|_| panic!("cannot read sample {name}"))
}

#[derive(Deserialize)]
struct Case {
    name: String,
    targets: Vec<CheckTarget>,
    observations: Vec<CheckObservation>,
    expected: Vec<LoCheckRecord>,
}

fn cases() -> Vec<Case> {
    serde_json::from_str(&data("derive.json")).unwrap()
}

fn rec_json(first: &str, resel: &str, cmp: bool) -> Value {
    json!({
        "step": "s", "lo": ["LO-1"], "method": "answer", "first_answer": first,
        "reselection": resel, "after_comparison": cmp, "max_hint": 0
    })
}

#[test]
fn version_is_one() {
    assert_eq!(LO_CHECK_CONTRACT_VERSION, 1);
}

#[test]
fn records_golden_roundtrips_and_fixes_field_order() {
    let src = data("records.json");
    let recs: Vec<LoCheckRecord> = serde_json::from_str(&src).unwrap();
    assert_eq!(recs.len(), 3);
    let keys = [
        "step",
        "lo",
        "method",
        "first_answer",
        "reselection",
        "after_comparison",
        "max_hint",
    ];
    for r in &recs {
        r.validate().unwrap();
        let s = serde_json::to_string(r).unwrap();
        let mut last = 0;
        for k in keys {
            let at = s.find(&format!("\"{k}\"")).unwrap();
            assert!(at >= last, "欄の順が崩れた: {k}");
            last = at;
        }
    }
    let again: Vec<Value> = recs
        .iter()
        .map(|r| serde_json::to_value(r).unwrap())
        .collect();
    let orig: Vec<Value> = serde_json::from_str(&src).unwrap();
    assert_eq!(again, orig);
}

#[test]
fn deserialization_rejects_unknown_fields_and_values() {
    let mut v = rec_json("correct", "none", false);
    assert!(serde_json::from_value::<LoCheckRecord>(v.clone()).is_ok());
    v["extra"] = json!(1);
    assert!(serde_json::from_value::<LoCheckRecord>(v).is_err());
    for (k, x) in [
        ("first_answer", "maybe"),
        ("reselection", "later"),
        ("method", "none"),
    ] {
        let mut v = rec_json("correct", "none", false);
        v[k] = json!(x);
        assert!(serde_json::from_value::<LoCheckRecord>(v).is_err(), "{k}");
    }
}

#[test]
fn deserialization_rejects_invariant_violations() {
    for (f, r, c) in [
        ("correct", "correct", false),
        ("correct", "none", true),
        ("unanswered", "not_correct", false),
        ("wrong", "none", false),
    ] {
        assert!(
            serde_json::from_value::<LoCheckRecord>(rec_json(f, r, c)).is_err(),
            "{f} {r} {c}"
        );
    }
    let mut v = rec_json("correct", "none", false);
    v["step"] = json!("");
    assert!(serde_json::from_value::<LoCheckRecord>(v).is_err());
    let mut v = rec_json("correct", "none", false);
    v["lo"] = json!([]);
    assert!(serde_json::from_value::<LoCheckRecord>(v).is_err());
}

#[test]
fn error_display_is_fixed_text() {
    let errs = [
        LoCheckError::EmptyStep,
        LoCheckError::EmptyLo,
        LoCheckError::ReselectionWithoutWrong,
        LoCheckError::MissingReselection,
    ];
    for e in errs {
        assert!(e.to_string().is_ascii());
    }
}

#[test]
fn derive_cases_match_samples() {
    let cs = cases();
    assert!(cs.len() >= 10);
    for c in cs {
        let got = derive_records(&c.targets, &c.observations);
        assert_eq!(got, c.expected, "case {}", c.name);
        for r in &got {
            r.validate().unwrap();
        }
    }
}

#[test]
fn reselection_after_comparison_is_kept_apart_from_first_answer() {
    let c = cases().into_iter().find(|c| c.name == "d").unwrap();
    let got = derive_records(&c.targets, &c.observations);
    assert_eq!(got[0].first_answer, FirstAnswer::Wrong);
    assert_eq!(got[0].reselection, Reselection::Correct);
    assert!(got[0].after_comparison);
}

#[test]
fn derived_records_always_validate() {
    let mut alphabet: Vec<CheckObservation> = Vec::new();
    for s in ["s", "x"] {
        alphabet.push(CheckObservation::Answered {
            step: s.into(),
            correct: true,
        });
        alphabet.push(CheckObservation::Answered {
            step: s.into(),
            correct: false,
        });
        alphabet.push(CheckObservation::ComparisonShown { step: s.into() });
        alphabet.push(CheckObservation::HintOpened {
            step: s.into(),
            opened: 2,
        });
    }
    let targets = [
        CheckTarget {
            step: "s".into(),
            lo: vec!["LO-1".into()],
            method: CheckMethod::Answer,
        },
        CheckTarget {
            step: "none".into(),
            lo: vec!["LO-2".into()],
            method: CheckMethod::Approve,
        },
    ];
    let n = alphabet.len();
    let mut checked = 0usize;
    for len in 0..=4u32 {
        for mut code in 0..n.pow(len) {
            let mut obs = Vec::new();
            for _ in 0..len {
                obs.push(alphabet[code % n].clone());
                code /= n;
            }
            let out = derive_records(&targets, &obs);
            assert_eq!(out.len(), targets.len());
            for r in out {
                r.validate().unwrap();
            }
            checked += 1;
        }
    }
    assert!(checked > 4000);
}

#[test]
fn check_method_all_and_as_str_roundtrip() {
    for m in CheckMethod::ALL {
        let v = serde_json::to_value(m).unwrap();
        assert_eq!(v, json!(m.as_str()));
        assert_eq!(serde_json::from_value::<CheckMethod>(v).unwrap(), m);
    }
}

#[test]
fn derive_skips_targets_that_would_make_invalid_records() {
    let targets = [
        CheckTarget {
            step: String::new(),
            lo: vec!["LO-1".into()],
            method: CheckMethod::Answer,
        },
        CheckTarget {
            step: "s".into(),
            lo: vec!["LO-1".into()],
            method: CheckMethod::Answer,
        },
        CheckTarget {
            step: "t".into(),
            lo: vec![],
            method: CheckMethod::Approve,
        },
    ];
    let out = derive_records(&targets, &[]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].step, "s");
    for r in &out {
        r.validate().unwrap();
        let json = serde_json::to_string(r).unwrap();
        let back: LoCheckRecord = serde_json::from_str(&json).unwrap();
        assert_eq!(&back, r);
    }
}
