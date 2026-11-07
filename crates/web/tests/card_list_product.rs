//! 製品の左エリアのカードの一覧（TASK-799・D-100。#2055）。
//!
//! (b) の部品で行を描くこと、親に `role` が付かないこと、キーが `p<経路>`（長い経路は `i<位置>`）であること、
//! 状態が語彙の内なら固定の文字、語彙の外の値なら教材の語のままで出ることを、製品の画面（`EngineHost`）で確かめる。期待値は手で数えた値で、
//! 実装の関数から導かない。試験では `unwrap` を使ってよい。

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

/// カードの一覧の親の開始タグと本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"card_list\"").unwrap();
    let open = html[..at].rfind("<ul").unwrap();
    let end = at + html[at..].find('>').unwrap();
    // 行の中の `card-tags` も `ul` なので、次の兄弟の一覧の直前までを本体とする。
    let close = end
        + html[end..]
            .find("</ul><ul data-testid=\"view-list\"")
            .unwrap();
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
    h.update(HostAction::SelectView(ViewKind::CardList));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    assert_eq!(count(&body, "<li"), 0);
    approve(&mut h);
    h.update(HostAction::SelectView(ViewKind::CardList));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    // 行の数は `data-path` の数。`li` は各行の `card-tags` の中にも 1 個ずつある。
    let n = count(&body, "data-path=");
    assert!(n > 0);
    for pat in [
        "data-kind=\"card\"",
        "<article>",
        "data-testid=\"card-label\"",
        "data-testid=\"card-detail\"",
        "data-testid=\"card-tags\"",
        "data-part=\"root\"",
    ] {
        assert_eq!(count(&body, pat), n, "{pat}");
    }
    assert!(body.contains("data-key=\"pnotes/draft\""), "{body}");
    // 状態 `open` は固定の文字、語彙の外の空でない値（`todo`）は教材の語のまま（D-101）、状態の無い行だけが固定の「不明」。
    assert!(body.contains(">オープン</span>"), "{body}");
    assert!(body.contains(">todo</span>"), "{body}");
    assert!(body.contains(">不明</span>"), "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

#[test]
fn 長い経路のキーは位置に退き語彙の外の状態は値の文字のまま出る() {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let long = format!("c/{}", "x".repeat(300));
    let mut log = Vec::new();
    for (path, state) in [("c/short", "merged"), (long.as_str(), "<b>x</b>")] {
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
    h.update(HostAction::SelectView(ViewKind::CardList));
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "data-path="), 2, "{body}");
    assert!(body.contains("data-key=\"pc/short\""), "{body}");
    assert!(body.contains("data-key=\"i1\""), "{body}");
    assert!(body.contains(">マージ済み</span>"), "{body}");
    // 語彙の外の値は文字ノードとして出る。エスケープされ、生の要素にも属性にもならない（D-101）。
    assert!(body.contains(">&lt;b&gt;x&lt;/b&gt;</span>"), "{body}");
    assert!(!body.contains("<b>x</b>"), "{body}");
    assert!(!body.contains("\"<b>x</b>\""), "{body}");
    assert!(!body.contains("不明"), "{body}");
}
