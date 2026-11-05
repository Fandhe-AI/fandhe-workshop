//! 製品の左エリアのチェックの一覧（TASK-799・D-100。#2057）。
//!
//! (b) の部品で行を描くこと、親に `role` が付かないこと、キーが `p<経路>`（長い経路は `i<位置>`）であること、
//! 状態が閉じた語彙の固定の文字で出て、語彙の外の値が既知の値へ寄らないことを、製品の画面（`EngineHost`）で確かめる。
//! 期待値は手で数えた値で、実装の関数から導かない。試験では `unwrap` を使ってよい。

use std::path::{Path, PathBuf};

use fandhe_frontend_interactive::Component;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::artifact_view::ViewKind;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};

fn pack_ir() -> Ir {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/product_view_pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {} 件の診断", d.len()));
    load_ir(&bytes).unwrap()
}

fn approve(h: &mut EngineHost) {
    h.update(EngineHost::decode_action("decide", "approve").unwrap());
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

/// チェックの一覧の親の開始タグと本体。この種類の行の中には `ul` が無いので、次の `</ul>` までが本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"check_list\"").unwrap();
    let open = html[..at].rfind("<ul").unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    (html[open..=end].to_owned(), html[end + 1..close].to_owned())
}

fn count(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

#[test]
fn 製品の行は_b_の部品で出て親に_role_が付かない() {
    let ir = pack_ir();
    let node = ir.starts[0].node.clone();
    let mut h =
        EngineHost::with_session(Session::entered_at(ir, &node), standard_registry()).unwrap();
    h.update(HostAction::SelectView(ViewKind::CheckList));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    assert_eq!(count(&body, "data-path="), 0);
    approve(&mut h);
    h.update(HostAction::SelectView(ViewKind::CheckList));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    let n = count(&body, "data-path=");
    assert!(n > 0);
    for pat in [
        "data-kind=\"check\"",
        "data-testid=\"check-label\"",
        "data-testid=\"check-detail\"",
        "data-part=\"root\"",
    ] {
        assert_eq!(count(&body, pat), n, "{pat}");
    }
    assert!(body.contains("data-key=\"pnotes/draft\""), "{body}");
    // 状態 `open` などは語彙の外で、既知の状態へ寄らず「? 不明」になる。
    assert_eq!(count(&body, ">? 不明</span>"), n, "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

#[test]
fn 状態は固定の文字で出て語彙の外は不明になり長い経路のキーは位置に退く() {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let long = format!("c/{}", "x".repeat(300));
    let mut log = Vec::new();
    for (path, state) in [
        ("c/ok", "success"),
        ("c/ng", "failure"),
        ("c/case", "Success"),
        (long.as_str(), "<b>x</b>"),
    ] {
        log.push(serde_json::json!({"changed": {
            "at": {"generation": 1, "step": "main/st-1"},
            "path": path,
            "before": null,
            "after": {"fixture": null, "attrs": {"title": "t", "state": state}},
            "cause": "input",
        }}));
    }
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(pack_ir(), state), standard_registry())
            .unwrap();
    h.update(HostAction::SelectView(ViewKind::CheckList));
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "data-path="), 4, "{body}");
    assert!(body.contains("data-key=\"pc/ok\""), "{body}");
    assert!(body.contains("data-key=\"i3\""), "{body}");
    assert!(body.contains(">✓ 成功</span>"), "{body}");
    assert!(body.contains(">✗ 失敗</span>"), "{body}");
    // 大文字の違いと HTML の形は語彙の外で「? 不明」になる。
    assert_eq!(count(&body, ">? 不明</span>"), 2, "{body}");
    assert!(!body.contains("<b>x</b>"), "状態の値は画面に出ない");
    assert!(!body.contains("&lt;b&gt;"), "状態の値は画面に出ない");
}
