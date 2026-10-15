//! TASK-205 の契約適合試験の骨格（TASK-204 2/3）。
//!
//! 本ファイルの範囲: 最小 3 Step の IR から `Session` を作り、決定的な `Op` の列を適用して、
//! 決定性（EVT-22）・3 つの入口（自由関数・`Engine`・`Session`）の一致・イベント列とログの末尾の一致・
//! 世代が減らないこと・`Keep` の操作で世代が変わらないことを確かめる。
//! TASK-205（#169・#170）で足すもの: サンプルのイベント列、サンプルの版、EVT-6 の (3)(4)(6) の +1 の検査。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::input::{CheckpointId, Control, GenerationRule, Input, Op};
use fandhe_workshop_core::provider::StateChangeProvider;
use fandhe_workshop_core::session::{Engine, SessionState};
use fandhe_workshop_core::{apply, load_ir, Session};

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn ops() -> Vec<Op> {
    let uid = StepUid("main/st-1".to_string());
    vec![
        Op::Control(Control::Begin("start".into())),
        Op::Input(Input::Command("run".into())),
        Op::Input(Input::Cancel),
        Op::Tick(TickAddr {
            generation: Generation(1),
            step: uid.clone(),
            index: 0,
        }),
        Op::Control(Control::Stop),
        Op::Control(Control::GoTo(uid)),
        Op::Control(Control::Save),
        Op::Control(Control::Restore(CheckpointId(1))),
    ]
}

fn run_session(ir: &Ir) -> (SessionState, Vec<Vec<fandhe_workshop_core::event::Event>>) {
    let mut s = Session::new(ir.clone());
    let mut per_op = Vec::new();
    for op in ops() {
        let before = s.generation();
        let rule = op.generation_rule();
        let events = s.apply(op).to_vec();
        assert!(s.generation() >= before, "generation must not decrease");
        if rule == GenerationRule::Keep {
            assert_eq!(s.generation(), before);
        }
        per_op.push(events);
    }
    (s.into_state(), per_op)
}

#[test]
fn same_ops_give_same_result() {
    let ir = ir();
    assert_eq!(run_session(&ir), run_session(&ir));
}

#[test]
fn three_entry_points_agree_and_log_tail_matches() {
    let ir = ir();
    let engine = Engine::new(&ir);
    let mut free = SessionState::default();
    let mut via_provider = SessionState::default();
    for op in ops() {
        let len = free.log.len();
        let (next, events) = apply(&ir, free, op.clone());
        assert_eq!(&next.log[len..], events.as_slice());
        free = next;
        let (next, events_p) = engine.apply(via_provider, op);
        assert_eq!(events, events_p);
        via_provider = next;
    }
    let (session_state, _) = run_session(&ir);
    assert_eq!(free, via_provider);
    assert_eq!(free, session_state);
}
