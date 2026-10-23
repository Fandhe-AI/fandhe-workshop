//! 入口の写しとチェックポイントの構成の試験（TASK-242 1/2、MOD-10）。
//!
//! 確かめること。
//!
//! - 状態の入口の写しは「無い」か「現在の Step のもの」の 1 つだけで（不変条件 I-1）、Step に入るたびに置き換わる
//!   （遷移・呼び出し元への戻り）。古い Step の写しがチェックポイントや停止の戻り先に残らない
//! - チェックポイントは全体の状態の写し・入口の写し・保存の時点のログの長さ・持ち主の根を持ち、
//!   通し番号 `next_checkpoint` は `Session` で 1 つである
//! - 任意の操作列でも I-1 と通し番号の規則が保たれる（性質テスト。統計は出力だけで assert しない）
//!
//! 停止が世界状態を「写しの後に持ち主が書いた経路だけ」戻すことは `tests/stop_owner_slice.rs`（TASK-242 2/2）。
//! 変異の定義は `tests/mutations/MOD10-1.mutation`。呼び出し元は CI の `make test` のみ。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{
    guided_op_specs, guided_op_specs_with_reload, op_specs, op_specs_with_reload, run_with,
    Applied, OpSpec,
};
use common::property::{check, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::instance::owner_of;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{apply, load_ir, Session};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn confirm_ui() -> Op {
    Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/draft".into(),
    })
}

/// 受付から進行を最後まで流して、st-1 の処理を終える。
fn finish_processing(ir: &Ir, mut state: SessionState) -> SessionState {
    state = apply(ir, state, first_note()).0;
    while state.run.busy.is_some() {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

/// 開始の写しを持つ st-1 から、達成・遷移して st-2 に入った直後の状態。
fn entered_st2(ir: &Ir) -> SessionState {
    let state = Session::entered_at(ir.clone(), "main/st-1").into_state();
    assert!(state.entry.is_some(), "開始の写しがある");
    let state = finish_processing(ir, state);
    let (state, events) = apply(ir, state, confirm_ui());
    assert!(
        events.iter().any(|e| matches!(e, Event::Entered { .. })),
        "{events:?}"
    );
    assert_eq!(state.run.current, Some(uid("main/st-2")));
    state
}

/// I-1: 状態の入口の写しは無いか、現在の Step のもの。
fn entry_is_current(s: &SessionState) -> bool {
    s.entry
        .as_ref()
        .is_none_or(|e| e.run.current == s.run.current)
}

#[test]
fn entering_next_step_replaces_the_entry_copy() {
    let ir = ir();
    let state = entered_st2(&ir);
    assert!(entry_is_current(&state), "{:?}", state.entry);
}

#[test]
fn checkpoint_after_transition_has_no_stale_entry() {
    let ir = ir();
    let state = entered_st2(&ir);
    let (state, _) = apply(&ir, state, save());
    let cp = state
        .checkpoints
        .get(&CheckpointId(1))
        .expect("checkpoint 1");
    assert!(
        cp.entry
            .as_ref()
            .is_none_or(|e| e.run.current == cp.run.current),
        "{:?}",
        cp.entry
    );
}

#[test]
fn stop_after_reentering_the_same_step_keeps_earlier_world() {
    // st-1 の次を st-1 自身にして、同じ Step に入り直す（巡回）。古い写し（開始の時点）へ戻ると、
    // 1 周目の世界状態まで取り消されるので、戻り先が入り直した後の受付の時点であることを確かめる。
    let mut ir = ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-1" {
            for t in &mut n.step.next {
                t.to = "main/st-1".into();
            }
        }
    }
    let state = Session::entered_at(ir.clone(), "main/st-1").into_state();
    let state = finish_processing(&ir, state);
    let (state, _) = apply(&ir, state, confirm_ui());
    assert_eq!(state.run.current, Some(uid("main/st-1")));
    assert!(entry_is_current(&state));
    let world_before = state.run.world.clone();
    assert!(!world_before.is_empty(), "1 周目の世界状態がある");
    let (state, _) = apply(&ir, state, first_note());
    let (state, _) = if state.run.busy.is_some() {
        apply(&ir, state, Op::Control(Control::Stop))
    } else {
        (state, Vec::new())
    };
    assert_eq!(state.run.busy, None);
    for (k, v) in &world_before {
        assert_eq!(state.run.world.get(k), Some(v), "{k} が戻っていない");
    }
}

