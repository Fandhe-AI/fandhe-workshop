//! 未知のコマンドと順序違いの分類（TASK-268 1/2、JUDGE-4）の試験。
//!
//! 誤入力の組の出所は次のとおりで、受け入れ条件（2 場面で期待する分類が 100%、次の行動が空の拒否が 0 件）を
//! 1 つの試験で数える。
//!
//! - `testdata/engine-event-consistency/test-data/min-3step.json`（台本 `e-unknown`・`e-wrong-order`）
//! - `testdata/input-field-ime/test-data/ch1-pack.json`（台本 `pt-ch1` の `st-1-4` ほか）
//! - `tests/data/command_class_1/judgment-ir.json`（pack の `judgment-pack` の宣言を IR に写した試験データ。
//!   `e-unknown-1〜3`・`e-order-1`・`e-order-2` に対応する。core は pack に依存できないため、手元の写しを使う）

mod common;

use std::path::PathBuf;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::command_class::{
    classify_command, CommandClass, CommandClassError, CommandKind,
};
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::rejection_text::{resolve_rejection, RejectionSource};
use fandhe_workshop_core::step_status::StepStatus;
use proptest::prelude::*;
use serde_json::{json, Value};

fn load(rel: &str) -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel);
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn min3() -> Ir {
    load("../../testdata/engine-event-consistency/test-data/min-3step.json")
}
fn ch1() -> Ir {
    load("../../testdata/input-field-ime/test-data/ch1-pack.json")
}
fn judgment() -> Ir {
    load("tests/data/command_class_1/judgment-ir.json")
}

fn step<'a>(ir: &'a Ir, uid: &str) -> &'a Step {
    &ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .unwrap_or_else(|| panic!("no node {uid}"))
        .step
}

fn classify(
    ir: &Ir,
    step: &Step,
    status: StepStatus,
    raw: &str,
) -> Result<CommandClass, CommandClassError> {
    let lexed = lex_command("/", raw).unwrap_or_else(|e| panic!("{raw:?}: {e}"));
    classify_command(ir, step, status, &lexed)
}

fn entered(ir: &Ir, uid: &str, raw: &str) -> CommandClass {
    classify(ir, step(ir, uid), StepStatus::Entered, raw).expect("classify")
}

/// 合成した Step（`input` がコマンドの宣言）。
fn synth(name: &str) -> Step {
    serde_json::from_value(json!({
        "id": "st-x",
        "title": "x",
        "input": { "kind": "command", "name": name, "args": [] }
    }))
    .expect("synth step")
}

#[test]
fn ir_files_load() {
    // 試験データが load_ir を通る（上限の検査を含む）ことを最初に確かめる。
    assert_eq!(judgment().nodes.len(), 3);
    assert!(!min3().nodes.is_empty());
    assert!(!ch1().nodes.is_empty());
}

#[test]
fn wrong_input_pairs_are_classified_100_percent() {
    struct Case {
        ir: Ir,
        uid: &'static str,
        raw: &'static str,
        expect: RejectCategory,
    }
    let c = |ir: &Ir, uid, raw, expect| Case {
        ir: ir.clone(),
        uid,
        raw,
        expect,
    };
    let (m, h, j) = (min3(), ch1(), judgment());
    use RejectCategory::{UnknownCommand as U, WrongOrder as W};
    let cases = vec![
        c(&m, "main/st-1", "/nothing", U),
        c(&m, "main/st-1", "/publish-note", W),
        c(&h, "main/st-1-4", "/advance-phase", W),
        c(&h, "main/st-1-4", "/new-idea/advance-phase", U),
        c(&h, "main/st-1-1", "/advance-phase", W),
        c(&h, "main/st-1-3", "/advance-phase", W),
        c(&h, "main/st-1-6", "/advance-phase", W),
        c(&j, "main/st-clarify", "/nothing", U),
        c(&j, "main/st-two-args", "/mk-idea", U),
        c(&j, "main/st-one-arg", "/start", U),
        c(&j, "main/st-clarify", "/publish-run", W),
        c(&j, "main/st-two-args", "/publish-run", W),
    ];
    let total = cases.len();
    let mut matched = 0;
    let mut empty_next = 0;
    for case in &cases {
        let s = step(&case.ir, case.uid);
        let class = classify(&case.ir, s, StepStatus::Entered, case.raw).expect("classify");
        if class.category() == Some(case.expect) {
            matched += 1;
        }
        let Some(cat) = class.category() else {
            continue;
        };
        let r = resolve_rejection(&case.ir, s, RejectionSource::Category(cat)).expect("text");
        if r.next_action.as_str().trim().is_empty() {
            empty_next += 1;
        }
    }
    assert_eq!(matched, total, "classification rate must be 100%");
    assert_eq!(empty_next, 0, "no rejection may have an empty next action");
}

