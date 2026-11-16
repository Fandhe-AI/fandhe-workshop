//! `Session::begun_at`（開始点に `Control::Begin` と同じ規則で入る製品の起動の入口。#2164）の試験。
//!
//! effects・案内の水準・飛ばした Step の省略が `Session::new` + `Begin` と一致すること、断る場合（無い開始 ID・開始条件の不成立・
//! 効果が読めない）に Session を返さないことを確かめる。IR は最小の 3 Step の IR に開始点を書き足して作る。
//! 変異注入 `mutations/BEGUN-AT-1`・`BEGUN-AT-2` が検出する。

use std::path::PathBuf;

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, BeginAtRefusal, Session};
use serde_json::{json, Value};

fn start(
    id: &str,
    node: &str,
    effects: Vec<Value>,
    guidance: Option<&str>,
    requires: Option<Value>,
) -> Start {
    Start {
        id: id.to_string(),
        node: node.to_string(),
        effects,
        guidance: guidance.map(|g| json!(g)),
        requires,
        unmet: None,
    }
}

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut ir = load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir");
    ir.starts = vec![
        start(
            "s-fx",
            "main/st-2",
            vec![json!({"set_flag_global": "from-start"})],
            Some("C"),
            None,
        ),
        start(
            "s-need",
            "main/st-2",
            vec![],
            None,
            Some(json!({"step": {"step": "st-1", "state": "achieved"}})),
        ),
        start(
            "s-bad-fx",
            "main/st-2",
            vec![json!({"nope": 1})],
            None,
            None,
        ),
        start("s-bad-guidance", "main/st-2", vec![], Some("Z"), None),
    ];
    ir
}

#[test]
fn effects_案内_省略が_begin_と一致する() {
    let (session, events) = Session::begun_at(ir(), "s-fx").expect("begins");
    let mut reference = Session::new(ir());
    reference.apply(Op::Control(Control::Begin("s-fx".to_owned())));
    assert_eq!(session.state(), reference.state());

    let run = &session.state().run;
    assert!(
        run.global_flags.contains("from-start"),
        "effects が反映される"
    );
    assert_eq!(run.guidance_level, GuidanceLevel::C);
    assert_eq!(run.current, Some(StepUid("main/st-2".to_owned())));
    assert_eq!(
        session.step_status(&StepUid("main/st-1".to_owned())),
        StepStatus::Skipped
    );
    assert!(events.iter().any(|e| matches!(e, Event::Begun { .. })));
    assert!(events.iter().any(|e| matches!(e, Event::Skipped { .. })));
}

#[test]
fn 無い開始_id_は断る() {
    let got = Session::begun_at(ir(), "s-none").err();
    assert_eq!(
        got,
        Some(BeginAtRefusal::Refused(RefuseReason::UnknownStart))
    );
}

#[test]
fn 開始条件が成り立たない開始点は断る() {
    let got = Session::begun_at(ir(), "s-need").err();
    assert_eq!(got, Some(BeginAtRefusal::Refused(RefuseReason::StartUnmet)));
}

#[test]
fn 読めない効果と読めない案内の水準は断る() {
    for id in ["s-bad-fx", "s-bad-guidance"] {
        assert_eq!(
            Session::begun_at(ir(), id).err(),
            Some(BeginAtRefusal::Blocked),
            "{id}"
        );
    }
}

#[test]
fn 遷移の上限で止まる開始は断らず止まった状態で返す() {
    // 巡回する IR は読み込みの時点では断らない（SHELL-31）。開始が上限で止まる（SAFE-23）と現在の Step は空になる。
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-self.json",
    );
    let ir = load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir");
    let (session, events) = Session::begun_at(ir, "s-enter-self").expect("halted, not refused");
    assert!(session.state().run.current.is_none());
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. })));
}
