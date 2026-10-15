//! 案内レベルとヒントの型（TASK-290、版 1）の適合試験（GUIDE-3・GUIDE-17 の型の水準）。
//!
//! 直列化の綴り、`visible_hints` の規則（fail-closed）、全域性を、`tests/data/guidance_1/` のサンプルで固定する。
//! サンプルの書き換えは契約の変更であり、`GUIDANCE_CONTRACT_VERSION` の版上げの判断が要る（spec を先に直す）。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;
use std::path::Path;

use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel, GUIDANCE_CONTRACT_VERSION};
use fandhe_workshop_contract::ir::{Ir, Step};
use serde_json::{json, Value};

fn data(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/guidance_1")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("cannot read sample {name}"))
}

fn sample_ir() -> Ir {
    serde_json::from_slice(&data("hints-ir.json")).expect("サンプルが Ir として読めること")
}

fn ir_with(hints: Vec<(&str, Value)>) -> Ir {
    let mut ir = sample_ir();
    ir.hints = hints
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect::<BTreeMap<_, _>>();
    ir
}

fn step_with(ids: &[&str]) -> Step {
    serde_json::from_value(json!({"id": "s", "title": "t", "hints": ids})).unwrap()
}

#[test]
fn version_is_one() {
    assert_eq!(GUIDANCE_CONTRACT_VERSION, 1);
}

#[test]
fn level_serializes_as_letters_and_roundtrips() {
    for (l, s) in [
        (GuidanceLevel::A, "\"A\""),
        (GuidanceLevel::B, "\"B\""),
        (GuidanceLevel::C, "\"C\""),
    ] {
        assert_eq!(serde_json::to_string(&l).unwrap(), s);
        assert_eq!(serde_json::from_str::<GuidanceLevel>(s).unwrap(), l);
    }
}

#[test]
fn level_rejects_unknown_values() {
    for bad in ["\"a\"", "\"D\"", "\"AA\"", "1", "null", "\"\""] {
        assert!(serde_json::from_str::<GuidanceLevel>(bad).is_err(), "{bad}");
    }
}

#[test]
fn parse_as_str_all_and_default() {
    for l in GuidanceLevel::ALL {
        assert_eq!(GuidanceLevel::parse(l.as_str()), Some(l));
        assert_eq!(l.to_string(), l.as_str());
    }
    assert_eq!(
        GuidanceLevel::ALL,
        [GuidanceLevel::A, GuidanceLevel::B, GuidanceLevel::C]
    );
    for bad in ["", "a", "D", "AB", " A"] {
        assert_eq!(GuidanceLevel::parse(bad), None, "{bad:?}");
    }
    assert_eq!(GuidanceLevel::default(), GuidanceLevel::A);
}

#[test]
fn fallback_chain_goes_toward_denser_levels() {
    use GuidanceLevel::{A, B, C};
    assert_eq!(A.fallback_chain(), &[A]);
    assert_eq!(B.fallback_chain(), &[B, A]);
    assert_eq!(C.fallback_chain(), &[C, B, A]);
}

#[test]
fn visible_hints_matches_sample_expectation() {
    let ir = sample_ir();
    let expected: BTreeMap<String, Vec<String>> =
        serde_json::from_slice(&data("visible.json")).unwrap();
    let step = &ir.nodes[0].step;
    for l in GuidanceLevel::ALL {
        let got: Vec<&str> = visible_hints(&ir, step, l);
        let want: Vec<&str> = expected[l.as_str()].iter().map(String::as_str).collect();
        assert_eq!(got, want, "{l}");
    }
}

#[test]
fn empty_levels_means_all_levels() {
    let ir = ir_with(vec![("h", json!({"levels": []}))]);
    let step = step_with(&["h"]);
    for l in GuidanceLevel::ALL {
        assert_eq!(visible_hints(&ir, &step, l), vec!["h"], "{l}");
    }
}

#[test]
fn missing_definition_and_malformed_levels_are_not_counted() {
    let ir = ir_with(vec![
        ("not-object", json!("x")),
        ("levels-string", json!({"levels": "A"})),
        ("levels-null", json!({"levels": null})),
        ("levels-object", json!({"levels": {"A": 1}})),
        ("levels-number", json!({"levels": 1})),
        ("elem-number", json!({"levels": ["A", 1]})),
        ("elem-nested", json!({"levels": [["A"]]})),
        ("elem-unknown", json!({"levels": ["A", "D"]})),
        ("elem-lower", json!({"levels": ["a"]})),
    ]);
    let step = step_with(&[
        "absent",
        "not-object",
        "levels-string",
        "levels-null",
        "levels-object",
        "levels-number",
        "elem-number",
        "elem-nested",
        "elem-unknown",
        "elem-lower",
    ]);
    for l in GuidanceLevel::ALL {
        assert!(visible_hints(&ir, &step, l).is_empty(), "{l}");
    }
}

#[test]
fn no_hints_in_step_is_zero() {
    let ir = sample_ir();
    let step = step_with(&[]);
    for l in GuidanceLevel::ALL {
        assert_eq!(visible_hints(&ir, &step, l).len(), 0);
    }
}

#[test]
fn order_follows_the_step_not_the_table() {
    let ir = ir_with(vec![("a", json!({})), ("b", json!({}))]);
    let step = step_with(&["b", "a"]);
    assert_eq!(visible_hints(&ir, &step, GuidanceLevel::A), vec!["b", "a"]);
}
