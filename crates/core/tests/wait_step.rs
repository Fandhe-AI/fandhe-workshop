//! `wait` の Step の試験（EP-1・EVT-20。#2205、#985 の前提）。
//!
//! `min-3step` の st-2 を `input: { kind: wait }` と 2 段の `processing`・`confirm: none` にした IR で確かめる。
//! - 入ると受講者の入力なしに処理中になり（`Entered` のまま）、進行を 2 つ適用すると `Accepted` が出て受付になり、
//!   `confirm: none` なので続けて達成になって st-3 へ移る
//! - 処理中に停止すると、世代が +1 され、Step は `entered`、処理中は最初の進行から始め直し、進行の効果は入った時点の値に戻る。
//!   続けて進行を 2 つ適用すると受付・達成になる（EVT-20 の今の動き。#981 の決定）
//! - `wait` でない Step は、入った時点では処理中にならない（従来どおり）
//!
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;

use std::path::PathBuf;

use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::Busy;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::json;

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-2 を 2 段の `wait`・`confirm: none` にした IR。
fn wait_ir() -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-2" {
            n.step.input = Some(json!({ "kind": "wait" }));
            n.step.rules = vec![];
            n.step.confirm = Some(json!({ "kind": "none", "reason": "待つだけの場面" }));
            n.step.processing = vec![
                json!({ "name": "w1-run", "effects": [{ "put": { "path": "checks/w", "attrs": { "state": "running" } } }] }),
                json!({ "name": "w2-done", "effects": [{ "set_attr": { "path": "checks/w", "attr": "state", "value": "done" } }] }),
            ];
        }
    }
    ir
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// st-1 を受付・確認で達成にして、st-2 に入ったセッション。
fn at_wait(ir: Ir) -> Session {
    let mut s = Session::new(ir);
    s.apply(Op::Control(Control::Begin("s-1".into())));
    s.apply(Op::Input(Input::Command("/make-note first-note".into())));
    while let Some(addr) = next_tick(s.state()) {
        s.apply(Op::Tick(addr));
    }
    s.apply(Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/draft".into(),
    }));
    assert_eq!(
        s.state().run.current,
        Some(uid("main/st-2")),
        "st-2 に入った"
    );
    s
}

fn tick(s: &mut Session) -> Vec<Event> {
    let addr = next_tick(s.state()).expect("処理中");
    s.apply(Op::Tick(addr)).to_vec()
}

#[test]
fn ep1_wait_step_starts_processing_on_entry_and_accepts_after_the_last_tick() {
    let mut s = at_wait(wait_ir());
    assert_eq!(
        s.state().run.busy,
        Some(Busy { done: 0, total: 2 }),
        "入った時点で処理中"
    );
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Entered);
    let first = tick(&mut s);
    assert!(
        !first.iter().any(|e| matches!(e, Event::Accepted { .. })),
        "途中の進行では受付にならない"
    );
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Entered);
    let last = tick(&mut s);
    let accepted = last
        .iter()
        .position(|e| matches!(e, Event::Accepted { .. }));
    let achieved = last
        .iter()
        .position(|e| matches!(e, Event::Achieved { .. }));
    assert!(
        accepted.is_some() && achieved.is_some() && accepted < achieved,
        "{last:?}"
    );
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Achieved);
    assert_eq!(s.state().run.busy, None);
    assert_eq!(
        s.state().run.current,
        Some(uid("main/st-3")),
        "confirm: none なので次へ移る"
    );
    assert_eq!(s.state().run.world["checks/w"].attrs["state"], "done");
}

