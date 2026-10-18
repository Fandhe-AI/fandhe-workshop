//! 条件の語彙（TASK-42 1/2）の解釈の試験。11 語の形・誤りの種類・入れ子の上限・診断に値を写さないこと。

use fandhe_workshop_contract::condition::{
    ChoseIs, CmpOp, Condition, ConditionParseError, CountIs, ParamIs, StepIs, StepState,
    WorldAttrIs,
};
use fandhe_workshop_contract::limits::MAX_JSON_DEPTH;
use serde_json::{json, Value};

fn parse(v: Value) -> Result<Condition, ConditionParseError> {
    Condition::from_value(&v)
}

#[test]
fn eleven_words_parse() {
    assert_eq!(
        parse(json!({"step": {"step": "s1", "state": "achieved"}})),
        Ok(Condition::Step(StepIs {
            step: "s1".into(),
            state: StepState::Achieved
        }))
    );
    assert_eq!(parse(json!({"flag": "f"})), Ok(Condition::Flag("f".into())));
    assert_eq!(
        parse(json!({"world_has": "a/b"})),
        Ok(Condition::WorldHas("a/b".into()))
    );
    assert_eq!(
        parse(json!({"world_attr": {"path": "p", "attr": "a", "eq": "v"}})),
        Ok(Condition::WorldAttr(WorldAttrIs {
            path: "p".into(),
            attr: "a".into(),
            eq: "v".into()
        }))
    );
    assert_eq!(
        parse(json!({"count": {"prefix": "x/", "where": {"k": "v"}, "op": ">=", "n": 2}})),
        Ok(Condition::Count(CountIs {
            prefix: "x/".into(),
            where_attrs: [("k".to_string(), "v".to_string())].into(),
            op: CmpOp::Ge,
            n: 2
        }))
    );
    assert_eq!(
        parse(json!({"chose": {"step": "s", "option": "o"}})),
        Ok(Condition::Chose(ChoseIs {
            step: "s".into(),
            option: "o".into()
        }))
    );
    assert_eq!(
        parse(json!({"all": [{"flag": "a"}, {"not": {"flag": "b"}}]})),
        Ok(Condition::All(vec![
            Condition::Flag("a".into()),
            Condition::Not(Box::new(Condition::Flag("b".into())))
        ]))
    );
    assert_eq!(parse(json!({"any": []})), Ok(Condition::Any(vec![])));
    assert_eq!(
        parse(json!({"param": {"name": "n", "eq": "v"}})),
        Ok(Condition::Param(ParamIs {
            name: "n".into(),
            eq: "v".into()
        }))
    );
    assert_eq!(
        parse(json!({"clarifying": "cl-1"})),
        Ok(Condition::Clarifying("cl-1".into()))
    );
}

