//! 学習モジュールの実体ごとの状態の読み出し（MOD-1、TASK-233 1/2）の試験。
//!
//! `min-3step.json` の Step を 2 つの実体（`m@r1`・`m@r2`）へ複製した小さな IR で、同じ定義の Step が
//! `Session` の状態の上でも実体ごとに別の Step の実体として扱われ、片方の状態・フラグがもう片方へ漏れないことを確かめる。
//! 教材 1 本分の実体数・Step の実体数の照合は TASK-233 2/2、試作の教材での通しは pack の `module_instances_session` が持つ。

mod common;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, Stamp, StepUid};
use fandhe_workshop_core::effect::{apply_effects, NoConditions};
use fandhe_workshop_core::event::Cause;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{instance, load_ir, Session, SessionState};
use serde_json::{json, Value};

/// 2 つの実体 × Step 2 つに複製した IR。
fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    let mut nodes = Vec::new();
    for scope in ["m@r1", "m@r2"] {
        for sid in ["s1", "s2"] {
            let mut n = base.clone();
            n["uid"] = json!(format!("{scope}/{sid}"));
            n["scope"] = json!(scope);
            n["step"]["id"] = json!(sid);
            n["step"]["next"] = json!([]);
            nodes.push(n);
        }
    }
    v["nodes"] = Value::Array(nodes);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/u1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/u2"},
    ]);
    v["starts"] = json!([]);
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn session() -> Session {
    Session::entered_at(ir(), "m@r1/s1")
}

#[test]
fn instances_lists_every_root_in_ir_order() {
    let s = session();
    assert_eq!(
        s.instances(),
        vec![root("main"), root("m@r1"), root("m@r2")]
    );
}

#[test]
fn same_definition_is_a_different_step_instance_per_root() {
    let s = session();
    assert_eq!(
        s.steps_of(&root("m@r1")),
        vec![uid("m@r1/s1"), uid("m@r1/s2")]
    );
    assert_eq!(
        s.steps_of(&root("m@r2")),
        vec![uid("m@r2/s1"), uid("m@r2/s2")]
    );
    assert!(s.steps_of(&root("main")).is_empty());
}

#[test]
fn entering_one_instance_leaves_the_other_untouched() {
    let mut s = session();
    assert_eq!(s.step_status(&uid("m@r1/s1")), StepStatus::Entered);
    assert_eq!(s.step_status(&uid("m@r2/s1")), StepStatus::NotStarted);
    assert!(s.slice(&root("m@r2")).is_none());

    s.apply(Op::Control(Control::GoTo(uid("m@r2/s1"))));
    assert_eq!(s.step_status(&uid("m@r2/s1")), StepStatus::Entered);
    assert_eq!(s.step_status(&uid("m@r1/s1")), StepStatus::Entered);
    assert_eq!(s.step_status(&uid("m@r2/s2")), StepStatus::NotStarted);

    let a = s.slice(&root("m@r1")).expect("r1 slice");
    let b = s.slice(&root("m@r2")).expect("r2 slice");
    assert!(a.status.keys().all(|k| k.0.starts_with("m@r1/")));
    assert!(b.status.keys().all(|k| k.0.starts_with("m@r2/")));
}

#[test]
fn flags_set_in_one_instance_do_not_reach_the_other() {
    let mut s = session();
    s.apply(Op::Control(Control::GoTo(uid("m@r2/s1"))));
    let mut run = s.state().run.clone();
    let items = vec![EffectItem::from_value(&json!({"set_flag": "done"})).expect("effect")];
    let at = Stamp {
        generation: run.generation,
        step: uid("m@r2/s1"),
    };
    let _ = apply_effects(
        &mut run,
        &root("m@r2"),
        &items,
        &at,
        Cause::Input,
        &NoConditions,
    );
    let s = Session::from_state(
        s.ir().clone(),
        SessionState {
            run,
            ..s.state().clone()
        },
    );
    assert!(s
        .slice(&root("m@r2"))
        .is_some_and(|x| x.flags.contains("done")));
    assert!(s.slice(&root("m@r1")).is_some_and(|x| x.flags.is_empty()));
}

#[test]
fn reads_are_total_for_unknown_roots_and_malformed_uids() {
    let s = session();
    assert_eq!(s.step_status(&uid("")), StepStatus::NotStarted);
    assert_eq!(s.step_status(&uid("no-slash")), StepStatus::NotStarted);
    assert_eq!(s.step_status(&uid("x@y/z")), StepStatus::NotStarted);
    assert!(s.slice(&root("x@y")).is_none());
    assert!(s.steps_of(&root("x@y")).is_empty());
    assert_eq!(instance::root_of(&uid("m@r1/s1")), root("m@r1"));
}
