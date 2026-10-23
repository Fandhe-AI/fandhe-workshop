//! 受け付けたうえで止まる場面（TASK-273、JUDGE-7）の結合試験。教材テストに当たる。
//!
//! `module-reuse-rounds` をコンパイルした IR の `st-7-1`（必須の `external_checks` が無い入力は `missing_arg` の拒否）と
//! `st-7-3`（同じ入力が受付になり、世界状態が止まる）を、core の判定（`judge_command_step`・`accepted_command`・
//! `accept_command`）に通す。台本の実行器 `fandhe-pack test`（TASK-404）は未実装のため、本試験がその代わりの
//! 証拠である。初期の世界状態は、IR の途中開始 `<実体>#s-7-1` の効果を宣言どおりに適用して作る。

mod common;

use std::collections::BTreeMap;

use common::testdata;
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_contract::judgment::{JudgmentKind, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::command_accept::{accept_command, accepted_command};
use fandhe_workshop_core::command_judge::CommandVerdict;
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::command_rule::{
    judge_command_step, CommandRuleVerdict, CommandStepVerdict,
};
use fandhe_workshop_core::effect::{apply_effects, NoConditions};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use serde_json::Value;

const PACK: &str = "module-reuse-rounds/test-data/pack";
const LINE: &str = "/implement-issue-tree 42";

fn ir() -> Ir {
    let bytes = compile_dir(&testdata(PACK)).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn node<'a>(ir: &'a Ir, uid: &str) -> &'a Node {
    ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .unwrap_or_else(|| panic!("node {uid} not found"))
}

fn never(_: &Value) -> Option<bool> {
    None
}

fn judge(node: &Node, line: &str) -> CommandStepVerdict {
    let lexed = lex_command("/", line).expect("lexable");
    judge_command_step(&node.step, &lexed, &never).expect("valid declaration")
}

/// 途中開始 `<scope>#s-7-1` の効果を適用した、`uid` に入った状態。
fn start_run(ir: &Ir, scope: &str, uid: &str) -> RunState {
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == format!("{scope}#s-7-1"))
        .expect("start s-7-1");
    let items: Vec<EffectItem> = start
        .effects
        .iter()
        .map(|v| EffectItem::from_value(v).expect("effect"))
        .collect();
    let step_uid = StepUid(uid.to_string());
    let mut run = RunState {
        current: Some(step_uid.clone()),
        ..RunState::default()
    };
    let at = Stamp {
        generation: Generation::INITIAL,
        step: step_uid,
    };
    let ev = apply_effects(
        &mut run,
        &Root(scope.to_string()),
        &items,
        &at,
        Cause::Input,
        &NoConditions,
    );
    assert!(!ev.iter().any(|e| matches!(e, Event::Warning { .. })));
    run
}

#[test]
fn st_7_1_is_missing_arg_and_st_7_3_stops_the_world() {
    let ir = ir();
    let scope = "issue-to-merge@round1";

    let n1 = node(&ir, "issue-to-merge@round1/st-7-1");
    let v1 = judge(n1, LINE);
    assert_eq!(v1, CommandStepVerdict::Declared(CommandVerdict::MissingArg));
    assert_eq!(v1.reject_category(), Some(RejectCategory::MissingArg));

    let uid3 = "issue-to-merge@round1/st-7-3";
    let n3 = node(&ir, uid3);
    let v3 = judge(n3, LINE);
    assert_eq!(v3.kind(), Some(JudgmentKind::Accepted));
    let CommandStepVerdict::Rule(verdict) = v3 else {
        panic!("expected rule verdict");
    };
    assert!(
        matches!(&verdict, CommandRuleVerdict::Accepted { rule_id, .. } if rule_id == "r-blocked")
    );
    let acc = accepted_command(&n3.step, &verdict).expect("effects");

    let mut run = start_run(&ir, scope, uid3);
    // `${root}` の展開が効いていれば、開始の効果が `issues/root-42` を置いている。
    assert_eq!(run.world["issues/root-42"].attrs["state"], "open");
    let step_uid = StepUid(uid3.to_string());
    let at = Stamp {
        generation: Generation::INITIAL,
        step: step_uid.clone(),
    };
    let ev = accept_command(&ir, &mut run, &step_uid, &acc, &at, &NoConditions);
    assert!(!ev.iter().any(|e| matches!(e, Event::Warning { .. })));

    assert_eq!(run.world["issues/root-42"].attrs["state"], "blocked");
    let slices: &BTreeMap<Root, _> = &run.slices;
    let r1 = &slices[&Root(scope.to_string())];
    assert!(r1.flags.contains("stopped"));
    assert_eq!(r1.status[&step_uid], StepStatus::Accepted);
    // 実体ごとに分かれる。round2 の切り身にも全体のフラグにも立たない。
    assert!(slices
        .get(&Root("issue-to-merge@round2".into()))
        .is_none_or(|s| !s.flags.contains("stopped")));
    assert!(!run.global_flags.contains("stopped"));
}

#[test]
fn st_7_3_with_arg_is_rejected_and_round2_is_independent() {
    let ir = ir();
    let n3 = node(&ir, "issue-to-merge@round1/st-7-3");
    let v = judge(n3, "/implement-issue-tree 42 --external-checks []");
    assert!(matches!(
        v,
        CommandStepVerdict::Rule(CommandRuleVerdict::RejectedByRule { ref rejection, .. })
            if rejection == "rj-7-3-with-arg"
    ));
    // round2 の実体も同じ宣言で、同じ入力が受付になる。
    let n3b = node(&ir, "issue-to-merge@round2/st-7-3");
    assert_eq!(judge(n3b, LINE).kind(), Some(JudgmentKind::Accepted));
}