#[test]
fn rejection_text_ids_follow_defaults() {
    let ir = min3();
    let s = step(&ir, "main/st-1");
    let u = entered(&ir, "main/st-1", "/nothing");
    let r = resolve_rejection(
        &ir,
        s,
        RejectionSource::Category(u.category().expect("cat")),
    )
    .expect("text");
    assert_eq!(r.text, "rj-default-unknown-command");
    let w = entered(&ir, "main/st-1", "/publish-note");
    let r = resolve_rejection(
        &ir,
        s,
        RejectionSource::Category(w.category().expect("cat")),
    )
    .expect("text");
    assert_eq!(r.text, "rj-default-wrong-order");
}

#[test]
fn own_command_is_accepting_when_entered() {
    let m = min3();
    assert_eq!(
        entered(&m, "main/st-1", "/make-note first-note"),
        CommandClass::Accepting
    );
    let h = ch1();
    assert_eq!(
        entered(&h, "main/st-1-4", "/new-idea"),
        CommandClass::Accepting
    );
    assert_eq!(CommandClass::Accepting.category(), None);
}

#[test]
fn flow_command_depends_on_status() {
    let ir = min3();
    let s = synth("publish-note");
    let run = |st| classify(&ir, &s, st, "/publish-note").expect("classify");
    assert_eq!(run(StepStatus::Entered), CommandClass::Accepting);
    assert_eq!(run(StepStatus::Accepted), CommandClass::WrongOrder);
    assert_eq!(run(StepStatus::NotStarted), CommandClass::WrongOrder);
}

#[test]
fn accepted_status_ignores_broken_declaration() {
    let ir = min3();
    let broken: Step = serde_json::from_value(json!({
        "id": "st-x", "title": "x",
        "input": { "kind": "command", "name": "", "args": [] }
    }))
    .expect("step");
    assert_eq!(
        classify(&ir, &broken, StepStatus::Accepted, "/publish-note"),
        Ok(CommandClass::WrongOrder)
    );
    assert_eq!(
        classify(&ir, &broken, StepStatus::Entered, "/publish-note"),
        Err(CommandClassError::InvalidStep)
    );
}

#[test]
fn input_less_and_other_kind_steps_are_not_errors() {
    let ir = min3();
    let none: Step = serde_json::from_value(json!({ "id": "st-x", "title": "x" })).expect("step");
    assert_eq!(
        classify(&ir, &none, StepStatus::Entered, "/publish-note"),
        Ok(CommandClass::WrongOrder)
    );
    // 別の方式の Step（承認）
    assert_eq!(
        entered(&ir, "main/st-3", "/publish-note"),
        CommandClass::WrongOrder
    );
}

#[test]
fn name_is_case_sensitive() {
    let ir = min3();
    assert_eq!(
        entered(&ir, "main/st-1", "/Publish-note"),
        CommandClass::Unknown
    );
}

#[test]
fn builtin_and_skill_not_accepted_are_unclassified() {
    let ir = min3();
    assert_eq!(
        entered(&ir, "main/st-2", "/make-note x"),
        CommandClass::Unclassified(CommandKind::Skill)
    );
    assert_eq!(
        entered(&ir, "main/st-2", "/help"),
        CommandClass::Unclassified(CommandKind::Builtin)
    );
    assert_eq!(
        CommandClass::Unclassified(CommandKind::Skill).category(),
        None
    );
}

#[test]
fn unsupported_is_never_wrong_order_or_accepting() {
    let ir = min3();
    let u = CommandClass::Unclassified(CommandKind::Unsupported);
    assert_eq!(entered(&ir, "main/st-1", "/sync-cloud"), u);
    assert_eq!(entered(&ir, "main/st-3", "/sync-cloud"), u);
    let s = synth("sync-cloud");
    for st in [StepStatus::Entered, StepStatus::Accepted] {
        assert_eq!(classify(&ir, &s, st, "/sync-cloud"), Ok(u.clone()));
    }
}

