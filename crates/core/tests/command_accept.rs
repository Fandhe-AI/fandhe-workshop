//! 受け付けたうえで止まる場面の判定（TASK-273、JUDGE-7）の試験。
//!
//! 宣言の例 `st-7-1`・`st-7-3` は、試作の教材（PoC-3「3 場面の区別」）の `module-reuse-rounds` の
//! `modules/issue-to-merge/module.yaml` の写し（`${root}` は `42` に置き換え済み。使うキーだけを持つ形）である。
//! 出所は `testdata/module-reuse-rounds/test-data/pack/` で、`docs/spec/` は private のため試験からは直接読まない。
//! コンパイルした IR を通す結合試験は `crates/pack/tests/stop_scene.rs` にある。

mod common;

use std::collections::BTreeMap;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{JudgmentKind, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::command_accept::{accept_command, accepted_command, CommandAcceptError};
use fandhe_workshop_core::command_judge::CommandVerdict;
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::command_rule::{
    judge_command_step, CommandRuleVerdict, CommandStepVerdict,
};
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use proptest::prelude::*;
use serde_json::{json, Value};

const ROOT: &str = "issue-to-merge@round1";

fn steps() -> BTreeMap<String, Step> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/command_accept_1/steps.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn step(name: &str) -> Step {
    steps().remove(name).unwrap_or_else(|| panic!("{name}"))
}

fn uid(step_id: &str) -> StepUid {
    StepUid(format!("{ROOT}/{step_id}"))
}

fn stamp(step_id: &str) -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: uid(step_id),
    }
}

fn never(_: &Value) -> Option<bool> {
    None
}

fn judge(step: &Step, line: &str) -> CommandStepVerdict {
    let lexed = lex_command("/", line).expect("lexable");
    judge_command_step(step, &lexed, &never).expect("valid declaration")
}

/// `issues/root-42` を `running` で置いた、`step_id` に入った状態。
fn start(step_id: &str) -> RunState {
    let mut run = RunState {
        current: Some(uid(step_id)),
        ..RunState::default()
    };
    run.world.insert(
        "issues/root-42".into(),
        Element {
            fixture: Some("fx-kakuu-issue-root".into()),
            attrs: BTreeMap::from([("state".to_string(), "running".to_string())]),
        },
    );
    run.slices
        .entry(Root(ROOT.into()))
        .or_default()
        .status
        .insert(uid(step_id), StepStatus::Entered);
    run
}

fn rule_verdict(v: CommandStepVerdict) -> CommandRuleVerdict {
    match v {
        CommandStepVerdict::Rule(r) => r,
        other => panic!("expected rule verdict, got {other:?}"),
    }
}

const LINE: &str = "/implement-issue-tree 42";

#[test]
fn st_7_1_missing_external_checks_is_missing_arg() {
    let s = step("st-7-1");
    let v = judge(&s, LINE);
    assert_eq!(v, CommandStepVerdict::Declared(CommandVerdict::MissingArg));
    assert_eq!(v.kind(), Some(JudgmentKind::Rejected));
    assert_eq!(v.reject_category(), Some(RejectCategory::MissingArg));
    // 規則は読まれず、受付の経路にも入らない。
    let r = CommandRuleVerdict::NoRule;
    assert_eq!(
        accepted_command(&s, &r),
        Err(CommandAcceptError::NotAccepted)
    );
}

#[test]
fn st_7_3_same_input_is_accepted_and_stops_world() {
    let s = step("st-7-3");
    let v = judge(&s, LINE);
    assert_eq!(v.kind(), Some(JudgmentKind::Accepted));
    let verdict = rule_verdict(v);
    assert!(
        matches!(&verdict, CommandRuleVerdict::Accepted { rule_id, .. } if rule_id == "r-blocked")
    );
    let acc = accepted_command(&s, &verdict).expect("effects");
    assert_eq!(acc.rule_id, "r-blocked");
    assert_eq!(acc.effects.len(), 2);

    let mut run = start("st-7-3");
    let before = run.clone();
    let ev = accept_command(
        &common::min_ir(),
        &mut run,
        &uid("st-7-3"),
        &acc,
        &stamp("st-7-3"),
        &NoConditions,
    );

    assert_eq!(run.world["issues/root-42"].attrs["state"], "blocked");
    let slice = &run.slices[&Root(ROOT.into())];
    assert!(slice.flags.contains("stopped"));
    assert!(!run.global_flags.contains("stopped"));
    assert_eq!(slice.status[&uid("st-7-3")], StepStatus::Accepted);
    assert_eq!(run.current, before.current);
    assert_eq!(run.generation, before.generation);
    assert_eq!(run.busy, before.busy);
    assert_eq!(run.completed, before.completed);

    assert!(matches!(ev.first(), Some(Event::Accepted { .. })));
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Changed { path, .. } if path == "issues/root-42")));
    assert!(ev
        .iter()
        .any(|e| matches!(e, Event::Flag { name, on: true, .. } if name == "stopped")));
    assert!(!ev.iter().any(|e| matches!(
        e,
        Event::Warning { .. }
            | Event::Achieved { .. }
            | Event::Transitioned { .. }
            | Event::ChapterCompleted { .. }
    )));
}

#[test]
fn st_7_1_and_st_7_3_differ() {
    let v1 = judge(&step("st-7-1"), LINE);
    let v3 = judge(&step("st-7-3"), LINE);
    assert_eq!(v1.kind(), Some(JudgmentKind::Rejected));
    assert_eq!(v3.kind(), Some(JudgmentKind::Accepted));
    assert_ne!(v1, v3);

    // 拒否の側は世界状態を変える経路を持たない。受付の側だけが止める。
    let s3 = step("st-7-3");
    let acc = accepted_command(&s3, &rule_verdict(v3)).unwrap();
    let mut stopped = start("st-7-3");
    accept_command(
        &common::min_ir(),
        &mut stopped,
        &uid("st-7-3"),
        &acc,
        &stamp("st-7-3"),
        &NoConditions,
    );
    let untouched = start("st-7-1");
    assert_eq!(untouched.world["issues/root-42"].attrs["state"], "running");
    assert_eq!(stopped.world["issues/root-42"].attrs["state"], "blocked");
}

