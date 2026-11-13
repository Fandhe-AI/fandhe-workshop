//! 案内レベルが実行全体で 1 つであることの教材 1 本分の受け入れ試験（TASK-303 1/2、GUIDE-13・D-55。記録は `docs/testing.md` の 5.523）。
//!
//! `module-reuse-rounds` を `compile_dir` した IR の 2 周目の開始点（`s-r2`）に `guidance: B` を足した写しで、2 周目を開始した後に
//! 水準が B になり、表示に使う関数（`do`・本文）が B で選ばれ、1 周目の実体の Step・呼び出しの Step へ `GoTo` しても
//! `ClosedRound` で拒否されて B のままであることを確かめる。拒否の網羅は `round_closed_goto`、`use` に `guidance` を
//! 書けないことは `per_instance_guidance_gap`（`pack45_`）が受け持つ。変異は core の `GUIDE13-1`・`GUIDE13-2`。
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::guidance::GuidanceLevel::{A, B};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::body_level::select_body;
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::text_fill::step_do_at;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{begin_round2_plan, round1_plan, round_roots, run_plan, steps};
use serde_json::{json, Value};

/// 2 周目の開始点だけ `guidance: B` にし、全ての節点に水準 B の `do` と本文の版を足した IR。
fn ir_with_b_round2() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let starts = v["starts"].as_array_mut().expect("starts");
    let s2 = starts.iter_mut().find(|s| s["id"] == "s-r2").expect("s-r2");
    s2["guidance"] = json!("B");
    for n in v["nodes"].as_array_mut().expect("nodes") {
        n["step"]["do"] = json!("do-a");
        n["step"]["do_levels"] = json!({"B": "do-b"});
        n["body_doc"] = json!({"version":1,"blocks":[]});
        n["body_levels_doc"] = json!({"B": {"version":1,"blocks":[]}});
    }
    v["format_version"] = json!(2);
    v["pack"]["ir_format"] = json!(2);
    load_ir(&serde_json::to_vec(&v).expect("bytes")).expect("load_ir")
}

fn after_round2_begin(ir: &Ir) -> Session {
    let mut s = Session::from_state(ir.clone(), SessionState::default());
    run_plan(&mut s, &round1_plan(ir));
    assert_eq!(s.state().run.guidance_level, A);
    run_plan(&mut s, &begin_round2_plan());
    s
}

/// 現在の Step で `do` と本文が水準 B の版で選ばれること。
fn shows_b(ir: &Ir, s: &Session) {
    let cur = s.state().run.current.clone().expect("current");
    let node = ir.nodes.iter().find(|n| n.uid == cur.0).expect("node");
    let level = s.state().run.guidance_level;
    assert_eq!(step_do_at(&node.step, level), Some("do-b"));
    assert_eq!(select_body(node, level).expect("body").level, B);
}

#[test]
fn guide13_module_reuse_rounds_round2_shows_b() {
    let ir = ir_with_b_round2();
    let s = after_round2_begin(&ir);
    assert_eq!(s.state().run.guidance_level, B);
    shows_b(&ir, &s);
}

#[test]
fn guide13_module_reuse_rounds_goto_round1_refused_keeps_b() {
    let ir = ir_with_b_round2();
    let mut s = after_round2_begin(&ir);
    let (r1, _) = round_roots(&ir, "s-r1");
    let mut targets: Vec<StepUid> = steps(&ir, &r1);
    for i in &ir.instances {
        if r1.contains(&Root(i.id.clone())) {
            targets.push(StepUid(i.caller.clone().expect("caller")));
        }
    }
    assert!(targets.len() > 4);
    let before = s.state().clone();
    for to in targets {
        let ev = s.apply(Op::Control(Control::GoTo(to))).to_vec();
        assert!(
            matches!(
                ev.as_slice(),
                [Event::Refused {
                    op: ControlKind::GoTo,
                    reason: RefuseReason::ClosedRound,
                    ..
                }]
            ),
            "{ev:?}"
        );
        assert_eq!(s.state().run, before.run);
        assert_eq!(s.state().run.guidance_level, B);
        shows_b(&ir, &s);
    }
}
