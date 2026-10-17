//! Step の状態表示（`step_status`、TASK-435 1/2）の適合の試験。入力が受け付けられたら「受付済み（確認待ち）」になり、
//! 確認の前に「達成」が出ないこと、確認を省く Step（`confirm: none`）では受付と同時に「達成」が出ること、
//! 描いた要素が TASK-431 の契約（`terminal_area`）に適合することを確かめる。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::{check_no_undefined, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::*;
use fandhe_workshop_web::step_status::step_status;
use fandhe_workshop_web::terminal_area::STEP_STATUS_TESTID;

const WORD_FIELD: &str = "step_status_word";
const TOTAL_TICKS: u32 = 5;
const ACCEPTED: &str = "受付済み（確認待ち）";
const ACHIEVED: &str = "達成";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn pack_bytes() -> Vec<u8> {
    std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json")).unwrap()
}

fn host_of(bytes: &[u8]) -> EngineHost {
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(bytes).unwrap(), state);
    EngineHost::with_session(session, standard_registry()).unwrap()
}

fn begun_host() -> EngineHost {
    host_of(&pack_bytes())
}

fn op(o: Op) -> HostAction {
    HostAction::Engine(o)
}

fn make_note() -> HostAction {
    op(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

fn tick(index: u32) -> HostAction {
    op(Op::Tick(TickAddr {
        generation: Generation(1),
        step: StepUid::parse("main/st-1").unwrap(),
        index,
    }))
}

fn point(path: &str) -> HostAction {
    op(Op::Input(Input::Ui {
        view: "v-files".to_owned(),
        path: path.to_owned(),
    }))
}

fn go_to(uid: &str) -> HostAction {
    op(Op::Control(Control::GoTo(StepUid::parse(uid).unwrap())))
}

fn choose() -> HostAction {
    op(Op::Input(Input::Choice("opt-memo".to_owned())))
}

fn approve() -> HostAction {
    op(Op::Input(Input::Approval {
        decision: Decision::Approve,
        target: None,
    }))
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn find<'a>(n: &'a Node, testid: &str) -> Option<&'a Node> {
    if attr(n, TESTID_ATTR) == Some(testid) {
        return Some(n);
    }
    match n {
        Node::Element { children, .. } => children.iter().find_map(|c| find(c, testid)),
        _ => None,
    }
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Element { children, .. } => children
            .iter()
            .map(|c| match c {
                Node::Text(t) => t.clone(),
                _ => String::new(),
            })
            .collect(),
        _ => String::new(),
    }
}

fn text_field(host: &EngineHost) -> String {
    match host.bound_value(WORD_FIELD) {
        Some(BoundValue::Text(s)) => s,
        other => panic!("{WORD_FIELD}: {other:?}"),
    }
}

/// 描画の `step-status` と束縛値が同じ語を指すこと。語は固定の 2 語か空だけである。
fn assert_shows(host: &EngineHost, word: &str) {
    let view = host.view();
    let node = find(&view, STEP_STATUS_TESTID).expect("step-status");
    assert_eq!(text_of(node), word);
    assert_eq!(text_field(host), word);
    assert!(check_no_undefined(&view).is_ok());
}

fn session(host: &EngineHost) -> &Session {
    host.model().session().unwrap()
}

fn achieved_count(host: &EngineHost) -> usize {
    session(host)
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

fn at_step(host: &EngineHost, accepted: bool, uid: &str) -> usize {
    session(host)
        .log()
        .iter()
        .filter(|e| match e {
            Event::Accepted { at, .. } => accepted && at.step.0 == uid,
            Event::Achieved { at, .. } => !accepted && at.step.0 == uid,
            _ => false,
        })
        .count()
}

fn current(host: &EngineHost) -> String {
    session(host).state().run.current.clone().unwrap().0
}

fn to_accepted_in_st1(host: &mut EngineHost) {
    host.update(make_note());
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
    }
}

#[test]
fn 未読込のホストは語なしで_空の文字になる() {
    let host = EngineHost::new();
    assert_eq!(step_status(None), None);
    assert_shows(&host, "");
}