#[test]
fn st_7_3_with_arg_is_rejected_by_rule() {
    let s = step("st-7-3");
    let v = judge(&s, "/implement-issue-tree 42 --external-checks []");
    let verdict = rule_verdict(v);
    assert!(matches!(
        &verdict,
        CommandRuleVerdict::RejectedByRule { rejection, .. } if rejection == "rj-7-3-with-arg"
    ));
    assert_eq!(verdict.kind(), Some(JudgmentKind::Rejected));
    assert_eq!(
        accepted_command(&s, &verdict),
        Err(CommandAcceptError::NotAccepted)
    );
}

#[test]
fn invalid_effect_fails_closed() {
    let cases = [
        (
            "bad_effects_unknown",
            CommandAcceptError::InvalidEffect { item: 1 },
        ),
        ("bad_effects_shape", CommandAcceptError::InvalidEffects),
    ];
    for (name, want) in cases {
        let s = step(name);
        let verdict = rule_verdict(judge(&s, "/x"));
        assert!(
            matches!(verdict, CommandRuleVerdict::Accepted { .. }),
            "{name}"
        );
        assert_eq!(accepted_command(&s, &verdict), Err(want), "{name}");
    }
}

#[test]
fn rule_id_mismatch_is_invalid_declaration() {
    let s = step("st-7-3");
    let wrong_id = CommandRuleVerdict::Accepted {
        rule_index: 0,
        rule_id: "r-other".into(),
    };
    let out_of_range = CommandRuleVerdict::Accepted {
        rule_index: 9,
        rule_id: "r-blocked".into(),
    };
    // 添字 1 は `reject` の規則で、`id` が合っていても受付にしない。
    let not_accept = CommandRuleVerdict::Accepted {
        rule_index: 1,
        rule_id: "r-with-arg".into(),
    };
    for v in [wrong_id, out_of_range, not_accept] {
        assert_eq!(
            accepted_command(&s, &v),
            Err(CommandAcceptError::InvalidDeclaration),
            "{v:?}"
        );
    }
}

#[test]
fn omitted_effects_is_an_empty_accept() {
    let s: Step = serde_json::from_value(json!({
        "id": "s", "title": "t",
        "input": { "kind": "command", "name": "x", "args": [] },
        "rules": [{ "id": "r", "on": { "command": "x" }, "outcome": { "kind": "accept" } }],
    }))
    .unwrap();
    let acc = accepted_command(&s, &rule_verdict(judge(&s, "/x"))).unwrap();
    assert!(acc.effects.is_empty());
}

#[test]
fn error_display_has_no_values() {
    for e in [
        CommandAcceptError::NotAccepted,
        CommandAcceptError::InvalidDeclaration,
        CommandAcceptError::InvalidEffects,
        CommandAcceptError::InvalidEffect { item: 1 },
    ] {
        let text = e.to_string();
        assert!(
            !text.contains("unknown_word")
                && !text.contains("stopped")
                && !text.contains("blocked"),
            "{text}"
        );
    }
}

/// 受付の規則の効果の部品。
fn effect_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!({ "set_flag": "stopped" })),
        Just(json!({ "set_flag": "other" })),
        Just(
            json!({ "set_attr": { "path": "issues/root-42", "attr": "state", "value": "blocked" } })
        ),
        Just(json!({ "set_attr": { "path": "issues/missing", "attr": "state", "value": "x" } })),
    ]
}

#[test]
fn accept_properties() {
    let strat = (
        proptest::collection::vec(effect_strategy(), 0..5),
        any::<bool>(),
        any::<bool>(),
    );
    check(
        "command-accept",
        DEFAULT_CASES,
        strat,
        |(effects, with_root, accept_rule)| {
            let outcome = if accept_rule {
                json!({ "kind": "accept", "effects": effects })
            } else {
                json!({ "kind": "reject", "rejection": "rj" })
            };
            let s: Step = serde_json::from_value(json!({
                "id": "st", "title": "t",
                "input": { "kind": "command", "name": "cmd", "args": [] },
                "rules": [{ "id": "r", "on": { "command": "cmd" }, "outcome": outcome }],
            }))
            .unwrap();
            let verdict = rule_verdict(judge(&s, "/cmd"));
            let mut run = start("st");
            if !with_root {
                run.world.clear();
            }
            let before = run.clone();
            match accepted_command(&s, &verdict) {
                Ok(acc) => {
                    prop_assert!(accept_rule);
                    let ev = accept_command(
                        &common::min_ir(),
                        &mut run,
                        &uid("st"),
                        &acc,
                        &stamp("st"),
                        &NoConditions,
                    );
                    prop_assert_eq!(
                        run.slices[&Root(ROOT.into())].status[&uid("st")],
                        StepStatus::Accepted
                    );
                    prop_assert_eq!(run.generation, before.generation);
                    prop_assert_eq!(&run.current, &before.current);
                    let achieved = ev.iter().any(|e| matches!(e, Event::Achieved { .. }));
                    prop_assert!(!achieved);
                }
                Err(e) => {
                    prop_assert!(!accept_rule);
                    prop_assert_eq!(e, CommandAcceptError::NotAccepted);
                    prop_assert_eq!(&run, &before);
                }
            }
            Ok(())
        },
    );
}