#[test]
fn evt20_stop_during_wait_restarts_processing_from_the_entry() {
    let mut s = at_wait(wait_ir());
    let generation = s.state().run.generation;
    tick(&mut s);
    assert!(
        s.state().run.world.contains_key("checks/w"),
        "1 段目の効果が出ている"
    );
    let ev = s.apply(Op::Control(Control::Stop)).to_vec();
    assert!(
        ev.iter().any(|e| matches!(e, Event::Stopped { .. })),
        "{ev:?}"
    );
    let run = &s.state().run;
    assert_eq!(run.generation.0, generation.0 + 1, "世代 +1");
    assert_eq!(run.current, Some(uid("main/st-2")));
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Entered);
    assert_eq!(
        run.busy,
        Some(Busy { done: 0, total: 2 }),
        "最初の進行から始め直す"
    );
    assert!(
        !run.world.contains_key("checks/w"),
        "進行の効果は入った時点の値に戻る"
    );
    tick(&mut s);
    tick(&mut s);
    assert_eq!(
        s.step_status(&uid("main/st-2")),
        StepStatus::Achieved,
        "最初から 2 回の進行で受付・達成"
    );
    assert_eq!(s.state().run.current, Some(uid("main/st-3")));
}

#[test]
fn ep1_other_steps_are_not_busy_on_entry() {
    let s = at_wait(base_ir());
    assert_eq!(
        s.state().run.busy,
        None,
        "choice の st-2 は入った時点で処理中でない"
    );
}

#[test]
fn evt20_a_second_stop_on_the_same_wait_step_rewinds_again() {
    let mut s = at_wait(wait_ir());
    tick(&mut s);
    s.apply(Op::Control(Control::Stop));
    let generation = s.state().run.generation;
    tick(&mut s);
    assert!(s.state().run.world.contains_key("checks/w"));
    let ev = s.apply(Op::Control(Control::Stop)).to_vec();
    assert!(
        ev.iter().any(|e| matches!(e, Event::Stopped { .. })),
        "2 回目の停止も効く: {ev:?}"
    );
    assert_eq!(s.state().run.generation.0, generation.0 + 1);
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 2 }));
    assert!(!s.state().run.world.contains_key("checks/w"));
}

#[test]
fn evt20_stop_works_after_restoring_a_save_made_during_the_wait() {
    // 処理中に保存し、最後の進行で受付・達成になった後にその保存へ復帰する。復帰で消えた受付は停止を妨げない。
    let mut ir = wait_ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-2" {
            n.step.confirm = Some(json!({ "kind": "approve" }));
        }
    }
    let mut s = at_wait(ir);
    tick(&mut s);
    let saved = s.apply(Op::Control(Control::Save)).to_vec();
    let id = saved
        .iter()
        .find_map(|e| match e {
            Event::CheckpointSaved { id, .. } => Some(id.clone()),
            _ => None,
        })
        .expect("保存した");
    tick(&mut s);
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Accepted);
    s.apply(Op::Control(Control::Restore(id)));
    assert_eq!(s.state().run.busy, Some(Busy { done: 1, total: 2 }));
    assert_eq!(s.step_status(&uid("main/st-2")), StepStatus::Entered);
    let ev = s.apply(Op::Control(Control::Stop)).to_vec();
    assert!(
        ev.iter().any(|e| matches!(e, Event::Stopped { .. })),
        "{ev:?}"
    );
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 2 }));
}

#[test]
fn evt20_stop_keeps_the_effects_of_the_operation_that_entered_the_wait_step() {
    // 開始点の効果は、wait の Step に入った操作（Begin）の効果で、入口より前のもの。停止で戻さない。
    let mut ir = wait_ir();
    ir.starts.push(fandhe_workshop_contract::ir::Start {
        id: "s-w".into(),
        node: "main/st-2".into(),
        effects: vec![json!({ "put": { "path": "start/x", "attrs": { "from": "start" } } })],
        guidance: None,
        requires: None,
        unmet: None,
    });
    let mut s = Session::new(ir);
    s.apply(Op::Control(Control::Begin("s-w".into())));
    assert_eq!(
        s.state().run.busy,
        Some(Busy { done: 0, total: 2 }),
        "開始点から wait の Step に入る"
    );
    assert!(s.state().run.world.contains_key("start/x"));
    tick(&mut s);
    let ev = s.apply(Op::Control(Control::Stop)).to_vec();
    assert!(
        ev.iter().any(|e| matches!(e, Event::Stopped { .. })),
        "{ev:?}"
    );
    assert!(
        s.state().run.world.contains_key("start/x"),
        "入った操作の効果は残る"
    );
    assert!(
        !s.state().run.world.contains_key("checks/w"),
        "入った後の進行の効果は戻る"
    );
}