#[test]
fn 開始直後は語なしである() {
    let host = begun_host();
    assert_eq!(step_status(host.model().session()), None);
    assert_shows(&host, "");
}

#[test]
fn 送信の後は受付済みで_進行の各操作の後も達成にならない() {
    let mut host = begun_host();
    host.update(make_note());
    assert_eq!(
        step_status(host.model().session()),
        Some(StepStatus::Accepted)
    );
    assert_shows(&host, ACCEPTED);
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
        assert_shows(&host, ACCEPTED);
        assert_eq!(achieved_count(&host), 0, "tick {i}");
    }
    assert_eq!(current(&host), "main/st-1");
}

#[test]
fn 外れた指し先では受付済みのままで_達成にならない() {
    let mut host = begun_host();
    to_accepted_in_st1(&mut host);
    host.update(point("elsewhere/none"));
    assert_shows(&host, ACCEPTED);
    assert_eq!(achieved_count(&host), 0);
    assert_eq!(current(&host), "main/st-1");
}

#[test]
fn 正しい指し先で達成になり_次の_step_へ移る() {
    let mut host = begun_host();
    to_accepted_in_st1(&mut host);
    host.update(point("notes/draft"));
    assert_eq!(achieved_count(&host), 1);
    assert_eq!(current(&host), "main/st-2");
    assert_eq!(
        step_status(host.model().session()),
        Some(StepStatus::Achieved)
    );
    assert_shows(&host, ACHIEVED);
}

#[test]
fn 確認を省く_step_は受付と同時に達成になる() {
    let mut host = begun_host();
    host.update(go_to("main/st-2"));
    assert_shows(&host, "");
    host.update(choose());
    assert_eq!(at_step(&host, true, "main/st-2"), 1);
    assert_eq!(at_step(&host, false, "main/st-2"), 1);
    assert_eq!(current(&host), "main/st-3");
    assert_shows(&host, ACHIEVED);
    // 次の操作で「達成」は消える（st-3 への拒否される入力）。
    host.update(choose());
    assert_shows(&host, "");
}

#[test]
fn 確認が明示されていれば_選択の後は受付済みで達成にならない() {
    let mut pack: serde_json::Value = serde_json::from_slice(&pack_bytes()).unwrap();
    let mut replaced = 0;
    for n in pack["nodes"].as_array_mut().unwrap() {
        if n["uid"] == "main/st-2" {
            n["step"]["confirm"] = serde_json::json!({"kind": "approve"});
            replaced += 1;
        }
    }
    assert_eq!(replaced, 1);
    let mut host = host_of(&serde_json::to_vec(&pack).unwrap());
    host.update(go_to("main/st-2"));
    host.update(choose());
    assert_shows(&host, ACCEPTED);
    assert_eq!(achieved_count(&host), 0);
    assert_eq!(current(&host), "main/st-2");
}

#[test]
fn 承認の確認で達成になる() {
    let mut host = begun_host();
    host.update(go_to("main/st-3"));
    host.update(approve());
    assert_shows(&host, ACCEPTED);
    assert_eq!(achieved_count(&host), 0);
    host.update(approve());
    assert_eq!(achieved_count(&host), 1);
    assert_shows(&host, ACHIEVED);
}

#[test]
fn 移動と停止の直後は達成を出さない() {
    let mut host = begun_host();
    host.update(go_to("main/st-2"));
    host.update(choose());
    assert_shows(&host, ACHIEVED);
    host.update(go_to("main/st-1"));
    assert_shows(&host, "");
    host.update(make_note());
    host.update(op(Op::Control(Control::Stop)));
    // 停止は core の Step の状態を戻さない。表示は状態を写すだけなので「達成」にはならない。
    assert_eq!(achieved_count(&host), 1);
    assert_ne!(text_field(&host), ACHIEVED);
}

#[test]
fn 束縛点は語が変わった操作でだけ_dirty_になる() {
    let mut host = begun_host();
    host.update(make_note());
    assert!(host.dirty_fields().contains(&WORD_FIELD));
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
        assert!(!host.dirty_fields().contains(&WORD_FIELD), "tick {i}");
    }
}
