//! 2 周の教材でのヒントの使用回数の実体ごとの分離（TASK-297 1/2。GUIDE-7。`guide7_*`）の試験。
//!
//! `module-reuse-rounds` を `compile_dir` した IR で、1 周目の Step でヒントを開いてから 2 周目を開始し、
//! 2 周目の根の使用回数が 0 から始まること、2 周目で開いても 1 周目の合計が変わらないことを確かめる。
//! 停止・復帰・移動で減らないことの網羅は core の `hint_count_monotone` と pack の `round_hint_counts`（TASK-297 2/2）が持つ。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{begin_round2_plan, round1_plan, round_roots, run_plan, step_in};

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn goto(rs: &mut RecordedSession, uid: &StepUid) {
    rs.apply(Op::Control(Control::GoTo(uid.clone())));
    assert_eq!(rs.session().state().run.current.as_ref(), Some(uid));
}

/// 現在の Step のヒントを、出せる数だけ開き、開いた段数を返す。
fn open_all(rs: &mut RecordedSession) -> u32 {
    let n = current_hints(rs.session().ir(), &rs.session().state().run).len();
    assert!(n > 0, "the step has hints at the default level");
    for _ in 0..n {
        rs.apply(Op::Control(Control::OpenHint));
    }
    u32::try_from(n).expect("n")
}

struct Setup {
    rs: RecordedSession,
    r1_step: StepUid,
    r1_root: Root,
    r1_before: u32,
    r1_opened: u32,
    r2_roots: Vec<Root>,
    r2_step: StepUid,
}

/// 1 周目の Step でヒントを開き、2 周目を開始した直後の状態。
fn setup() -> Setup {
    let ir = ir();
    let mut rs = RecordedSession::new(Session::new(ir.clone()), RecordedSession::first_start(&ir));
    run_plan(&mut rs, &round1_plan(&ir));
    let (r1_roots, _) = round_roots(&ir, "s-r1");
    let r1_step = step_in(&ir, &r1_roots, "st-2-1");
    goto(&mut rs, &r1_step);
    let r1_before = rs.hints_opened_in(&r1_step.root());
    let r1_opened = open_all(&mut rs);
    run_plan(&mut rs, &begin_round2_plan());
    let (r2_roots, _) = round_roots(&ir, "s-r2");
    let r2_step = step_in(&ir, &r2_roots, "st-2-1");
    Setup {
        r1_root: r1_step.root(),
        rs,
        r1_step,
        r1_before,
        r1_opened,
        r2_roots,
        r2_step,
    }
}

#[test]
fn guide7_round2_starts_with_zero_hint_uses() {
    let s = setup();
    assert!(s.r1_opened > 0);
    assert_eq!(s.rs.hints_opened_in(&s.r1_root), s.r1_before + s.r1_opened);
    for r in &s.r2_roots {
        assert_eq!(s.rs.hints_opened_in(r), 0);
    }
    assert_eq!(s.rs.hints_opened_at(&s.r2_step), 0);
    assert!(s
        .rs
        .session()
        .state()
        .hints_opened
        .keys()
        .all(|k| !s.r2_roots.contains(&k.root())));
}

#[test]
fn guide7_round2_hints_do_not_reach_round1() {
    let mut s = setup();
    let r1_total = s.rs.hints_opened_in(&s.r1_root);
    goto(&mut s.rs, &s.r2_step.clone());
    let opened = open_all(&mut s.rs);
    assert_eq!(s.rs.hints_opened_at(&s.r2_step), opened);
    assert_eq!(s.rs.hints_opened_in(&s.r2_step.root()), opened);
    assert_eq!(s.rs.hints_opened_in(&s.r1_root), r1_total);

    goto(&mut s.rs, &s.r1_step.clone());
    assert_eq!(s.rs.hints_opened_at(&s.r1_step), s.r1_before + s.r1_opened);
    assert_eq!(s.rs.hints_opened_in(&s.r2_step.root()), opened);
}
