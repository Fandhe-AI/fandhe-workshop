//! 開始条件を満たさない `Control::Begin` が文面 ID を記録して断る試験（TASK-240 1/2、MOD-7 (a)）。
//!
//! 3 Step の教材（`min-3step.json`）に `requires`（`st-1` の達成）つきの開始点を足し、新しい状態に開始を当てる
//! （core の試験は pack に依存しない。周回の教材での固定は `crates/pack/tests/start_unmet.rs`）。
//! 呼び出し元は CI の `make test` のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{apply, SessionState};
use serde_json::json;

/// `requires` を満たさない開始点 `s-need` を足した IR。`unmet` に文面の ID を渡す。
fn ir_with_unmet_start(unmet: &str) -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut ir = load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir");
    ir.starts = vec![Start {
        id: "s-need".to_string(),
        node: "main/st-2".to_string(),
        effects: Vec::new(),
        guidance: None,
        requires: Some(json!({"step": {"step": "st-1", "state": "achieved"}})),
        unmet: Some(json!(unmet)),
    }];
    ir
}

fn begin() -> Op {
    Op::Control(Control::Begin("s-need".to_string()))
}

fn without_log(state: &SessionState) -> SessionState {
    let mut s = state.clone();
    s.log.clear();
    s
}

/// 解決できた文面 ID を `rejection` に記録して 1 件だけ断り、状態（`log` を除く）を変えない。
#[test]
fn begin_with_unmet_requires_records_rejection_id() {
    let ir = ir_with_unmet_start("rj-decline");
    let state = SessionState::default();
    let before = without_log(&state);
    let (state, events) = apply(&ir, state, begin());
    assert!(
        matches!(
            events.as_slice(),
            [Event::Refused {
                op: ControlKind::Begin,
                reason: RefuseReason::StartUnmet,
                rejection: Some(id),
                ..
            }] if id == "rj-decline"
        ),
        "{events:?}"
    );
    assert!(state.run.current.is_none());
    assert_eq!(without_log(&state), before);
}

/// 文面を解決できなくても開始は断る。このとき `rejection` は `None`（fail-closed）。
#[test]
fn begin_with_unresolvable_unmet_text_still_refuses_without_id() {
    let ir = ir_with_unmet_start("rj-gone");
    let state = SessionState::default();
    let before = without_log(&state);
    let (state, events) = apply(&ir, state, begin());
    assert!(
        matches!(
            events.as_slice(),
            [Event::Refused {
                op: ControlKind::Begin,
                reason: RefuseReason::StartUnmet,
                rejection: None,
                ..
            }]
        ),
        "{events:?}"
    );
    assert_eq!(without_log(&state), before);
}
