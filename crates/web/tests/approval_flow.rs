//! 承認の操作のログの文言と、承認 → 選択 → 最終の承認の流れ（TASK-447 2/2。TERM-27）の試験。
//!
//! 本 repo には CH-2 の教材が無いため、ST-2.6 と同じ構造（対象つきの「直す」でしか通れない承認の Step）を
//! `min-3step` の `st-1` に再現し、続く `st-2`（選択）・`st-3`（最終の承認）まで通す。
//! 章の完了（`ChapterCompleted`）は core が未実装のため、最後の Step の達成までを確かめる。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::{json, Value};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn host() -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let step = v["nodes"][0]["step"].as_object_mut().unwrap();
    step.insert("input".into(), json!({ "kind": "approval" }));
    step.insert("confirm".into(), json!({ "kind": "approve" }));
    step.insert(
        "rules".into(),
        json!([
            { "id": "r-t0", "on": { "approval": "revise", "target": "t-alpha" }, "outcome": { "kind": "accept" } },
            { "id": "r-t1", "on": { "approval": "revise", "target": "t-beta" }, "outcome": { "kind": "accept" } },
            { "id": "r-bare", "on": { "approval": "revise" }, "outcome": { "kind": "reject", "rejection": "rj-decline" } },
            { "id": "r-early", "on": { "approval": "approve" }, "outcome": { "kind": "reject", "rejection": "rj-decline" } }
        ]),
    );
    let ir: Ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let state: SessionState = serde_json::from_value(s["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn click_decide(h: &mut EngineHost, v: &str) {
    h.update(EngineHost::decode_action("decide", v).unwrap());
}

fn drain(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn current(h: &EngineHost) -> String {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .current
        .as_ref()
        .unwrap()
        .0
        .clone()
}

fn achieved(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

/// 承認の行（`data-input="approval"`）の `li` 1 件分の html を、現れた順に取り出す。
fn approval_rows(h: &EngineHost) -> Vec<String> {
    html(h)
        .split("<li")
        .skip(1)
        .filter(|s| s.contains("data-input=\"approval\""))
        .map(|s| s.split("</li>").next().unwrap().to_owned())
        .collect()
}

/// `<li` の続き（属性から始まる断片）から、タグを除いた文字だけを連結する。
fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut in_tag = true;
    for c in s.chars() {
        match c {
            '>' => in_tag = false,
            '<' => in_tag = true,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

fn texts(h: &EngineHost) -> Vec<String> {
    approval_rows(h).iter().map(|r| strip_tags(r)).collect()
}

fn rejected_rows(h: &EngineHost) -> usize {
    html(h).matches("data-kind=\"rejected\"").count()
}

fn buttons(h: &EngineHost) -> usize {
    html(h).matches("data-testid=\"approval-btn\"").count()
}

#[test]
fn 承認の操作の行は文言になり_はいで先へ進んで最後の_step_まで通る() {
    let mut h = host();
    assert_eq!(current(&h), "main/st-1");

    click_decide(&mut h, "revise");
    assert_eq!(rejected_rows(&h), 1);
    assert_eq!(texts(&h), vec!["承認の操作: 直す"]);

    h.update(EngineHost::decode_action("revise-target", "t-alpha").unwrap());
    drain(&mut h);
    assert_eq!(
        texts(&h),
        vec!["承認の操作: 直す", "承認の操作: 直す（対象: t-alpha）"]
    );
    assert!(approval_rows(&h)[1].contains("data-testid=\"line-target\""));
    assert_eq!(buttons(&h), 3);

    click_decide(&mut h, "approve");
    assert_eq!(current(&h), "main/st-2");
    assert_eq!(achieved(&h), 1);

    h.update(HostAction::Engine(Op::Input(Input::Choice(
        "opt-memo".to_owned(),
    ))));
    assert_eq!(current(&h), "main/st-3");

    click_decide(&mut h, "approve");
    drain(&mut h);
    click_decide(&mut h, "approve");
    drain(&mut h);
    assert_eq!(achieved(&h), 3, "{}", html(&h));
    assert_eq!(buttons(&h), 0, "{}", html(&h));
    assert_eq!(
        texts(&h),
        vec![
            "承認の操作: 直す",
            "承認の操作: 直す（対象: t-alpha）",
            "承認の操作: はい",
            "承認の操作: はい",
            "承認の操作: はい",
        ]
    );
    assert_eq!(rejected_rows(&h), 1);
}
