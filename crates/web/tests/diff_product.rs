//! 製品の左エリアの差分のビュー（TASK-799・D-100。#2057）。
//!
//! (b) の部品で行を描くこと、親に `role` が付かないこと、深さのクラスが付かないこと、キーが `p<経路>`
//! （長い経路は `i<位置>`）であること、行の種類・記号・行番号が閉じた規則で読まれ、語彙の外の値が既知の値へ寄らないことを、
//! 製品の画面（`EngineHost`）で確かめる。期待値は手で数えた値で、実装の関数から導かない。試験では `unwrap` を使ってよい。

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

/// 差分の一覧の親の開始タグと本体。この種類の行の中には `ul` が無いので、次の `</ul>` までが本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"diff\"").unwrap();
    let open = html[..at].rfind("<ul").unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    (html[open..=end].to_owned(), html[end + 1..close].to_owned())
}

fn count(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

/// 世界状態に要素を足す `changed` を差し込んだ製品の画面で、差分のビューを選ぶ。
fn host_with(entries: &[(&str, serde_json::Value)]) -> EngineHost {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let log: Vec<serde_json::Value> = entries
        .iter()
        .map(|(path, attrs)| {
            serde_json::json!({"changed": {
                "at": {"generation": 1, "step": "main/st-1"},
                "path": path,
                "before": null,
                "after": {"fixture": null, "attrs": attrs},
                "cause": "input",
            }})
        })
        .collect();
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(pack_ir(), state), standard_registry())
            .unwrap();
    h.update(HostAction::SelectView(ViewKind::Diff));
    h
}

#[test]
fn 製品の行は_b_の部品で出て親に_role_が付かない() {
    let ir = pack_ir();
    let node = ir.starts[0].node.clone();
    let mut h =
        EngineHost::with_session(Session::entered_at(ir, &node), standard_registry()).unwrap();
    h.update(HostAction::SelectView(ViewKind::Diff));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    assert_eq!(count(&body, "data-path="), 0);
    approve(&mut h);
    h.update(HostAction::SelectView(ViewKind::Diff));
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    let n = count(&body, "data-path=");
    assert!(n > 0);
    for pat in [
        "data-kind=\"line\"",
        "data-testid=\"diff-old-no\"",
        "data-testid=\"diff-new-no\"",
        "data-testid=\"diff-sign\"",
        "<pre ",
    ] {
        assert_eq!(count(&body, pat), n, "{pat}");
    }
    assert!(body.contains("data-key=\"pnotes/draft\""), "{body}");
    // 深さのクラスは付けない（`pre` がインデントを持つ）。
    for d in 0..10 {
        assert!(!body.contains(&format!("class=\"d{d}\"")), "{body}");
    }
    // 状態 `open` などは語彙の外で、`+`・`-` に寄らず `?` になる。
    assert_eq!(count(&body, "class=\"df-sign\">?</span>"), n, "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

#[test]
fn 行の種類と記号と行番号は閉じた規則で読まれる() {
    let h = host_with(&[
        (
            "d/a",
            serde_json::json!({"title": "@@ -1 +1 @@", "state": "hunk"}),
        ),
        (
            "d/b",
            serde_json::json!({"title": "added", "state": "add", "detail": "12:34"}),
        ),
        (
            "d/c",
            serde_json::json!({"title": "removed", "state": "del", "detail": "ab:7"}),
        ),
        (
            "d/d",
            serde_json::json!({"title": "same", "state": "ctx", "detail": "5"}),
        ),
        ("d/e", serde_json::json!({"state": "<b>x</b>"})),
    ]);
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "data-path="), 5, "{body}");
    // hunk だけが `hunk`、ほかは `line`。
    assert_eq!(count(&body, "data-kind=\"hunk\""), 1, "{body}");
    assert_eq!(count(&body, "data-kind=\"line\""), 4, "{body}");
    // 追加: 記号 `+`・旧 12・新 34。
    assert!(body.contains("df-old\">12</span>"), "{body}");
    assert!(body.contains("df-new\">34</span>"), "{body}");
    assert!(body.contains("df-sign\">+</span>"), "{body}");
    // 削除: 記号 `-`・旧は崩れた値なので空・新 7。
    assert!(body.contains("df-sign\">-</span>"), "{body}");
    assert!(body.contains("df-new\">7</span>"), "{body}");
    assert!(!body.contains(">ab<"), "{body}");
    // `:` が無い補足は両方空。文脈と hunk の記号は空。
    assert!(!body.contains(">5<"), "{body}");
    // `title` が `pre` の文字になり、無いときは経路になる。
    assert!(body.contains(">added</pre>"), "{body}");
    assert!(body.contains(">d/e</pre>"), "{body}");
    // 語彙の外の値は `?` で、生の値は画面に出ない。
    assert!(body.contains("df-sign\">?</span>"), "{body}");
    assert!(!body.contains("<b>x</b>"), "状態の値は画面に出ない");
    assert!(!body.contains("&lt;b&gt;"), "状態の値は画面に出ない");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

#[test]
fn 長い経路のキーは位置に退く() {
    let long = format!("c/{}", "x".repeat(300));
    let h = host_with(&[
        ("c/short", serde_json::json!({"state": "add"})),
        (long.as_str(), serde_json::json!({"state": "add"})),
    ]);
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "data-path="), 2, "{body}");
    assert!(body.contains("data-key=\"pc/short\""), "{body}");
    assert!(body.contains("data-key=\"i1\""), "{body}");
}
