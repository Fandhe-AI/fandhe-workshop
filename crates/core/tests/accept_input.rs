//! 入力の受付の試験（TASK-207 1/3、EVT-3 の (a)）。
//!
//! 3 Step の教材（`min-3step.json`。コマンド＋引数・選択・承認）で、入力が受付の規則に当たると Step が
//! 受付済みになり、達成にはならないことを確かめる。呼び出し元は CI の `make test` のみである。
//! st-2 の `confirm: none` は TASK-208 で受付と同時の達成になるため、試験は写しの `confirm` を
//! `approve` に置き換えて使い、TASK-208 の実装後も壊れないようにする。

mod common;

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-2 の `confirm` を `none` から `approve` に置き換えた教材。
fn ir() -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-2" {
            n.step.confirm = Some(json!({ "kind": "approve" }));
        }
    }
    ir
}

fn root() -> Root {
    Root("main".to_string())
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// 指定の Step に入って入力を待つ状態（サンプルの `initial` と同じ形）。
fn entered(step: &str) -> SessionState {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u),
        ..RunState::default()
    };
    run.slices.insert(uid(step).root(), slice);
    if step != "main/st-1" {
        // st-1 の受付が置いた下書き。st-2・st-3 の `set_attr` が当たる対象である。
        let mut el = Element::default();
        el.attrs.insert("kind".into(), "none".into());
        el.attrs.insert("state".into(), "created".into());
        run.world.insert("notes/draft".into(), el);
    }
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn session(ir: Ir, step: &str) -> Session {
    Session::from_state(ir, entered(step))
}

fn send(s: &mut Session, input: Input) -> Vec<Event> {
    s.apply(Op::Input(input)).to_vec()
}

fn cmd(s: &str) -> Input {
    Input::Command(s.to_string())
}

fn kinds(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::InputReceived { .. } => "input_received",
            Event::Accepted { .. } => "accepted",
            Event::Changed { .. } => "changed",
            Event::Achieved { .. } => "achieved",
            Event::Warning { .. } => "warning",
            Event::Flag { .. } => "flag",
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

fn no_achieved(s: &Session) {
    assert!(
        !s.log().iter().any(|e| matches!(e, Event::Achieved { .. })),
        "受付だけで Achieved を出してはならない"
    );
    assert!(!s
        .state()
        .run
        .slices
        .values()
        .flat_map(|sl| sl.status.values())
        .any(|st| *st == StepStatus::Achieved));
    assert!(!s.state().run.completed);
}

fn attr(s: &Session, key: &str) -> Option<String> {
    s.state()
        .run
        .world
        .get("notes/draft")
        .and_then(|e| e.attrs.get(key))
        .cloned()
}

fn only_input_received(events: &[Event]) {
    assert_eq!(kinds(events), ["input_received"], "{events:?}");
}

#[test]
fn command_accept_makes_step_accepted_not_achieved() {
    let mut s = session(ir(), "main/st-1");
    let events = send(&mut s, cmd("/make-note first-note"));
    assert_eq!(kinds(&events), ["input_received", "accepted", "changed"]);
    assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
    assert_eq!(attr(&s, "state").as_deref(), Some("created"));
    assert_eq!(attr(&s, "kind").as_deref(), Some("none"));
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 5 }));
    assert_eq!(s.state().run.current, Some(uid("main/st-1")));
    no_achieved(&s);
}

#[test]
fn command_with_bad_shape_is_only_received() {
    let mut s = session(ir(), "main/st-1");
    let before = s.state().run.clone();
    let events = send(&mut s, cmd("/make-note Bad_Name"));
    only_input_received(&events);
    assert_eq!(s.state().run, before);
}

#[test]
fn resend_after_accept_is_only_received() {
    let mut s = session(ir(), "main/st-1");
    send(&mut s, cmd("/make-note first-note"));
    let before = s.state().run.clone();
    let events = send(&mut s, cmd("/make-note first-note"));
    only_input_received(&events);
    assert_eq!(s.state().run, before);
}

#[test]
fn choice_accept_records_choice() {
    let mut s = session(ir(), "main/st-2");
    let events = send(&mut s, Input::Choice("opt-memo".into()));
    assert_eq!(kinds(&events), ["input_received", "accepted", "changed"]);
    assert_eq!(status(&s, "main/st-2"), StepStatus::Accepted);
    assert_eq!(
        s.state().run.slices[&root()].choices.get(&uid("main/st-2")),
        Some(&"opt-memo".to_string())
    );
    assert_eq!(attr(&s, "kind").as_deref(), Some("memo"));
    assert_eq!(s.state().run.busy, None);
    no_achieved(&s);
}

#[test]
fn undeclared_choice_is_only_received() {
    let mut s = session(ir(), "main/st-2");
    let before = s.state().run.clone();
    only_input_received(&send(&mut s, Input::Choice("opt-ghost".into())));
    assert_eq!(s.state().run, before);
}

