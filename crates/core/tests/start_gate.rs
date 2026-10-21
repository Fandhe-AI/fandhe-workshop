//! 開始条件の関門 `start_gate::check_start` の試験（TASK-51、PACK-18）。
//!
//! 条件なしは開始してよく、満たさない・評価できない開始条件は文面つきで断り、状態を変えないことを確かめる。
//! IR は最小の 3 Step の IR に開始点・文面・実体を書き足して作る（core の試験は pack に依存しない）。
//! 変異注入 `mutations/PACK18-2` が検出する。呼び出し元は CI の `make test` のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Instance, Ir, Start};
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::RefuseReason;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::rejection_text::RejectionTextError;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::start_gate::{check_start, StartRefusal};
use fandhe_workshop_core::step_status::StepStatus;
use serde_json::{json, Value};

fn start(id: &str, node: &str, requires: Option<Value>, unmet: Option<&str>) -> Start {
    Start {
        id: id.to_string(),
        node: node.to_string(),
        effects: Vec::new(),
        guidance: None,
        requires,
        unmet: unmet.map(|u| json!(u)),
    }
}

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut ir = load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir");
    let done = json!({"step": {"step": "st-1", "state": "achieved"}});
    ir.starts = vec![
        start("s-free", "main/st-2", None, None),
        start(
            "s-need",
            "main/st-2",
            Some(done.clone()),
            Some("rj-decline"),
        ),
        start(
            "s-bad",
            "main/st-2",
            Some(json!({"nope": 1})),
            Some("rj-decline"),
        ),
        start("s-notext", "main/st-2", Some(done), Some("rj-gone")),
        start(
            "s-flag",
            "main/st-2",
            Some(json!({"flag": "ready"})),
            Some("rj-decline"),
        ),
    ];
    ir
}

fn achieved(root: &str, step: &str) -> RunState {
    let mut run = RunState::default();
    let mut slice = Slice::default();
    slice
        .status
        .insert(StepUid(format!("{root}/{step}")), StepStatus::Achieved);
    run.slices.insert(Root(root.to_string()), slice);
    run
}

#[test]
fn pack18_no_requires_opens() {
    let ir = base_ir();
    let got = check_start(&ir, &RunState::default(), "s-free").expect("opens");
    assert_eq!(got.id, "s-free");
}

#[test]
fn pack18_satisfied_requires_opens() {
    let ir = base_ir();
    let got = check_start(&ir, &achieved("main", "st-1"), "s-need").expect("opens");
    assert_eq!(got.id, "s-need");
}

#[test]
fn pack18_unmet_refuses_with_text_and_state_unchanged() {
    let ir = base_ir();
    let state = SessionState::default();
    let before = serde_json::to_vec(&state).expect("ser");
    let err = check_start(&ir, &state.run, "s-need").expect_err("refuses");
    assert_eq!(err.reason(), RefuseReason::StartUnmet);
    let StartRefusal::Unmet { start, text } = err else {
        panic!("expected Unmet");
    };
    assert_eq!(start, "s-need");
    let text = text.expect("text resolves");
    assert_eq!(text.text, "rj-decline");
    assert!(!text.next_action.as_str().trim().is_empty());
    assert_eq!(before, serde_json::to_vec(&state).expect("ser"));
}

#[test]
fn pack18_unknown_start() {
    let ir = base_ir();
    let err = check_start(&ir, &RunState::default(), "nope").expect_err("refuses");
    assert_eq!(err, StartRefusal::UnknownStart);
    assert_eq!(err.reason(), RefuseReason::UnknownStart);
}

#[test]
fn pack18_unknown_node_is_unknown_start() {
    let mut ir = base_ir();
    ir.starts.push(start("s-ghost", "main/ghost", None, None));
    let err = check_start(&ir, &RunState::default(), "s-ghost").expect_err("refuses");
    assert_eq!(err, StartRefusal::UnknownStart);
}

#[test]
fn pack18_unevaluable_requires_refuses() {
    let ir = base_ir();
    let err = check_start(&ir, &achieved("main", "st-1"), "s-bad").expect_err("refuses");
    assert_eq!(err.reason(), RefuseReason::StartUnmet);
}

#[test]
fn pack18_missing_unmet_text_still_refuses() {
    let ir = base_ir();
    let err = check_start(&ir, &RunState::default(), "s-notext").expect_err("refuses");
    let StartRefusal::Unmet { text, .. } = err else {
        panic!("expected Unmet");
    };
    assert_eq!(text, Err(RejectionTextError::MissingText));
}

#[test]
fn pack18_module_start_uses_instance_root() {
    let mut ir = base_ir();
    let mut node = ir.nodes[1].clone();
    node.uid = "r2/st-2".to_string();
    node.scope = "r2".to_string();
    ir.nodes.push(node);
    ir.instances.push(Instance {
        id: "r2".to_string(),
        module: Some("mod".to_string()),
        args: Default::default(),
        caller: None,
    });
    ir.starts.push(start(
        "r2#s",
        "r2/st-2",
        Some(json!({"flag": "ready"})),
        Some("rj-decline"),
    ));
    let mut run = RunState::default();
    let mut main = Slice::default();
    main.flags.insert("ready".to_string());
    run.slices.insert(Root("main".to_string()), main);
    let err = check_start(&ir, &run, "r2#s").expect_err("main's flag does not count");
    assert_eq!(err.reason(), RefuseReason::StartUnmet);
    let mut r2 = Slice::default();
    r2.flags.insert("ready".to_string());
    run.slices.insert(Root("r2".to_string()), r2);
    assert!(check_start(&ir, &run, "r2#s").is_ok());
    assert!(check_start(&ir, &RunState::default(), "s-flag").is_err());
}
