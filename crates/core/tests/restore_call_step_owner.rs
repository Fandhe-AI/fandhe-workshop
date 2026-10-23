//! 呼び出しの Step で保存・復帰したとき、持ち主が呼ばれた実体の根になることの試験（MOD-9、F-4、変異 X6 の否定。TASK-241 2/2）。
//!
//! 手で組んだ小さな IR（呼び出しの Step `main/c1` とその先の実体 `m@r1`）を使う。呼び出しの Step の状態は呼ばれた実体の
//! 切り身に入る（MOD-3）ので、保存の持ち主を scope の根（`main`）にすると、復帰で呼び出しの Step の状態が戻らない。
//! 教材 1 本分の同じ確認は pack の `round_slice_init`（`mod9_restore_returns_call_step_status_with_owner`）が持つ。

mod common;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::instance::status_of;
use fandhe_workshop_core::session::{SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
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

fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    v["nodes"] = json!([
        node(&base, "main", "x", None),
        node(&base, "main", "c1", Some("m@r1/s1")),
        node(&base, "m@r1", "s1", None),
    ]);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/c1"},
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

fn state_at_call_step() -> SessionState {
    let mut st = SessionState::default();
    st.run.current = Some(uid("main/c1"));
    let mut callee = Slice::default();
    callee.status.insert(uid("main/c1"), StepStatus::Entered);
    st.run.slices.insert(root("m@r1"), callee);
    let mut main = Slice::default();
    main.flags.insert("keep".to_string());
    st.run.slices.insert(root("main"), main);
    st
}

#[test]
fn save_at_call_step_records_callee_root() {
    let ir = ir();
    let (st, _) = apply(&ir, state_at_call_step(), Op::Control(Control::Save));
    let cp = st.checkpoints.get(&CheckpointId(1)).expect("cp");
    assert_eq!(cp.owner, Some(root("m@r1")));
}

#[test]
fn restore_returns_call_step_status_with_owner() {
    let ir = ir();
    let (mut st, _) = apply(&ir, state_at_call_step(), Op::Control(Control::Save));
    st.run
        .slices
        .get_mut(&root("m@r1"))
        .expect("callee")
        .status
        .insert(uid("main/c1"), StepStatus::Achieved);
    st.run
        .slices
        .get_mut(&root("main"))
        .expect("main")
        .flags
        .insert("later".to_string());
    let (next, _) = apply(&ir, st, Op::Control(Control::Restore(CheckpointId(1))));
    assert_eq!(
        status_of(&ir, &next.run, &uid("main/c1")),
        StepStatus::Entered
    );
    let main = next.run.slices.get(&root("main")).expect("main");
    assert!(main.flags.contains("later"));
}