#[test]
fn checkpoint_has_the_four_parts() {
    let ir = ir();
    // 処理中でない（開始の写しあり）と処理中の 2 通り。
    let idle = Session::entered_at(ir.clone(), "main/st-1").into_state();
    let busy = apply(&ir, idle.clone(), first_note()).0;
    assert!(busy.run.busy.is_some());
    for before in [idle, busy] {
        let log_len = u32::try_from(before.log.len()).expect("len");
        let (after, events) = apply(&ir, before.clone(), save());
        let cp = after
            .checkpoints
            .get(&CheckpointId(1))
            .expect("checkpoint 1");
        assert_eq!(cp.run, before.run);
        assert_eq!(cp.entry, before.entry);
        assert_eq!(cp.log_len, log_len);
        let cur = before.run.current.as_ref().expect("current");
        assert_eq!(cp.owner, Some(owner_of(&ir, cur)));
        assert!(
            events.iter().any(|e| matches!(
                e,
                Event::CheckpointSaved { id, log_len: l, .. } if *id == CheckpointId(1) && *l == log_len
            )),
            "{events:?}"
        );
    }
}

#[test]
fn checkpoint_numbering_is_single_across_steps_and_rounds() {
    let ir = ir();
    let state = Session::entered_at(ir.clone(), "main/st-1").into_state();
    let (state, _) = apply(&ir, state, save());
    let state = finish_processing(&ir, state);
    let (state, _) = apply(&ir, state, confirm_ui());
    let (state, _) = apply(&ir, state, save());
    assert_eq!(state.next_checkpoint, CheckpointId(3));
    assert_eq!(
        state.checkpoints.keys().copied().collect::<Vec<_>>(),
        [CheckpointId(1), CheckpointId(2)]
    );
    // 開始・移動・復帰・停止は通し番号も保存済みも変えない。
    let ops = [
        Op::Control(Control::Begin("s-1".into())),
        Op::Control(Control::GoTo(uid("main/st-1"))),
        Op::Control(Control::Restore(CheckpointId(1))),
        Op::Control(Control::Restore(CheckpointId(2))),
        Op::Control(Control::Stop),
    ];
    let mut cur = state.clone();
    for op in ops {
        cur = apply(&ir, cur, op).0;
        assert_eq!(cur.next_checkpoint, CheckpointId(3));
        assert_eq!(cur.checkpoints, state.checkpoints);
    }
    // 周回をまたいだ後の保存は 3 を振る。
    let (cur, _) = apply(&ir, cur, save());
    assert!(cur.checkpoints.contains_key(&CheckpointId(3)));
    assert_eq!(cur.next_checkpoint, CheckpointId(4));
}

/// 各操作の後の不変条件。
fn check_invariants(i: usize, a: &Applied) -> Result<(), TestCaseError> {
    let s = &a.after;
    if !entry_is_current(s) {
        return Err(TestCaseError::fail(format!("I-1 違反（操作 {i}）")));
    }
    if s.checkpoints.keys().any(|k| k.0 >= s.next_checkpoint.0) {
        return Err(TestCaseError::fail(format!("通し番号の違反（操作 {i}）")));
    }
    let saved = a
        .events
        .iter()
        .filter(|e| matches!(e, Event::CheckpointSaved { .. }))
        .count();
    let expected = a.before.next_checkpoint.0 + u32::try_from(saved).unwrap_or(u32::MAX);
    if s.next_checkpoint.0 != expected || saved > 1 {
        return Err(TestCaseError::fail(format!(
            "通し番号が保存と一致しない（操作 {i}）"
        )));
    }
    Ok(())
}

fn run_property<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let ir = ir();
    let initial = load().initial;
    check(name, DEFAULT_CASES, strategy, |specs| {
        run_with(&ir, initial.clone(), &specs, check_invariants).map(|_| ())
    });
}

#[test]
fn invariants_hold_for_fixed_sequences() {
    run_property("entry_copy_fixed", op_specs());
}

#[test]
fn invariants_hold_for_guided_sequences() {
    run_property("entry_copy_guided", guided_op_specs());
}

#[test]
fn invariants_hold_for_fixed_sequences_with_reload() {
    run_property("entry_copy_fixed_reload", op_specs_with_reload());
}

#[test]
fn invariants_hold_for_guided_sequences_with_reload() {
    run_property("entry_copy_guided_reload", guided_op_specs_with_reload());
}
