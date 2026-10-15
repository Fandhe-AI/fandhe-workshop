//! 確認を省く明示の Step の、受付と同時の達成の試験（TASK-208、EVT-5）。
//!
//! 3 Step の教材（`min-3step.json`）の st-2（選択の入力、`confirm: none` と理由）で、受付の 1 回の操作が
//! `Accepted` と `Achieved` を同時に出して次の Step に進むこと、明示の無い Step と不正な省略の宣言では
//! 達成にならないことを確かめる。呼び出し元は CI の `make test` のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

const REASON_MARK: &str = "SECRET-REASON-MARK";

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-2 を書き換えた教材。
fn ir_with(f: impl FnOnce(&mut Step)) -> Ir {
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/st-2") {
        f(&mut n.step);
    }
    ir
}

fn skip(reason: Value) -> Value {
    json!({ "kind": "none", "reason": reason })
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn st2_state() -> SessionState {
    let u = uid("main/st-2");
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "created".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn send(ir: Ir, input: Input) -> (Session, Vec<Event>) {
    let mut s = Session::from_state(ir, st2_state());
    let events = s.apply(Op::Input(input)).to_vec();
    (s, events)
}

fn memo() -> Input {
    Input::Choice("opt-memo".into())
}

fn kinds(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::InputReceived { .. } => "input_received",
            Event::Accepted { .. } => "accepted",
            Event::Achieved { .. } => "achieved",
            Event::Transitioned { .. } => "transitioned",
            Event::Entered { .. } => "entered",
            Event::Changed { .. } => "changed",
            Event::Warning { .. } => "warning",
            _ => "other",
        })
        .collect()
}

fn status(s: &Session, step: &str) -> StepStatus {
    let u = uid(step);
    s.state()
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

fn count_achieved(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

#[test]
fn explicit_skip_achieves_on_accept_and_transitions() {
    let (s, events) = send(
        ir_with(|st| st.confirm = Some(skip(json!("memo only")))),
        memo(),
    );
    assert_eq!(
        kinds(&events),
        [
            "input_received",
            "accepted",
            "achieved",
            "transitioned",
            "entered",
            "changed"
        ],
        "{events:?}"
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Achieved {
            explicit_skip: false,
            ..
        }
    )));
    assert_eq!(status(&s, "main/st-2"), StepStatus::Achieved);
    assert_eq!(status(&s, "main/st-3"), StepStatus::Entered);
    assert_eq!(s.state().run.current, Some(uid("main/st-3")));
}

#[test]
fn undeclared_steps_never_achieve_on_accept() {
    let variants: Vec<Option<Value>> = vec![
        None,
        Some(json!({ "kind": "approve" })),
        Some(json!({ "kind": "point_at", "targets": [{ "view": "v", "path": "p" }] })),
    ];
    for confirm in variants {
        let (s, events) = send(ir_with(|st| st.confirm = confirm.clone()), memo());
        assert_eq!(count_achieved(&events), 0, "{confirm:?}: {events:?}");
        assert_eq!(status(&s, "main/st-2"), StepStatus::Accepted, "{confirm:?}");
        assert_eq!(s.state().run.current, Some(uid("main/st-2")));
    }
}

fn assert_rejected(ir: Ir) {
    let (s, events) = send(ir, memo());
    assert_eq!(kinds(&events), ["input_received", "warning"], "{events:?}");
    assert_eq!(status(&s, "main/st-2"), StepStatus::Entered);
    assert_eq!(s.state().run, st2_state().run);
}

#[test]
fn skip_without_reason_is_rejected_without_state_change() {
    assert_rejected(ir_with(|st| st.confirm = Some(json!({ "kind": "none" }))));
    assert_rejected(ir_with(|st| st.confirm = Some(skip(json!("")))));
    assert_rejected(ir_with(|st| st.confirm = Some(skip(json!("  \t\n")))));
    assert_rejected(ir_with(|st| st.confirm = Some(skip(json!(null)))));
}

#[test]
fn skip_with_unknown_field_is_rejected() {
    assert_rejected(ir_with(|st| {
        st.confirm = Some(json!({ "kind": "none", "reason": "ok", "extra": 1 }));
    }));
}

#[test]
fn skip_with_unreadable_on_achieved_is_rejected() {
    assert_rejected(ir_with(|st| {
        st.confirm = Some(skip(json!("ok")));
        st.on_achieved = vec![json!({ "bogus": true })];
    }));
}

#[test]
fn non_matching_input_does_not_achieve() {
    let (s, events) = send(
        ir_with(|st| st.confirm = Some(skip(json!("ok")))),
        Input::Choice("opt-ghost".into()),
    );
    assert_eq!(kinds(&events), ["input_received"], "{events:?}");
    assert_eq!(status(&s, "main/st-2"), StepStatus::Entered);
}

#[test]
fn skip_with_processing_stays_accepted_and_busy() {
    let (s, events) = send(
        ir_with(|st| {
            st.confirm = Some(skip(json!("ok")));
            st.processing = vec![json!({ "label": "x" })];
        }),
        memo(),
    );
    assert_eq!(count_achieved(&events), 0, "{events:?}");
    assert_eq!(status(&s, "main/st-2"), StepStatus::Accepted);
    assert!(s.state().run.busy.is_some());
    assert_eq!(s.state().run.current, Some(uid("main/st-2")));
}

#[test]
fn unknown_transition_target_warns_after_achieved() {
    let (s, events) = send(
        ir_with(|st| {
            st.confirm = Some(skip(json!("ok")));
            for t in &mut st.next {
                t.to = "main/ghost".to_string();
            }
        }),
        memo(),
    );
    let k = kinds(&events);
    let a = k.iter().position(|x| *x == "achieved").expect("achieved");
    assert_eq!(k.get(a + 1), Some(&"warning"), "{events:?}");
    assert!(!k.contains(&"transitioned"));
    assert_eq!(s.state().run.current, Some(uid("main/st-2")));
}

#[test]
fn warnings_do_not_embed_reason_or_input() {
    let (_s, events) = send(
        ir_with(|st| {
            st.confirm = Some(json!({ "kind": "none", "reason": REASON_MARK, "extra": 1 }));
        }),
        memo(),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Warning { .. })));
    for e in &events {
        if let Event::Warning { message, .. } = e {
            assert!(!message.contains(REASON_MARK) && !message.contains("opt-memo"));
        }
    }
}