#[test]
fn approval_accept_is_not_achievement() {
    let mut s = session(ir(), "main/st-3");
    let events = send(
        &mut s,
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
    );
    assert_eq!(kinds(&events), ["input_received", "accepted", "changed"]);
    assert_eq!(status(&s, "main/st-3"), StepStatus::Accepted);
    assert_eq!(attr(&s, "state").as_deref(), Some("published"));
    no_achieved(&s);
}

#[test]
fn rejection_by_rule_is_only_received() {
    let mut s = session(ir(), "main/st-3");
    let before = s.state().run.clone();
    let events = send(
        &mut s,
        Input::Approval {
            decision: Decision::Reject,
            target: None,
        },
    );
    only_input_received(&events);
    assert_eq!(s.state().run, before);
}

#[test]
fn wrong_input_kind_is_only_received() {
    let mut s = session(ir(), "main/st-1");
    only_input_received(&send(&mut s, Input::Choice("opt-memo".into())));
    let mut s = session(ir(), "main/st-3");
    only_input_received(&send(&mut s, cmd("/make-note first-note")));
    assert_eq!(status(&s, "main/st-3"), StepStatus::Entered);
}

#[test]
fn cancel_is_only_received() {
    let mut s = session(ir(), "main/st-1");
    let before = s.state().run.clone();
    only_input_received(&send(&mut s, Input::Cancel));
    assert_eq!(s.state().run, before);
}

#[test]
fn text_accept_is_not_achievement() {
    let mut ir = ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-1" {
            n.step.input = Some(json!({ "kind": "text", "min": 1, "max": 50 }));
            n.step.rules = vec![json!({
                "id": "r-text",
                "on": { "text": true },
                "outcome": { "kind": "accept", "effects": [
                    { "put": { "path": "notes/memo", "attrs": { "state": "written" } } }
                ] }
            })];
        }
    }
    let mut s = session(ir, "main/st-1");
    let events = send(&mut s, Input::Text("今日のメモ\n二行目".into()));
    assert_eq!(kinds(&events), ["input_received", "accepted", "changed"]);
    assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
    // processing を持つ Step なので処理中になる。
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 5 }));
    no_achieved(&s);
}

#[test]
fn no_current_step_changes_nothing() {
    let mut s = Session::new(ir());
    let events = send(&mut s, cmd("/make-note first-note"));
    assert!(events.is_empty());
    assert_eq!(s.state(), &SessionState::default());
}

#[test]
fn three_steps_never_achieve_and_replay_is_deterministic() {
    let run = || {
        let mut all = Vec::new();
        let mut s = session(ir(), "main/st-1");
        all.extend(send(&mut s, cmd("/make-note Bad_Name")));
        all.extend(send(&mut s, cmd("/make-note first-note")));
        all.extend(send(&mut s, cmd("/make-note first-note")));
        no_achieved(&s);
        // 次の Step へ進む操作は本件の範囲外なので、状態を組み直して順に流す。
        let mut st = s.into_state();
        for step in ["main/st-2", "main/st-3"] {
            st.run.current = Some(uid(step));
            st.run.busy = None;
            if let Some(sl) = st.run.slices.get_mut(&root()) {
                sl.status.insert(uid(step), StepStatus::Entered);
            }
            let mut s = Session::from_state(ir(), st);
            let input = if step == "main/st-2" {
                Input::Choice("opt-todo".into())
            } else {
                Input::Approval {
                    decision: Decision::Approve,
                    target: None,
                }
            };
            all.extend(send(&mut s, input));
            no_achieved(&s);
            // 戻りのイベント列はログの末尾と一致する。
            assert!(s.log().ends_with(&all[all.len() - 3..]));
            st = s.into_state();
        }
        (st, all)
    };
    let (a, ev_a) = run();
    let (b, ev_b) = run();
    assert_eq!((a.clone(), ev_a.clone()), (b, ev_b));
    assert_eq!(a.run.slices[&root()].choices.len(), 1);
    assert!(!ev_a.iter().any(|e| matches!(e, Event::Achieved { .. })));
}

#[test]
fn invalid_rule_effects_warn_and_change_nothing() {
    let mut ir = ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-3" {
            if let Some(Value::Object(o)) = n.step.rules.get_mut(0) {
                o["outcome"] = json!({ "kind": "accept", "effects": "not-an-array" });
            }
        }
    }
    let mut s = session(ir, "main/st-3");
    let before = s.state().run.clone();
    let events = send(
        &mut s,
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
    );
    assert_eq!(kinds(&events), ["input_received", "warning"]);
    assert_eq!(s.state().run, before);
}

#[test]
fn rule_with_extra_args_key_does_not_match_command() {
    let mut ir = ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-1" {
            if let Some(Value::Object(o)) = n.step.rules.get_mut(0) {
                o["on"]["args"] = json!({ "name": "x" });
            }
        }
    }
    let mut s = session(ir, "main/st-1");
    only_input_received(&send(&mut s, cmd("/make-note first-note")));
    assert_eq!(status(&s, "main/st-1"), StepStatus::Entered);
}
