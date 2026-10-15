//! 状態変化の語彙（要素・属性・フラグ）の解釈の試験（TASK-43 1/2、PACK-9）。

use fandhe_workshop_contract::effect::{Effect, EffectItem, EffectParseError};
use serde_json::json;

fn parse(v: serde_json::Value) -> Result<EffectItem, EffectParseError> {
    EffectItem::from_value(&v)
}

#[test]
fn pack9_all_seven_words_parse() {
    let put = parse(json!({"put": {"path": "a", "fixture": "f", "attrs": {"k": "v"}}})).unwrap();
    match put.effect {
        Effect::Put(p) => {
            assert_eq!(p.path, "a");
            assert_eq!(p.fixture.as_deref(), Some("f"));
            assert_eq!(p.attrs["k"], "v");
        }
        e => panic!("{e:?}"),
    }
    assert_eq!(
        parse(json!({"remove": "a"})).unwrap().effect,
        Effect::Remove("a".into())
    );
    assert!(matches!(
        parse(json!({"replace": {"path": "a"}})).unwrap().effect,
        Effect::Replace(_)
    ));
    assert!(matches!(
        parse(json!({"set_attr": {"path": "a", "attr": "s", "value": "v"}}))
            .unwrap()
            .effect,
        Effect::SetAttr(_)
    ));
    assert_eq!(
        parse(json!({"set_flag": "f"})).unwrap().effect,
        Effect::SetFlag("f".into())
    );
    assert_eq!(
        parse(json!({"clear_flag": "f"})).unwrap().effect,
        Effect::ClearFlag("f".into())
    );
    assert_eq!(
        parse(json!({"set_flag_global": "f"})).unwrap().effect,
        Effect::SetFlagGlobal("f".into())
    );
}

#[test]
fn pack9_put_fixture_and_attrs_are_optional() {
    match parse(json!({"put": {"path": "a"}})).unwrap().effect {
        Effect::Put(p) => {
            assert!(p.fixture.is_none());
            assert!(p.attrs.is_empty());
        }
        e => panic!("{e:?}"),
    }
}

#[test]
fn pack9_unknown_fields_are_rejected_with_the_name() {
    for (v, name) in [
        (json!({"put": {"path": "a"}, "script": "x"}), "script"),
        (json!({"put": {"path": "a", "code": "x"}}), "code"),
    ] {
        let e = parse(v).unwrap_err();
        assert_eq!(
            e,
            EffectParseError::Shape {
                unknown_field: Some(name.to_string())
            }
        );
    }
}

#[test]
fn pack9_word_count_must_be_one() {
    assert_eq!(
        parse(json!({})).unwrap_err(),
        EffectParseError::WordCount(0)
    );
    assert_eq!(
        parse(json!({"when": {"x": 1}})).unwrap_err(),
        EffectParseError::WordCount(0)
    );
    assert_eq!(
        parse(json!({"put": {"path": "a"}, "remove": "a"})).unwrap_err(),
        EffectParseError::WordCount(2)
    );
}

#[test]
fn pack9_type_mismatch_is_a_shape_error() {
    let e = parse(json!({"set_attr": {"path": "a", "attr": "s", "value": 1}})).unwrap_err();
    assert_eq!(
        e,
        EffectParseError::Shape {
            unknown_field: None
        }
    );
    assert!(matches!(
        parse(json!("x")).unwrap_err(),
        EffectParseError::Shape { .. }
    ));
}

#[test]
fn pack9_empty_names_are_rejected() {
    assert_eq!(
        parse(json!({"put": {"path": ""}})).unwrap_err(),
        EffectParseError::EmptyName("path")
    );
    assert_eq!(
        parse(json!({"set_attr": {"path": "a", "attr": "", "value": "v"}})).unwrap_err(),
        EffectParseError::EmptyName("attr")
    );
    assert_eq!(
        parse(json!({"set_flag": ""})).unwrap_err(),
        EffectParseError::EmptyName("flag")
    );
}

#[test]
fn pack9_empty_attr_keys_in_put_and_replace_are_rejected() {
    for word in ["put", "replace"] {
        assert_eq!(
            parse(json!({word: {"path": "a", "attrs": {"": "v"}}})).unwrap_err(),
            EffectParseError::EmptyName("attr")
        );
    }
}