#[test]
fn invalid_kind_values_fail_closed() {
    let bad = [
        json!({ "description": "d" }),
        json!(3),
        json!(null),
        json!("weird"),
        json!("Flow"),
        json!(""),
    ];
    for v in bad {
        let mut ir = min3();
        ir.commands.insert("odd".to_string(), v.clone());
        let s = step(&ir, "main/st-1").clone();
        assert_eq!(
            classify(&ir, &s, StepStatus::Entered, "/odd"),
            Err(CommandClassError::InvalidKind),
            "{v}"
        );
        // 他の行の不正は、無関係な入力に影響しない。
        assert_eq!(
            classify(&ir, &s, StepStatus::Entered, "/nothing"),
            Ok(CommandClass::Unknown)
        );
        assert_eq!(
            classify(&ir, &s, StepStatus::Entered, "/publish-note"),
            Ok(CommandClass::WrongOrder)
        );
    }
}

#[test]
fn error_display_is_fixed_text() {
    for e in [
        CommandClassError::InvalidKind,
        CommandClassError::InvalidStep,
    ] {
        let t = e.to_string();
        assert!(!t.is_empty());
        assert!(!t.contains("odd") && !t.contains("publish"));
    }
}

#[test]
fn kind_from_value_accepts_only_four_words() {
    assert_eq!(
        CommandKind::from_value(&json!("builtin")),
        Some(CommandKind::Builtin)
    );
    assert_eq!(
        CommandKind::from_value(&json!("skill")),
        Some(CommandKind::Skill)
    );
    assert_eq!(
        CommandKind::from_value(&json!("flow")),
        Some(CommandKind::Flow)
    );
    assert_eq!(
        CommandKind::from_value(&json!("unsupported")),
        Some(CommandKind::Unsupported)
    );
    assert_eq!(CommandKind::from_value(&json!(" flow")), None);
}

fn status_strategy() -> impl Strategy<Value = StepStatus> {
    prop_oneof![
        Just(StepStatus::NotStarted),
        Just(StepStatus::Entered),
        Just(StepStatus::Accepted),
        Just(StepStatus::Achieved),
        Just(StepStatus::Skipped),
    ]
}

#[test]
fn unknown_name_is_unknown_for_any_step_and_status() {
    let ir = min3();
    let uids: Vec<String> = ir.nodes.iter().map(|n| n.uid.clone()).collect();
    check(
        "class-unknown",
        DEFAULT_CASES,
        ("[a-z0-9]{1,12}", 0..uids.len(), status_strategy()),
        |(name, i, st)| {
            prop_assume!(!ir.commands.contains_key(&name));
            let s = step(&ir, &uids[i]);
            let r = classify(&ir, s, st, &format!("/{name}"));
            prop_assert_eq!(r, Ok(CommandClass::Unknown));
            Ok(())
        },
    );
}

#[test]
fn other_flow_name_is_wrong_order_for_any_status() {
    let ir = min3();
    let s = step(&ir, "main/st-1");
    check(
        "class-wrong-order",
        DEFAULT_CASES,
        status_strategy(),
        |st| {
            // st-1 のコマンドは make-note なので、flow の publish-note は常に順序違い。
            let r = classify(&ir, s, st, "/publish-note");
            prop_assert_eq!(r, Ok(CommandClass::WrongOrder));
            Ok(())
        },
    );
}

fn value_strategy() -> impl Strategy<Value = Value> {
    prop_oneof![
        Just(json!(null)),
        any::<bool>().prop_map(Value::from),
        any::<i64>().prop_map(Value::from),
        "[a-zA-Z ]{0,12}".prop_map(Value::from),
        Just(json!("flow")),
        Just(json!({ "description": "d" })),
        Just(json!(["flow"])),
    ]
}

#[test]
fn arbitrary_kind_values_are_total_and_deterministic() {
    let base = min3();
    check(
        "class-total",
        DEFAULT_CASES,
        (value_strategy(), status_strategy()),
        |(v, st)| {
            let mut ir = base.clone();
            ir.commands.insert("odd".to_string(), v);
            let s = step(&ir, "main/st-1");
            let a = classify(&ir, s, st, "/odd");
            let b = classify(&ir, s, st, "/odd");
            prop_assert_eq!(&a, &b);
            Ok(())
        },
    );
}