#[test]
fn count_where_is_optional_and_all_five_ops_parse() {
    for (sym, op) in [
        ("<", CmpOp::Lt),
        ("<=", CmpOp::Le),
        ("==", CmpOp::Eq),
        (">=", CmpOp::Ge),
        (">", CmpOp::Gt),
    ] {
        let c = parse(json!({"count": {"prefix": "", "op": sym, "n": 0}})).expect("parse");
        match c {
            Condition::Count(k) => {
                assert_eq!(k.op, op);
                assert!(k.where_attrs.is_empty());
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}

#[test]
fn cmp_op_holds() {
    assert!(CmpOp::Lt.holds(1, 2) && !CmpOp::Lt.holds(2, 2));
    assert!(CmpOp::Le.holds(2, 2) && !CmpOp::Le.holds(3, 2));
    assert!(CmpOp::Eq.holds(2, 2) && !CmpOp::Eq.holds(1, 2));
    assert!(CmpOp::Ge.holds(2, 2) && !CmpOp::Ge.holds(1, 2));
    assert!(CmpOp::Gt.holds(3, 2) && !CmpOp::Gt.holds(2, 2));
}

#[test]
fn word_count_must_be_one() {
    assert_eq!(parse(json!({})), Err(ConditionParseError::WordCount(0)));
    assert_eq!(
        parse(json!({"flag": "a", "world_has": "b"})),
        Err(ConditionParseError::WordCount(2))
    );
}

#[test]
fn unknown_fields_and_code_fields_are_shape_errors() {
    for word in ["script", "code", "eval", "exec", "expr", "js", "if"] {
        let v = json!({ word: "x" });
        assert_eq!(
            parse(v),
            Err(ConditionParseError::Shape {
                unknown_field: Some(word.into())
            })
        );
    }
    assert!(matches!(
        parse(json!({"flag": "a", "script": "x"})),
        Err(ConditionParseError::Shape { .. })
    ));
    // 入れ子の中でも拒否する。
    assert!(matches!(
        parse(json!({"all": [{"not": {"script": "x"}}]})),
        Err(ConditionParseError::Shape { .. })
    ));
    // 本体の未知の欄。
    assert_eq!(
        parse(json!({"step": {"step": "s", "state": "achieved", "extra": 1}})),
        Err(ConditionParseError::Shape {
            unknown_field: Some("extra".into())
        })
    );
}

#[test]
fn type_mismatches_are_shape_errors() {
    for v in [
        json!("flag"),
        json!([{"flag": "a"}]),
        json!(null),
        json!({"flag": 1}),
        json!({"flag": null}),
        json!({"count": {"prefix": "", "op": "!=", "n": 1}}),
        json!({"count": {"prefix": "", "op": "<", "n": -1}}),
        json!({"count": {"prefix": "", "op": "<", "n": 1.5}}),
        json!({"step": {"step": "s", "state": "done"}}),
        json!({"all": {"flag": "a"}}),
        json!({"world_attr": {"path": "p", "attr": "a", "eq": 1}}),
    ] {
        assert!(
            matches!(
                Condition::from_value(&v),
                Err(ConditionParseError::Shape { .. })
            ),
            "{v}"
        );
    }
}

#[test]
fn empty_names_are_rejected_but_empty_prefix_and_eq_are_allowed() {
    for v in [
        json!({"flag": ""}),
        json!({"world_has": ""}),
        json!({"world_attr": {"path": "", "attr": "a", "eq": "v"}}),
        json!({"world_attr": {"path": "p", "attr": "", "eq": "v"}}),
        json!({"step": {"step": "", "state": "entered"}}),
        json!({"chose": {"step": "s", "option": ""}}),
        json!({"clarifying": ""}),
        json!({"param": {"name": "", "eq": "v"}}),
    ] {
        assert!(
            matches!(
                Condition::from_value(&v),
                Err(ConditionParseError::EmptyName(_))
            ),
            "{v}"
        );
    }
    assert!(parse(json!({"count": {"prefix": "", "op": "==", "n": 0}})).is_ok());
    assert!(parse(json!({"world_attr": {"path": "p", "attr": "a", "eq": ""}})).is_ok());
}

fn nested(levels: usize) -> Value {
    let mut v = json!({"flag": "f"});
    for _ in 0..levels {
        v = json!({"not": v});
    }
    v
}

#[test]
fn nesting_limit_is_enforced() {
    assert!(parse(nested(MAX_JSON_DEPTH)).is_ok());
    assert_eq!(
        parse(nested(MAX_JSON_DEPTH + 1)),
        Err(ConditionParseError::TooDeep)
    );
}

#[test]
fn display_never_contains_input_values() {
    let secret = "SECRET-VALUE-4711";
    let cases = [
        json!({"flag": 1, "x": secret}),
        json!({"flag": secret, "world_has": secret}),
        json!({"count": {"prefix": secret, "op": secret, "n": 1}}),
        json!({"step": {"step": secret, "state": secret}}),
        json!({format!("{secret} x"): "x"}),
        json!({"all": [{"flag": ""}], "any": [secret]}),
    ];
    for v in cases {
        let e = Condition::from_value(&v).expect_err("must fail");
        assert!(!e.to_string().contains(secret), "{e}");
    }
}
