//! コマンド入力の規則の照合と問い返しの判定（TASK-272、JUDGE-6）の試験。
//!
//! `st-1-4`（引数なしの `/new-idea` を問い返しにする Step）は 2 つの形で試す。1 つは
//! `testdata/input-field-ime/test-data/ch1-pack.json` の実データ（使わない照合キーを `null` で並べる古い形）、
//! もう 1 つは `tests/data/command_rule_1/steps.json` の手書きの短い形（使うキーだけを持つ pack の golden の形）で、
//! 両方で同じ結果になることを確かめる。試験データの宣言は試作の教材（PoC-3）の形の写しで、
//! `docs/spec/` は private のため試験からは直接読まない。

mod common;

use std::collections::BTreeMap;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{JudgmentKind, RejectCategory};
use fandhe_workshop_core::command_judge::CommandVerdict;
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::command_rule::{
    judge_command_step, CommandRuleError, CommandRuleVerdict, CommandStepVerdict,
};
use fandhe_workshop_core::load_ir;
use proptest::prelude::*;
use serde_json::{json, Value};

fn compact_steps() -> BTreeMap<String, Step> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/command_rule_1/steps.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn compact(name: &str) -> Step {
    compact_steps()
        .remove(name)
        .unwrap_or_else(|| panic!("{name}"))
}

fn ch1_pack_st_1_4() -> Step {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/input-field-ime/test-data/ch1-pack.json"
    );
    let ir = load_ir(&std::fs::read(path).unwrap()).unwrap();
    ir.nodes
        .into_iter()
        .find(|n| n.uid == "main/st-1-4")
        .expect("st-1-4")
        .step
}

fn never(_: &Value) -> Option<bool> {
    None
}

fn judge(step: &Step, line: &str) -> Result<CommandStepVerdict, CommandRuleError> {
    let lexed = lex_command("/", line).expect("lexable");
    judge_command_step(step, &lexed, &never)
}

fn assert_clarified(v: Result<CommandStepVerdict, CommandRuleError>) {
    let v = v.expect("valid declaration");
    match &v {
        CommandStepVerdict::Rule(CommandRuleVerdict::Clarified {
            rule_id,
            clarification,
            ..
        }) => {
            assert_eq!(rule_id, "r-ask-name");
            assert_eq!(clarification, "cl-new-idea-name");
        }
        other => panic!("expected clarified, got {other:?}"),
    }
    assert_eq!(v.kind(), Some(JudgmentKind::Clarified));
    assert_ne!(v.kind(), Some(JudgmentKind::Rejected));
    assert_eq!(v.reject_category(), None);
}

#[test]
fn clarify_on_missing_name_ch1_pack() {
    assert_clarified(judge(&ch1_pack_st_1_4(), "/new-idea"));
}

#[test]
fn clarify_on_missing_name_compact() {
    assert_clarified(judge(&compact("ask"), "/new-idea"));
}

#[test]
fn clarify_is_not_counted_as_rejection() {
    let inputs = ["/new-idea", "/new-idea Bad_Name", "/new-idea my-idea"];
    for step in [ch1_pack_st_1_4(), compact("ask")] {
        let (mut clarified, mut rejected, mut accepted) = (0, 0, 0);
        for line in inputs {
            let v = judge(&step, line).unwrap();
            match v.kind() {
                Some(JudgmentKind::Clarified) => clarified += 1,
                Some(JudgmentKind::Rejected) => rejected += 1,
                Some(JudgmentKind::Accepted) => accepted += 1,
                _ => {}
            }
        }
        // 問い返しは 1 件、拒否は形の誤りの 1 件だけで、問い返しは拒否に数えない。受付にもならない。
        assert_eq!((clarified, rejected, accepted), (1, 1, 0));
    }
}

