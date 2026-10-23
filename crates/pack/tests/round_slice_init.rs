//! 次の周回の開始時の切り身とフラグの初期状態の試験（MOD-3、TASK-235 1/2）。
//!
//! `module-reuse-rounds` をコンパイルした IR で、1 周目の終了状態（全 Step が達成・フラグが立っている）から
//! 2 周目の開始点へ入ったとき、2 周目の根の Step が現在の Step と呼び出しの Step を除きすべて `NotStarted` で、
//! フラグが空であることを確かめる。2 周目の根は IR の開始点と実体の呼び出し元から導き、実体 ID を直書きしない
//! （1 周目のフラグを立てる実体だけ、spec の例の名前 `issue-to-merge@round1` を使う）。
//! 2 周目の終了後に 1 周目の切り身が不変であることは TASK-235 2/2 が持つ。

mod common;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::owner_of;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

/// 開始点 `start_id` の節点と同じ `unit` の `main` の節点を呼び出し元とする実体の根（IR の順）と、開始の節点。
fn round_roots(ir: &Ir, start_id: &str) -> (Vec<Root>, String) {
    let node_id = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .expect("start")
        .node
        .clone();
    let unit = ir
        .nodes
        .iter()
        .find(|n| n.uid == node_id)
        .expect("start node")
        .unit
        .clone();
    let callers: Vec<&str> = ir
        .nodes
        .iter()
        .filter(|n| n.scope == "main" && n.unit == unit && n.enter.is_some())
        .map(|n| n.uid.as_str())
        .collect();
    let roots = ir
        .instances
        .iter()
        .filter(|i| i.caller.as_deref().is_some_and(|c| callers.contains(&c)))
        .map(|i| Root(i.id.clone()))
        .collect();
    (roots, node_id)
}

fn steps(ir: &Ir, roots: &[Root]) -> Vec<StepUid> {
    roots
        .iter()
        .flat_map(|r| fandhe_workshop_core::instance::steps_of(ir, r))
        .collect()
}

/// 1 周目の終了状態。1 周目の全 Step を達成にし、`issue-to-merge@round1` のフラグを立てる。
/// 入力で完走させる部品は未実装（`when` つきの `next` の評価なし）のため、状態を直接組む。
fn finished_round1(ir: &Ir, r1: &[Root]) -> Session {
    let mut state = SessionState::default();
    for u in steps(ir, r1) {
        let owner = owner_of(ir, &u);
        state
            .run
            .slices
            .entry(owner)
            .or_default()
            .status
            .insert(u, StepStatus::Achieved);
    }
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .expect("issue-to-merge round1");
    let slice = state.run.slices.entry(stopper.clone()).or_default();
    slice.flags.insert("stopped".into());
    slice.flags.insert("precheck_ok".into());
    // 移動の元になる現在の Step は、1 周目の最後の呼び出しの Step に置く。
    state.run.current = steps(ir, r1)
        .into_iter()
        .rfind(|u| ir.nodes.iter().any(|n| n.uid == u.0 && n.enter.is_some()));
    Session::from_state(ir.clone(), state)
}

#[test]
fn second_round_roots_hold_eighteen_steps_with_two_call_steps() {
    let ir = ir();
    let (r2, _) = round_roots(&ir, "s-r2");
    assert_eq!(r2.len(), 2, "round 2 has two module instances");
    let all = steps(&ir, &r2);
    // 8 + 8 + 呼び出しの Step 2 件（spec の MOD-3 の 18 件）。
    assert_eq!(all.len(), 18);
    let calls = all
        .iter()
        .filter(|u| ir.nodes.iter().any(|n| n.uid == u.0 && n.enter.is_some()))
        .count();
    assert_eq!(calls, 2);
}

#[test]
fn second_round_is_untouched_before_it_starts() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let s = finished_round1(&ir, &r1);
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .unwrap();
    assert!(s.slice(stopper).unwrap().flags.contains("stopped"));
    for u in steps(&ir, &r2) {
        assert_eq!(s.step_status(&u), StepStatus::NotStarted, "{}", u.0);
    }
    for r in &r2 {
        assert!(s.slice(r).is_none_or(|sl| sl.flags.is_empty()));
    }
}

#[test]
fn second_round_starts_with_only_the_current_and_call_steps_entered() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, call_node) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    // `Control::Begin`（TASK-237 1/2）が実装されたら、`GoTo` を `Begin` に置き換えて同じ表明を流し直す。
    s.apply(Op::Control(Control::GoTo(StepUid(call_node.clone()))));
    let current = s.state().run.current.clone().expect("current");
    let all = steps(&ir, &r2);
    let entered: Vec<&StepUid> = all
        .iter()
        .filter(|u| s.step_status(u) == StepStatus::Entered)
        .collect();
    assert_eq!(entered.len(), 2, "{entered:?}");
    assert!(entered.contains(&&current));
    assert!(entered.iter().any(|u| u.0 == call_node));
    for u in all.iter().filter(|u| !entered.contains(u)) {
        assert_eq!(s.step_status(u), StepStatus::NotStarted, "{}", u.0);
    }
    // 呼び出しの Step の状態は呼ばれた実体の切り身にあり、`main` には無い。
    let main = s.slice(&Root("main".into()));
    assert!(main.is_none_or(|sl| !sl.status.contains_key(&StepUid(call_node.clone()))));
    // 2 周目のフラグは空で、1 周目のフラグは立ったまま。
    for r in &r2 {
        assert!(s.slice(r).is_none_or(|sl| sl.flags.is_empty()));
    }
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .unwrap();
    assert!(s.slice(stopper).unwrap().flags.contains("stopped"));
}
