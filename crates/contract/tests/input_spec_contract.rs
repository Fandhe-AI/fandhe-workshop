//! 入力方式の宣言の型と `text` の判定の契約の適合試験（TASK-44、PACK-10）。
//!
//! 語彙の文書との一致は `input_spec_vocabulary.rs`、ここでは型の形と判定の境界を確かめる。

use fandhe_workshop_contract::input_spec::*;
use fandhe_workshop_contract::judgment::{Category, JudgmentKind};
use serde_json::json;

fn text(min: u32, max: u32) -> TextSpec {
    TextSpec { min, max }
}

#[test]
fn text_spec_has_only_min_and_max_fields() {
    let v = serde_json::to_value(text(1, 10)).expect("serialize");
    let mut keys: Vec<&String> = v.as_object().expect("object").keys().collect();
    keys.sort();
    assert_eq!(keys, ["max", "min"]);
}

#[test]
fn text_spec_rejects_content_judging_fields() {
    for field in ["contains", "pattern", "regex", "keywords", "expected"] {
        let v = json!({"min": 1, "max": 10, field: "x"});
        assert!(
            serde_json::from_value::<TextSpec>(v).is_err(),
            "{field} must be rejected"
        );
    }
}

#[test]
fn empty_and_blank_inputs_are_empty() {
    let t = text(0, 10);
    for s in ["", "   ", "\n\t", "\u{3000}"] {
        assert_eq!(t.judge(s), TextVerdict::Empty, "{s:?}");
    }
}

#[test]
fn lower_and_upper_bounds_are_inclusive() {
    let t = text(3, 5);
    assert_eq!(t.judge("ab"), TextVerdict::TooShort);
    assert_eq!(t.judge("abc"), TextVerdict::Accepted);
    assert_eq!(t.judge("abcde"), TextVerdict::Accepted);
    assert_eq!(t.judge("abcdef"), TextVerdict::TooLong);
}

#[test]
fn min_equals_max() {
    let t = text(2, 2);
    assert_eq!(t.judge("a"), TextVerdict::TooShort);
    assert_eq!(t.judge("ab"), TextVerdict::Accepted);
    assert_eq!(t.judge("abc"), TextVerdict::TooLong);
}

#[test]
fn counts_code_points_not_bytes_or_utf16_units() {
    let t = text(1, 3);
    assert_eq!(t.judge("あいう"), TextVerdict::Accepted);
    assert_eq!(t.judge("あいうえ"), TextVerdict::TooLong);
    // サロゲートペアになる絵文字は 1 文字。
    assert_eq!(t.judge("😀😀😀"), TextVerdict::Accepted);
    assert_eq!(t.judge("😀😀😀😀"), TextVerdict::TooLong);
}

#[test]
fn outer_whitespace_is_not_counted_but_inner_is() {
    let t = text(3, 3);
    assert_eq!(t.judge("  ab  "), TextVerdict::TooShort);
    assert_eq!(t.judge("a\nb"), TextVerdict::Accepted);
    assert_eq!(t.judge("a b "), TextVerdict::Accepted);
}

#[test]
fn zero_min_still_rejects_empty() {
    assert_eq!(text(0, 5).judge(""), TextVerdict::Empty);
    assert_eq!(text(0, 5).judge("a"), TextVerdict::Accepted);
}

#[test]
fn huge_input_is_judged_without_counting_past_the_limit() {
    let big = "a".repeat(1_000_000);
    assert_eq!(text(1, 10).judge(&big), TextVerdict::TooLong);
}

#[test]
fn verdict_maps_to_kind_and_category() {
    assert_eq!(TextVerdict::Accepted.kind(), JudgmentKind::Accepted);
    assert_eq!(TextVerdict::Accepted.category(), Category::FreeTextUnjudged);
    for v in [
        TextVerdict::Empty,
        TextVerdict::TooShort,
        TextVerdict::TooLong,
    ] {
        assert_eq!(v.kind(), JudgmentKind::Rejected);
        assert_eq!(v.category(), Category::BadArgShape);
    }
}