#[test]
fn missing_required_is_rejection_not_clarify() {
    let step = compact("required");
    let v = judge(&step, "/new-idea").unwrap();
    assert_eq!(v, CommandStepVerdict::Declared(CommandVerdict::MissingArg));
    assert_eq!(v.kind(), Some(JudgmentKind::Rejected));
    assert_eq!(v.reject_category(), Some(RejectCategory::MissingArg));
}

#[test]
fn named_arg_present_does_not_clarify() {
    let step = compact("ask");
    assert_eq!(
        judge(&step, "/new-idea my-idea").unwrap(),
        CommandStepVerdict::Rule(CommandRuleVerdict::NoRule)
    );
    assert_eq!(
        judge(&step, "/new-idea Bad_Name").unwrap(),
        CommandStepVerdict::Declared(CommandVerdict::BadArgShape)
    );
    assert_eq!(
        judge(&step, "/other").unwrap(),
        CommandStepVerdict::Declared(CommandVerdict::NameMismatch)
    );
}

#[test]
fn cancel_rule_is_skipped_for_commands() {
    // `r-cancel` は `on.cancel` だけの規則。コマンドの入力には一致しない。
    let step = compact("ask");
    let v = judge(&step, "/new-idea my-idea").unwrap();
    assert_eq!(v, CommandStepVerdict::Rule(CommandRuleVerdict::NoRule));
}

