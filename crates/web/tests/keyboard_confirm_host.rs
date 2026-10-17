//! 製品の左の成果物に指せる行が付くことと、その行の操作で確認が成立することの試験（TASK-358 2/2。A11Y-18・FEND-13）。
//!
//! ブラウザの試験（`tools/browser/keyboard-confirm.test.mjs`）が Enter・Space で押す行は、`ArtifactPart` が描く行である。
//! この層では、その行が 4 属性を組で持つこと、`data-payload` を `decode_action("ui", ..)` に渡すと core が確認（`point_at`）
//! を達成にすること、宣言 ID が無い場合は表示だけの行になることを画面なしで確かめる。
//! 規則（選ばれたビューの宣言 ID が引ける行はすべて指せる）は仮置きで、確定は TASK-470・TASK-347 の後続と TASK-739 の担当である。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::artifact_view::{PointTarget, POINT_ACTION};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use serde_json::Value;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn host_with(edit: impl FnOnce(&mut Value)) -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    edit(&mut v);
    let ir: Ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let state: SessionState = serde_json::from_value(s["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn drain(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

/// st-1 の受付と進行までを終え、確認待ちにする。
fn accepted() -> EngineHost {
    accepted_with(|_| {})
}

fn accepted_with(edit: impl FnOnce(&mut Value)) -> EngineHost {
    let mut h = host_with(edit);
    h.update(EngineHost::decode_action("draft", "/make-note first-note").unwrap());
    h.update(EngineHost::decode_action("submit", "").unwrap());
    drain(&mut h);
    h
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

/// `notes/draft` の行の `<li ...>` の開始タグの文字列。
fn row_open_tag(h: &EngineHost) -> String {
    let html = render_component_html(h);
    let at = html.find("data-path=\"notes/draft\"").expect("行がある");
    let start = html[..at].rfind("<li").unwrap();
    let end = at + html[at..].find('>').unwrap();
    html[start..=end].to_owned()
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!(" {name}=\"");
    let at = tag.find(&key)? + key.len();
    let len = tag[at..].find('"')?;
    Some(tag[at..at + len].replace("&quot;", "\""))
}

#[test]
fn 確認待ちの行は_4_属性を組で持ち_宣言_id_の宛先になる() {
    let h = accepted();
    assert_eq!(current(&h), "main/st-1");
    let tag = row_open_tag(&h);
    assert_eq!(attr(&tag, "data-action").as_deref(), Some(POINT_ACTION));
    assert_eq!(attr(&tag, "tabindex").as_deref(), Some("0"));
    assert_eq!(attr(&tag, "data-row").as_deref(), Some("notes/draft"));
    let payload = attr(&tag, "data-payload").expect("payload がある");
    let t = PointTarget::decode(POINT_ACTION, &payload).expect("復号できる");
    assert_eq!(t.view(), "v-files");
    assert_eq!(t.row().as_str(), "notes/draft");
}

#[test]
fn 行の_payload_を_ui_として送ると確認が成立して次の_step_へ進む() {
    let mut h = accepted();
    assert_eq!(achieved(&h), 0);
    let payload = attr(&row_open_tag(&h), "data-payload").unwrap();
    h.update(EngineHost::decode_action(POINT_ACTION, &payload).unwrap());
    drain(&mut h);
    assert_eq!(achieved(&h), 1);
    assert_eq!(current(&h), "main/st-2");
}

#[test]
fn 宣言_id_が引けないビューの行は表示だけで_4_属性が付かない() {
    let h = accepted_with(|v| {
        v["nodes"][0]["step"]["views"] = Value::Array(vec![]);
    });
    let tag = row_open_tag(&h);
    for name in ["data-action", "data-payload", "data-row", "tabindex"] {
        assert_eq!(attr(&tag, name), None, "{name} が付いてはいけない");
    }
}
