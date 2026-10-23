//! 呼び出しの Step の状態の持ち主（MOD-3、TASK-235 1/2）の試験。
//!
//! 手で組んだ小さな IR（呼び出しの Step `main/c1`・`main/c2` と、その先の実体 `m@r1`・`m@r2`）で、
//! 呼び出しの Step の状態が呼ばれた実体の切り身に入り、フラグは scope の根のままであることを確かめる。
//! 教材 1 本分の周回の初期状態は pack の `round_slice_init` が持つ。

mod common;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::{owner_of, root_of, steps_of};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

fn node(base: &Value, scope: &str, sid: &str, enter: Option<&str>) -> Value {
    let mut n = base.clone();
    n["uid"] = json!(format!("{scope}/{sid}"));
    n["scope"] = json!(scope);
    n["step"]["id"] = json!(sid);
    n["step"]["next"] = json!([]);
    if let Some(to) = enter {
        n["enter"] = json!(to);
    }
    n
}

fn ir_with(instances: Value) -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    v["nodes"] = json!([
        node(&base, "main", "x", None),
        node(&base, "main", "c1", Some("m@r1/s1")),
        node(&base, "main", "c2", Some("m@r2/s1")),
        node(&base, "m@r1", "s1", None),
        node(&base, "m@r1", "s2", None),
        node(&base, "m@r2", "s1", None),
        node(&base, "m@r2", "s2", None),
    ]);
    v["instances"] = instances;
    v["starts"] = json!([]);
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn ir() -> Ir {
    ir_with(json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/c1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/c2"},
    ]))
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

#[test]
fn owner_of_call_step_is_the_called_instance() {
    let ir = ir();
    assert_eq!(owner_of(&ir, &uid("main/c1")), root("m@r1"));
    assert_eq!(owner_of(&ir, &uid("main/c2")), root("m@r2"));
    assert_eq!(owner_of(&ir, &uid("m@r1/s1")), root("m@r1"));
    assert_eq!(owner_of(&ir, &uid("main/x")), root("main"));
}

#[test]
fn owner_of_falls_back_to_scope_root_and_never_panics() {
    let ir = ir();
    assert_eq!(owner_of(&ir, &uid("main/unknown")), root("main"));
    assert_eq!(owner_of(&ir, &uid("no-slash")), root_of(&uid("no-slash")));
    assert_eq!(owner_of(&ir, &uid("")), root_of(&uid("")));
}

#[test]
fn owner_of_with_duplicate_callers_picks_the_first_in_ir_order() {
    let ir = ir_with(json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/c1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/c1"},
    ]));
    assert_eq!(owner_of(&ir, &uid("main/c1")), root("m@r1"));
}

#[test]
fn steps_of_includes_the_call_step_in_the_called_instance_only() {
    let ir = ir();
    let r1 = steps_of(&ir, &root("m@r1"));
    assert!(r1.contains(&uid("main/c1")));
    assert!(!r1.contains(&uid("main/c2")));
    let main = steps_of(&ir, &root("main"));
    assert_eq!(main, vec![uid("main/x")]);
}

#[test]
fn call_step_status_lands_in_the_called_instance_slice() {
    let mut s = Session::entered_at(ir(), "main/x");
    s.apply(Op::Control(Control::GoTo(uid("main/c1"))));
    let r1 = s.slice(&root("m@r1")).expect("m@r1 slice");
    assert_eq!(r1.status.get(&uid("main/c1")), Some(&StepStatus::Entered));
    assert_eq!(r1.status.get(&uid("m@r1/s1")), Some(&StepStatus::Entered));
    let main = s.slice(&root("main")).expect("main slice");
    assert!(!main.status.contains_key(&uid("main/c1")));
    assert_eq!(s.step_status(&uid("main/c1")), StepStatus::Entered);
    assert_eq!(s.step_status(&uid("main/c2")), StepStatus::NotStarted);
}

#[test]
fn every_status_key_is_owned_by_its_slice() {
    let mut s = Session::entered_at(ir(), "main/x");
    s.apply(Op::Control(Control::GoTo(uid("main/c1"))));
    s.apply(Op::Control(Control::GoTo(uid("main/c2"))));
    for r in s.instances() {
        if let Some(slice) = s.slice(&r) {
            for k in slice.status.keys() {
                assert_eq!(owner_of(s.ir(), k), r, "{}", k.0);
            }
        }
    }
}