#[test]
fn judgment_does_not_look_at_content() {
    let t = text(5, 20);
    let sensible = "this plan is right";
    let nonsense = "qwerty zxcv asdfgh";
    assert_eq!(sensible.chars().count(), nonsense.chars().count());
    assert_eq!(t.judge(sensible), t.judge(nonsense));
}

#[test]
fn text_check_rejects_min_above_max() {
    assert_eq!(text(5, 4).check(), Err(SpecError::MinAboveMax));
    assert_eq!(text(4, 4).check(), Ok(()));
}

#[test]
fn command_check() {
    let arg = |n: &str| ArgSpec {
        name: n.to_string(),
        positional: true,
        required: true,
        shape: ArgShape::Any,
    };
    let c = |name: &str, args: Vec<ArgSpec>| CommandSpec {
        name: name.to_string(),
        args,
    };
    assert_eq!(c("x", vec![arg("a"), arg("b")]).check(), Ok(()));
    assert_eq!(c(" ", vec![]).check(), Err(SpecError::EmptyName));
    assert_eq!(c("x", vec![arg("")]).check(), Err(SpecError::EmptyName));
    assert_eq!(
        c("x", vec![arg("a"), arg("a")]).check(),
        Err(SpecError::Duplicate)
    );
}

#[test]
fn command_arg_defaults_and_unknown_shape() {
    let a: ArgSpec = serde_json::from_value(json!({"name": "n"})).expect("defaults");
    assert!(!a.positional && !a.required);
    assert_eq!(a.shape, ArgShape::Any);
    let bad = json!({"name": "n", "shape": "email"});
    assert!(serde_json::from_value::<ArgSpec>(bad).is_err());
    let shape: ArgShape = serde_json::from_value(json!("kebab_case")).expect("kebab");
    assert_eq!(shape, ArgShape::KebabCase);
}

#[test]
fn choice_check() {
    let o = |id: &str, label: &str| ChoiceOption {
        id: id.to_string(),
        label: label.to_string(),
    };
    let ok = ChoiceSpec {
        options: vec![o("a", "A"), o("b", "B")],
    };
    assert_eq!(ok.check(), Ok(()));
    assert_eq!(
        ChoiceSpec { options: vec![] }.check(),
        Err(SpecError::EmptyList)
    );
    let dup = ChoiceSpec {
        options: vec![o("a", "A"), o("a", "B")],
    };
    assert_eq!(dup.check(), Err(SpecError::Duplicate));
    let empty = ChoiceSpec {
        options: vec![o("a", "")],
    };
    assert_eq!(empty.check(), Err(SpecError::EmptyName));
}

#[test]
fn ui_action_check() {
    let t = |v: &str, p: &str| UiTarget {
        view: v.to_string(),
        path: p.to_string(),
    };
    assert_eq!(
        UiActionSpec {
            targets: vec![t("v", "p")]
        }
        .check(),
        Ok(())
    );
    assert_eq!(
        UiActionSpec { targets: vec![] }.check(),
        Err(SpecError::EmptyList)
    );
    assert_eq!(
        UiActionSpec {
            targets: vec![t("", "p")]
        }
        .check(),
        Err(SpecError::EmptyName)
    );
}

#[test]
fn approval_takes_no_fields() {
    assert!(serde_json::from_value::<ApprovalSpec>(json!({})).is_ok());
    assert!(serde_json::from_value::<ApprovalSpec>(json!({"x": 1})).is_err());
}

#[test]
fn spec_errors_are_fixed_sentences() {
    for e in [
        SpecError::MinAboveMax,
        SpecError::EmptyName,
        SpecError::Duplicate,
        SpecError::EmptyList,
    ] {
        assert!(!e.to_string().is_empty());
    }
}

#[test]
fn wait_spec_has_no_fields() {
    assert!(serde_json::from_value::<WaitSpec>(json!({})).is_ok());
    assert!(serde_json::from_value::<WaitSpec>(json!({"extra": 1})).is_err());
}

#[test]
fn only_wait_is_an_extension_point() {
    assert_eq!(InputKind::Wait.extension_point(), Some("EP-1"));
    for k in InputKind::ALL.into_iter().filter(|k| *k != InputKind::Wait) {
        assert_eq!(k.extension_point(), None, "{}", k.as_str());
    }
    assert_eq!(CANCEL_RULE_KEY, "cancel");
}
