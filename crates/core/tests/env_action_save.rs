//! 保存を Step の入力として宣言した Step（`env_action`、`action: save`）の試験（TASK-64 1/2、PACK-34・D-69）。
//!
//! 試験データ `tests/data/env_action_1/ir.json` は pack の試験用教材（`crates/pack/tests/data/env-action-pack`）を
//! compile した IR で、保存の Step（`st-save`）・書き出しの Step（`st-export`）・終端（`st-end`）を持つ。
//! 両者の一致は pack 側の `tests/env_action_input.rs` が確かめる。確かめるのは次の点である。
//! - 保存の成功で保存の Step が受付になり、確認を省く宣言なら達成して次の Step へ進む（イベントの順を含む）
//! - 確認の宣言が `none` でなければ `Accepted` で止まる（受付と達成は別）
//! - 書き出しの Step・宣言を持たない Step・保存に失敗したときは、保存で受付にならない
//! - 受講者の入力は `env_action` の Step を受付にしない
//! - 世代は変わらない。同じ操作列は同じ状態とログになり、ログから `Projection` を導ける
//!
//! 試験では `unwrap` を使ってよい。

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use serde_json::{json, Value};

fn ir_value() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/env_action_1/ir.json"
    );
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn load(v: &Value) -> Ir {
    load_ir(&serde_json::to_vec(v).unwrap()).unwrap()
}

fn ir() -> Ir {
    load(&ir_value())
}

/// `uid` の Step の `step` の欄を書き換えた IR。
fn ir_with(uid: &str, edit: impl Fn(&mut Value)) -> Ir {
    let mut v = ir_value();
    let node = v["nodes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|n| n["uid"] == uid)
        .unwrap();
    edit(&mut node["step"]);
    load(&v)
}

fn uid(step: &str) -> StepUid {
    StepUid(format!("main/{step}"))
}

/// `step` を現在の Step として入力待ち（`Entered`）にした状態。
fn state_at(step: &str) -> SessionState {
    let mut s = SessionState::default();
    let u = uid(step);
    s.run.current = Some(u.clone());
    s.run
        .slices
        .entry(u.root())
        .or_default()
        .status
        .insert(u, StepStatus::Entered);
    s
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn status(state: &SessionState, step: &str) -> StepStatus {
    let u = uid(step);
    state
        .run
        .slices
        .get(&u.root())
        .and_then(|s| s.status.get(&u))
        .copied()
        .unwrap_or_default()
}

/// 保存の結果のイベントの種類の並び（`Changed` は集約の都合で除く）。
fn shape(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::CheckpointSaved { .. } => Some("saved"),
            Event::Accepted { .. } => Some("accepted"),
            Event::Achieved { .. } => Some("achieved"),
            Event::Transitioned { .. } => Some("transitioned"),
            Event::Entered { .. } => Some("entered"),
            Event::Warning { .. } => Some("warning"),
            Event::InputReceived { .. } => Some("input"),
            Event::Judged { .. } => Some("judged"),
            _ => None,
        })
        .collect()
}

#[test]
fn pack34_save_achieves_a_step_that_declares_save() {
    let (state, events) = apply(&ir(), state_at("st-save"), save());
    assert_eq!(
        shape(&events),
        ["saved", "accepted", "achieved", "transitioned", "entered"],
        "{events:?}"
    );
    assert_eq!(status(&state, "st-save"), StepStatus::Achieved);
    assert_eq!(state.run.current, Some(uid("st-export")));
    assert_eq!(state.checkpoints.len(), 1, "保存は従来どおり写しを控える");
}

#[test]
fn pack34_save_does_not_change_the_generation() {
    let before = state_at("st-save");
    let (after, _) = apply(&ir(), before.clone(), save());
    assert_eq!(after.run.generation, before.run.generation);
}

#[test]
fn pack34_export_step_is_not_achieved_by_save() {
    let (state, events) = apply(&ir(), state_at("st-export"), save());
    assert_eq!(shape(&events), ["saved"], "{events:?}");
    assert_eq!(status(&state, "st-export"), StepStatus::Entered);
    assert_eq!(state.run.current, Some(uid("st-export")));
}

#[test]
fn pack34_step_without_declaration_keeps_the_old_events() {
    let ir = ir_with("main/st-save", |step| {
        step["input"] = json!({"kind": "wait"});
    });
    let (state, events) = apply(&ir, state_at("st-save"), save());
    assert_eq!(shape(&events), ["saved"], "{events:?}");
    assert_eq!(status(&state, "st-save"), StepStatus::Entered);
    // 宣言そのものが無い Step も同じ。
    let ir = ir_with("main/st-save", |step| {
        step.as_object_mut().unwrap().remove("input");
    });
    let (_, events) = apply(&ir, state_at("st-save"), save());
    assert_eq!(shape(&events), ["saved"], "{events:?}");
}

