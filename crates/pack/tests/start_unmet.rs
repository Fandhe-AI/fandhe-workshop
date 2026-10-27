//! 条件を満たさない開始を拒否として記録する試験（TASK-240 1/2、MOD-7 (a)。記録は `docs/testing.md` の 5.331）。
//!
//! `module-reuse-rounds` の 2 周目の開始点 `s-r2` は、1 周目の `r1-impl` の達成を `requires` に持つ。
//! 条件を満たさない開始で、(1) 1 周目の途中の `Begin` は状態・下書き・チェックポイント・ヒント・世代を 1 ビットも変えず
//! `Refused` を 1 件だけ記録し、(2) 最初から 2 周目の開始点で起動すると開始が失敗して `StartUnmet` を示し、
//! (3) 示す文面は分類が順序違いで次の行動が空でないことを確かめる。
//! 切る位置は `/init-claude` の直前（`r1-impl` に入る前で、世界状態・切り身・チェックポイントが空でない時点）である。
//! 範囲外: 存在しない開始 ID・1 周目を終えた後の開始の成功（TASK-240 2/2）。呼び出し元は CI の `make test` だけである。

mod common;
#[path = "common/rounds.rs"]
mod rounds;
#[path = "common/sha256.rs"]
mod sha256;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::rejection_text::{resolve_rejection, RejectionSource};
use fandhe_workshop_core::start_gate::{check_start, StartRefusal};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{round1_plan, run_plan, PlanStep};

const TEXT_ID: &str = "rj-start-needs-previous-round";

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

/// `log` を除いた状態の SHA-256。`log` は拒否の 1 件が足されるので比べる対象から外す。
fn state_hash(state: &SessionState) -> String {
    let mut copy = state.clone();
    copy.log.clear();
    let json = serde_json::to_vec(&copy).expect("serialize");
    sha256::hex(&sha256::sha256(&json))
}

/// 開始の失敗の記録が `Refused` 1 件だけで、操作前の世代と現在の Step を指すことを確かめる。
fn assert_refused_once(events: &[Event], before: &SessionState, after: &SessionState) {
    let step = before
        .run
        .current
        .clone()
        .unwrap_or_else(|| StepUid(String::new()));
    assert!(
        matches!(
            events,
            [Event::Refused {
                at,
                op: ControlKind::Begin,
                reason: RefuseReason::StartUnmet,
                rejection: Some(id),
            }] if id == TEXT_ID && at.generation == before.run.generation && at.step == step
        ),
        "{events:?}"
    );
    assert_eq!(after.log.len(), before.log.len() + 1);
    assert_eq!(after.log.last(), events.first());
}

#[test]
fn mod7_begin_round2_mid_round1_refuses_without_change() {
    let ir = ir();
    let plan = round1_plan(&ir);
    let at = plan
        .iter()
        .position(|s| {
            matches!(s, PlanStep::Op(Op::Input(Input::Command(c))) if c.starts_with("/init-claude"))
        })
        .expect("/init-claude in round 1 plan");
    let mut session = Session::new(ir.clone());
    run_plan(&mut session, &plan[..at]);
    let _ = session.apply(Op::Control(Control::Save)).to_vec();
    let _ = session.apply(Op::Control(Control::OpenHint)).to_vec();
    let mut state = session.state().clone();
    let root = state.draft_root().expect("current root");
    state.drafts.insert(root, "書きかけの下書き".to_string());
    if state.hints_opened.is_empty() {
        let cur = state.run.current.clone().expect("current");
        state.hints_opened.insert(cur, 1);
    }
    let mut session = Session::from_state(ir.clone(), state);

    // 前提（空振りの防止）
    let before = session.state().clone();
    let impl_uid = StepUid("main/r1-impl".into());
    assert_ne!(session.step_status(&impl_uid), StepStatus::Achieved);
    assert!(!before.drafts.is_empty());
    assert!(!before.checkpoints.is_empty());
    assert!(!before.hints_opened.is_empty());
    assert!(!before.run.slices.is_empty());
    assert!(matches!(
        check_start(&ir, &before.run, "s-r2"),
        Err(StartRefusal::Unmet { .. })
    ));

    let events = session
        .apply(Op::Control(Control::Begin("s-r2".into())))
        .to_vec();
    let after = session.state();
    assert_refused_once(&events, &before, after);
    assert_eq!(state_hash(after), state_hash(&before));
    assert_eq!(after.run.generation, before.run.generation);
    assert_eq!(after.current_draft(), before.current_draft());
    assert_eq!(after.run, before.run);
}

#[test]
fn mod7_fresh_boot_at_round2_start_fails_with_start_unmet() {
    let ir = ir();
    let mut session = Session::new(ir.clone());
    let before = session.state().clone();
    assert!(before.run.current.is_none());
    let events = session
        .apply(Op::Control(Control::Begin("s-r2".into())))
        .to_vec();
    let after = session.state();
    assert_refused_once(&events, &before, after);
    assert!(after.run.current.is_none(), "開始は失敗する");
    assert!(!events.iter().any(|e| matches!(
        e,
        Event::Begun { .. } | Event::Entered { .. } | Event::Skipped { .. }
    )));
    assert_eq!(state_hash(after), state_hash(&before));
    match check_start(&ir, &before.run, "s-r2") {
        Err(StartRefusal::Unmet { start, text: Ok(r) }) => {
            assert_eq!(start, "s-r2");
            assert_eq!(r.text, TEXT_ID);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn mod7_unmet_text_is_wrong_order_with_next_action() {
    let ir = ir();
    let start = ir.starts.iter().find(|s| s.id == "s-r2").expect("s-r2");
    let node = ir
        .nodes
        .iter()
        .find(|n| n.uid == start.node)
        .expect("start node");
    let r = resolve_rejection(&ir, &node.step, RejectionSource::Rule(TEXT_ID)).expect("resolve");
    assert_eq!(r.category, RejectCategory::WrongOrder);
    assert!(!r.next_action.as_str().trim().is_empty());
}
