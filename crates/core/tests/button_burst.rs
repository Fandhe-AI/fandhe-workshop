//! 残り続けるボタンへの連打と世代の不変の試験（TASK-209 2/2、仕様 EVT-7・EVT-6 (1)）。
//!
//! 受付後も画面から消えない承認のボタン（PoC の B-2c。st-3 の「いいえ」を 20 回押す）は、20 回とも
//! 規則による拒否（`not_in_this_step`、文面 `rj-decline`）になる。ログは 1 回につき `InputReceived` と
//! `Judged` の 2 件ずつ増えるだけで、Step の状態・世界状態・世代は変わらない。
//! 世代の取り違えを検出できるよう、世代は既定値でない 2 で始める（B-2c と同じく移動の後の状態）。
//!
//! 処理中の Step への連打（`busy`）は `busy_resend.rs` が扱う。st-3 は処理中にならないため、
//! 本ファイルの拒否は `busy` ではない（送信の直前に `classify_busy` が `None` であることも確かめる）。
//! 呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::busy_class::classify_busy;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;

const ST3: &str = "main/st-3";
const GEN: Generation = Generation(2);

/// 最小の 3 Step の教材を読む。
fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-3 が `Entered`・処理中でない・世代 2 の状態。
fn entered_st3() -> SessionState {
    let u = StepUid(ST3.to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: GEN,
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "saved".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

/// 残り続けるボタン「いいえ」の入力。
fn reject() -> Input {
    Input::Approval {
        decision: Decision::Reject,
        target: None,
    }
}

/// `InputReceived` と、規則による拒否（`not_in_this_step`・`rj-decline`）の `Judged` だけか。
fn is_rule_rejection_only(events: &[Event]) -> bool {
    matches!(
        events,
        [Event::InputReceived { .. }, Event::Judged { judgment: Judgment::Rejected(r), .. }]
            if r.category == RejectCategory::NotInThisStep && r.text == "rj-decline"
    )
}

/// 全イベントの世代と Step が、st-3・世代 2 のままか。
fn stamps_unchanged(events: &[Event]) -> bool {
    events.iter().all(|e| match e {
        Event::InputReceived { at, .. } | Event::Judged { at, .. } => {
            at.generation == GEN && at.step.0 == ST3
        }
        _ => false,
    })
}

/// 1 回送り、直前に `busy` でなかったか・戻りのイベントを返す。
fn send(session: &mut Session) -> (bool, Vec<Event>) {
    let not_busy = classify_busy(&session.state().run, &reject()).is_none();
    let events = session.apply(Op::Input(reject())).to_vec();
    (not_busy, events)
}

/// 受付（`Accepted`）の件数。
fn accepted_count(state: &SessionState) -> usize {
    state
        .log
        .iter()
        .filter(|e| matches!(e, Event::Accepted { .. }))
        .count()
}

/// n 回連打し、セッション・連打前の状態・各回の戻りを返す。
fn burst(n: usize) -> (Session, SessionState, Vec<Vec<Event>>) {
    let mut session = Session::from_state(base_ir(), entered_st3());
    let before = session.state().clone();
    let mut all = Vec::new();
    for _ in 0..n {
        let (not_busy, events) = send(&mut session);
        assert!(not_busy, "st-3 への入力は busy ではない");
        all.push(events);
    }
    (session, before, all)
}

#[test]
fn burst_of_20_on_remaining_button_rejects_all_and_keeps_state() {
    // 前提の確認: 1 回目の戻りが想定の拒否であること（食い違いを 20 回のループより前に知らせる）。
    let mut probe = Session::from_state(base_ir(), entered_st3());
    let (_, first) = send(&mut probe);
    assert!(is_rule_rejection_only(&first), "first: {first:?}");

    let (session, before, all) = burst(20);
    for events in &all {
        assert!(is_rule_rejection_only(events), "{events:?}");
        assert!(stamps_unchanged(events), "{events:?}");
    }
    let state = session.state();
    assert_eq!(accepted_count(state), 0);
    assert_eq!(state.log.len(), before.log.len() + 40);
    assert_eq!(state.run, before.run);
    assert_eq!(session.generation(), GEN);
    let st3 = StepUid(ST3.to_string());
    let status = state
        .run
        .slices
        .get(&st3.root())
        .and_then(|s| s.status.get(&st3));
    assert_eq!(status, Some(&StepStatus::Entered));
}

#[test]
fn burst_on_remaining_button_is_deterministic() {
    let (a, _, ea) = burst(20);
    let (b, _, eb) = burst(20);
    assert_eq!(a.state(), b.state());
    assert_eq!(ea, eb);
}

#[test]
fn p6_remaining_button_burst_keeps_state() {
    check(
        "p6-remaining-button-burst",
        DEFAULT_CASES,
        1..=MAX_OPS,
        |n| {
            let (session, before, all) = burst(n);
            let state = session.state();
            prop_assert!(all.iter().all(|e| is_rule_rejection_only(e)));
            prop_assert_eq!(accepted_count(state), 0);
            prop_assert_eq!(state.log.len(), before.log.len() + 2 * n);
            prop_assert_eq!(&state.run, &before.run);
            prop_assert_eq!(session.generation(), GEN);
            Ok(())
        },
    );
}