#[test]
fn pack9_when_is_kept_as_value() {
    let w = json!({"flag": "x"});
    let item = parse(json!({"set_flag": "f", "when": w})).unwrap();
    assert_eq!(item.when, Some(w));
}

#[test]
fn pack9_errors_do_not_carry_input_values() {
    let e = parse(json!({"set_attr": {"path": "a", "attr": "s", "value": ["SECRET-VALUE"]}}))
        .unwrap_err();
    assert!(!format!("{e:?} {e}").contains("SECRET-VALUE"));
    let e = parse(json!({"put": {"path": "a"}, "sec ret `x": "SECRET-VALUE"})).unwrap_err();
    assert!(!format!("{e:?} {e}").contains("SECRET-VALUE"));
}

#[test]
fn pack9_positional_array_forms_are_rejected() {
    for v in [
        json!(["put"]),
        json!({"put": ["a", null, {}]}),
        json!({"replace": ["a", null, {}]}),
        json!({"set_attr": ["a", "b", "c"]}),
    ] {
        assert!(
            matches!(parse(v.clone()), Err(EffectParseError::Shape { .. })),
            "{v}"
        );
    }
}

#[test]
fn pack9_null_word_is_not_treated_as_unspecified() {
    assert!(matches!(
        parse(json!({"put": {"path": "a"}, "remove": null})),
        Err(EffectParseError::Shape { .. })
    ));
    assert!(matches!(
        parse(json!({"remove": null})),
        Err(EffectParseError::Shape { .. })
    ));
}

// ---- 進行（processing）と when の形（TASK-43 2/2） ----

use fandhe_workshop_contract::effect::{ProcessingParseError, ProcessingTick};

fn tick(v: serde_json::Value) -> Result<ProcessingTick, ProcessingParseError> {
    ProcessingTick::from_value(&v)
}

#[test]
fn pack9_processing_tick_parses_with_when() {
    let t = tick(json!({"name": "scan", "effects": [
        {"set_flag": "f"},
        {"put": {"path": "a"}, "when": {"flag": "f"}}
    ]}))
    .unwrap();
    assert_eq!(t.name, "scan");
    assert_eq!(t.effects.len(), 2);
    assert_eq!(t.effects[1].when, Some(json!({"flag": "f"})));
    // effects 省略は空列。
    assert!(tick(json!({"name": "n"})).unwrap().effects.is_empty());
}

#[test]
fn pack9_processing_tick_rejects_bad_shapes() {
    assert_eq!(
        tick(json!({"name": "n", "script": "x"})),
        Err(ProcessingParseError::Shape {
            unknown_field: Some("script".into())
        })
    );
    for v in [
        json!("s"),
        json!([1]),
        json!({"effects": []}),
        json!({"name": 1}),
        json!({"name": "n", "effects": {"a": 1}}),
    ] {
        assert_eq!(
            tick(v),
            Err(ProcessingParseError::Shape {
                unknown_field: None
            })
        );
    }
    assert_eq!(
        tick(json!({"name": ""})),
        Err(ProcessingParseError::EmptyName("name"))
    );
    assert!(matches!(
        tick(json!({"name": "n", "effects": [{"set_flag": "f"}, {}]})),
        Err(ProcessingParseError::Effect { index: 1, .. })
    ));
}

#[test]
fn pack9_when_must_be_a_table() {
    for w in [json!("x"), json!(1), json!([1]), json!(null)] {
        assert_eq!(
            parse(json!({"set_flag": "f", "when": w})),
            Err(EffectParseError::Shape {
                unknown_field: None
            })
        );
    }
    assert!(parse(json!({"set_flag": "f", "when": {"any": 1}})).is_ok());
}

#[test]
fn pack9_processing_errors_do_not_echo_input() {
    let e = tick(json!({"name": "SECRET-VALUE", "effects": 1})).unwrap_err();
    assert!(!format!("{e:?}{e}").contains("SECRET-VALUE"));
    let e = tick(json!({"name": "n", "effects": [{"set_flag": 1, "when": "SECRET-VALUE"}]}))
        .unwrap_err();
    assert!(!format!("{e:?}{e}").contains("SECRET-VALUE"));
}