#[test]
fn first_matching_rule_wins() {
    let step = compact("order");
    // `when` が `None` の `r-cond` と、名前の違う `r-other-name` は飛ばされ、`r-with-name`（空の missing）が勝つ。
    match judge(&step, "/new-idea").unwrap() {
        CommandStepVerdict::Rule(CommandRuleVerdict::RejectedByRule {
            rule_id, rejection, ..
        }) => {
            assert_eq!(rule_id, "r-with-name");
            assert_eq!(rejection, "rj-any");
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn when_true_takes_rule_and_false_or_unknown_skips() {
    let step = compact("order");
    let lexed = lex_command("/", "/new-idea").unwrap();
    let truthy = |_: &Value| Some(true);
    let falsy = |_: &Value| Some(false);
    let rule_id = |v: CommandStepVerdict| match v {
        CommandStepVerdict::Rule(CommandRuleVerdict::Accepted { rule_id, .. }) => rule_id,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        rule_id(judge_command_step(&step, &lexed, &truthy).unwrap()),
        "r-cond"
    );
    for skip in [&falsy as &dyn Fn(&Value) -> Option<bool>, &never] {
        assert!(matches!(
            judge_command_step(&step, &lexed, skip).unwrap(),
            CommandStepVerdict::Rule(CommandRuleVerdict::RejectedByRule { .. })
        ));
    }
}

#[test]
fn on_args_matches_only_equal_values_and_unknown_keys_never_match() {
    let rule_of = |name: &str, line: &str| match judge(&compact(name), line) {
        Ok(CommandStepVerdict::Rule(r)) => r,
        other => panic!("{other:?}"),
    };
    assert!(matches!(
        rule_of("args_eq", "/x b"),
        CommandRuleVerdict::Accepted { .. }
    ));
    for line in ["/x c", "/x"] {
        assert_eq!(rule_of("args_eq", line), CommandRuleVerdict::NoRule);
    }
    assert_eq!(rule_of("unknown_key", "/x"), CommandRuleVerdict::NoRule);
}

#[test]
fn invalid_declarations_fail_closed() {
    let cases = [
        ("bad_kind", "/x", CommandRuleError::InvalidDeclaration),
        ("bad_missing", "/x", CommandRuleError::InvalidDeclaration),
        (
            "bad_clarification",
            "/x",
            CommandRuleError::InvalidDeclaration,
        ),
        ("args_cond", "/x", CommandRuleError::InvalidDeclaration),
        (
            "args_not_string",
            "/x",
            CommandRuleError::UnsupportedCondition,
        ),
        ("not_command", "/x", CommandRuleError::NotCommand),
    ];
    for (name, line, want) in cases {
        assert_eq!(judge(&compact(name), line), Err(want), "{name}");
    }
    for e in [
        CommandRuleError::NotCommand,
        CommandRuleError::InvalidDeclaration,
        CommandRuleError::UnsupportedCondition,
    ] {
        let text = e.to_string();
        assert!(!text.contains("ghost") && !text.contains("defer"));
    }
}

#[test]
fn rule_shape_errors_are_invalid_declarations() {
    let mut step = compact("ask");
    for rules in [
        json!(["not-an-object"]),
        json!([{ "on": { "command": "new-idea" }, "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": "x", "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": { "command": 1 }, "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": { "command": "new-idea", "missing": "name" }, "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": { "command": "new-idea", "missing": [1] }, "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": { "command": "new-idea", "args": 1 }, "outcome": { "kind": "accept" } }]),
        json!([{ "id": "r", "on": { "command": "new-idea" }, "outcome": "accept" }]),
        json!([{ "id": "r", "on": { "command": "new-idea" }, "outcome": { "kind": "reject" } }]),
    ] {
        step.rules = rules.as_array().unwrap().clone();
        assert_eq!(
            judge(&step, "/new-idea"),
            Err(CommandRuleError::InvalidDeclaration),
            "{rules}"
        );
    }
}

/// 規則の並びの部品。
fn rule_strategy() -> impl Strategy<Value = Value> {
    (0u8..3, 0u8..3, any::<bool>()).prop_map(|(kind, missing, null_noise)| {
        let outcome = match kind {
            0 => json!({ "kind": "accept" }),
            1 => json!({ "kind": "reject", "rejection": "rj" }),
            _ => json!({ "kind": "clarify", "clarification": "cl" }),
        };
        let mut on = serde_json::Map::new();
        on.insert("command".into(), json!("cmd"));
        match missing {
            0 => {}
            1 => {
                on.insert("missing".into(), json!(["a"]));
            }
            _ => {
                on.insert("missing".into(), json!([]));
            }
        }
        if null_noise {
            on.insert("option".into(), Value::Null);
            on.insert("cancel".into(), Value::Null);
        }
        json!({ "id": "r", "on": on, "outcome": outcome })
    })
}

#[test]
fn clarify_properties() {
    let strat = (
        proptest::collection::vec(rule_strategy(), 0..6),
        any::<bool>(),
        "\\PC{0,12}",
    );
    check(
        "command-clarify",
        DEFAULT_CASES,
        strat,
        |(rules, with_arg, junk)| {
            let step: Step = serde_json::from_value(json!({
                "id": "s",
                "title": "t",
                "input": { "kind": "command", "name": "cmd",
                    "args": [{ "name": "a", "positional": true, "required": false, "shape": "any" }] },
                "rules": rules,
            }))
            .unwrap();
            let line = if with_arg {
                format!("/cmd v{}", junk.replace(char::is_whitespace, ""))
            } else {
                "/cmd".to_string()
            };
            let lexed = lex_command("/", &line).unwrap();
            let v1 = judge_command_step(&step, &lexed, &never);
            let v2 = judge_command_step(&step, &lexed, &never);
            prop_assert_eq!(&v1, &v2);
            let v = v1.expect("valid declaration");
            // 上から最初に一致した規則（missing が無い・空、または引数が無いとき ["a"]）。
            let first = rules.iter().find(|r| match r["on"].get("missing") {
                None => true,
                Some(m) => m.as_array().unwrap().is_empty() || !with_arg,
            });
            let expect_clarify = first.is_some_and(|r| r["outcome"]["kind"] == "clarify");
            let is_clarified = matches!(
                v,
                CommandStepVerdict::Rule(CommandRuleVerdict::Clarified { .. })
            );
            prop_assert_eq!(is_clarified, expect_clarify);
            if is_clarified {
                prop_assert_eq!(v.reject_category(), None);
                prop_assert_eq!(v.kind(), Some(JudgmentKind::Clarified));
            }
            Ok(())
        },
    );
}