#[test]
fn pack34_unreadable_declaration_is_not_accepted() {
    let ir = ir_with("main/st-save", |step| {
        step["input"] = json!({"kind": "env_action", "action": "delete"});
    });
    let (state, events) = apply(&ir, state_at("st-save"), save());
    assert_eq!(shape(&events), ["saved"], "{events:?}");
    assert_eq!(status(&state, "st-save"), StepStatus::Entered);
}

#[test]
fn pack34_failed_save_does_not_accept() {
    // 次の ID があふれる（保存に失敗し `Warning` だけが出る）状態。
    let mut state = state_at("st-save");
    state.next_checkpoint = CheckpointId(u32::MAX);
    let (state, events) = apply(&ir(), state, save());
    assert_eq!(shape(&events), ["warning"], "{events:?}");
    assert_eq!(status(&state, "st-save"), StepStatus::Entered);
    assert_eq!(state.run.current, Some(uid("st-save")));
}

#[test]
fn pack34_save_without_current_step_is_a_no_op() {
    let (_, events) = apply(&ir(), SessionState::default(), save());
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn pack34_accept_and_achieve_are_separate_without_confirm_none() {
    // 確認を省く宣言を外す（確認の種類を既定にする）と、受付で止まって確認の操作を待つ。
    let ir = ir_with("main/st-save", |step| {
        step.as_object_mut().unwrap().remove("confirm");
    });
    let (state, events) = apply(&ir, state_at("st-save"), save());
    assert_eq!(shape(&events), ["saved", "accepted"], "{events:?}");
    assert_eq!(status(&state, "st-save"), StepStatus::Accepted);
    assert_eq!(state.run.current, Some(uid("st-save")));
    // 受付済みの Step での 2 回目の保存は、受付をやり直さない。
    let (_, again) = apply(&ir, state, save());
    assert_eq!(shape(&again), ["saved"], "{again:?}");
}

#[test]
fn pack34_invalid_confirm_none_warns_and_changes_nothing() {
    let ir = ir_with("main/st-save", |step| {
        step["confirm"] = json!({"kind": "none", "reason": "  "});
    });
    let (state, events) = apply(&ir, state_at("st-save"), save());
    assert_eq!(shape(&events), ["saved", "warning"], "{events:?}");
    assert_eq!(status(&state, "st-save"), StepStatus::Entered);
}

#[test]
fn pack34_busy_step_is_not_accepted() {
    let mut state = state_at("st-save");
    state.run.busy = Some(fandhe_workshop_core::session::Busy { done: 0, total: 2 });
    let (_, events) = apply(&ir(), state, save());
    assert_eq!(shape(&events), ["saved"], "{events:?}");
}

#[test]
fn pack34_learner_inputs_do_not_accept_an_env_action_step() {
    let inputs = [
        Input::Command("/make-note first-note".into()),
        Input::Choice("a".into()),
        Input::Cancel,
    ];
    for input in inputs {
        let before = state_at("st-save");
        let (after, events) = apply(&ir(), before.clone(), Op::Input(input.clone()));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::Accepted { .. } | Event::Achieved { .. })),
            "{input:?}: {events:?}"
        );
        assert_eq!(after.run.current, before.run.current, "{input:?}");
        assert_eq!(status(&after, "st-save"), StepStatus::Entered, "{input:?}");
        assert_eq!(after.run.world, before.run.world, "{input:?}");
    }
}

#[test]
fn pack34_replay_is_deterministic_and_projection_is_derivable() {
    let ops = [save(), save(), save()];
    let run = |ir: &Ir| {
        let mut s = state_at("st-save");
        for op in ops.clone() {
            s = apply(ir, s, op).0;
        }
        s
    };
    let ir = ir();
    let a = run(&ir);
    let b = run(&ir);
    assert_eq!(a.log, b.log);
    assert_eq!(a.run, b.run);
    // 2 回目の保存は書き出しの Step の上なので受付にならない。
    assert_eq!(a.run.current, Some(uid("st-export")));
    let p = Projection::from_events(&a.log);
    assert!(
        !p.log().is_empty(),
        "保存と受付・達成の行が右のログに並ぶこと"
    );
    assert_eq!(p.consumed(), a.log.len());
}
