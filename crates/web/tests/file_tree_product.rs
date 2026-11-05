//! 製品の左エリアのファイルツリー（TASK-799・D-100。#2054）。
//!
//! (b) の部品で行を描くこと、Dir と File の区別、深さ優先の並び、親の `role=tree` の条件、畳みが働く行数で木の意味を親と項目の両方から外すことを、
//! 製品の画面（`EngineHost`）で確かめる。試験では `unwrap` を使ってよい。

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

/// ファイルツリーの親の一覧の開始タグと本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"file_tree\"").unwrap();
    let open = html[..at].rfind("<ul").unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    (html[open..=end].to_owned(), html[end + 1..close].to_owned())
}

fn count(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

#[test]
fn 製品の行は_b_の部品と木の属性で出て親に_role_tree_が付く() {
    let ir = pack_ir();
    let node = ir.starts[0].node.clone();
    let mut h =
        EngineHost::with_session(Session::entered_at(ir, &node), standard_registry()).unwrap();
    h.update(HostAction::SelectView(ViewKind::FileTree));
    let (head, body) = list_of(&h);
    assert!(
        !head.contains("role="),
        "0 件の親に role は付けない: {head}"
    );
    assert_eq!(count(&body, "<li"), 0);
    approve(&mut h);
    h.update(HostAction::SelectView(ViewKind::FileTree));
    let (head, body) = list_of(&h);
    assert!(head.contains("role=\"tree\""), "{head}");
    let n = count(&body, "<li");
    assert!(n > 0);
    for pat in [
        "role=\"treeitem\"",
        "aria-level=",
        "aria-posinset=",
        "aria-setsize=",
        "data-path=",
        "data-kind=\"file\"",
        "data-part=\"item-text\"",
        "class=\"detail\"",
    ] {
        assert_eq!(count(&body, pat), n, "{pat}");
    }
    assert!(body.contains("data-key=\"pnotes/draft\""), "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

/// `(Step, 経路の接頭辞, 件数)` の列をログに積み、ファイルツリーを選んだホスト。
fn folded_host(spec: &[(&str, &str, usize)]) -> EngineHost {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let mut log = Vec::new();
    for (step, prefix, n) in spec {
        for i in 0..*n {
            log.push(serde_json::json!({"changed": {
                "at": {"generation": 1, "step": step},
                "path": format!("{prefix}/{i:05}"),
                "before": null,
                "after": {"fixture": null, "attrs": {"title": "t", "state": "open"}},
                "cause": "input",
            }}));
        }
    }
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(pack_ir(), state), standard_registry())
            .unwrap();
    h.update(HostAction::SelectView(ViewKind::FileTree));
    h
}

#[test]
fn 畳みが働かない行数では木の意味が残り畳むと親と項目の両方から外れる() {
    // 目安ちょうど（1,000）。畳まない。
    let h = folded_host(&[("main/st-1", "a", 500), ("main/st-2", "b", 500)]);
    let (head, body) = list_of(&h);
    assert_eq!(count(&body, "data-fold"), 0);
    assert_eq!(count(&body, "<li"), 1_000);
    assert!(head.contains("role=\"tree\""));
    assert_eq!(count(&body, "role=\"treeitem\""), 1_000);
    assert!(a11y::check_no_undefined(&h.view()).is_ok());

    // 1,001 行。古い Step が要約になる。
    let h = folded_host(&[("main/st-1", "a", 600), ("main/st-2", "b", 401)]);
    let (head, body) = list_of(&h);
    assert!(count(&body, "data-fold=\"step\"") >= 1);
    assert!(count(&body, "<li") <= 1_000);
    assert!(!head.contains("role="), "畳むと親の role を外す: {head}");
    assert_eq!(count(&body, "role=\"treeitem\""), 0);
    assert_eq!(count(&body, "aria-level="), 0);
    assert_eq!(count(&body, "aria-posinset="), 0);
    assert_eq!(count(&body, "aria-setsize="), 0);
    assert_eq!(count(&body, "aria-expanded="), 0);
    // 行の中身は残る。
    assert_eq!(count(&body, "data-path=\"b/"), 401);
    assert_eq!(count(&body, "data-kind=\"file\""), 401);
    assert_eq!(count(&body, "data-part=\"item-text\""), 401);
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

/// 経路をそのままログに積み、ファイルツリーを選んだホスト。
fn paths_host(paths: &[&str]) -> EngineHost {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let log: Vec<serde_json::Value> = paths
        .iter()
        .map(|path| {
            serde_json::json!({"changed": {
                "at": {"generation": 1, "step": "main/st-1"},
                "path": path,
                "before": null,
                "after": {"fixture": null, "attrs": {"title": "t", "state": "open"}},
                "cause": "input",
            }})
        })
        .collect();
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(pack_ir(), state), standard_registry())
            .unwrap();
    h.update(HostAction::SelectView(ViewKind::FileTree));
    h
}

/// 開始タグ（`<li ... data-path="<path>" ...>`）の文字列。
fn li_tag(body: &str, path: &str) -> String {
    let at = body.find(&format!("data-path=\"{path}\"")).unwrap();
    let open = body[..at].rfind("<li").unwrap();
    let end = at + body[at..].find('>').unwrap();
    body[open..=end].to_owned()
}

#[test]
fn 他の行の祖先になる経路だけがディレクトリで_描画は深さ優先順になる() {
    // 文字列の辞書順は a・a-b・a/x・a/x/y。a/x が a-b の後ろに来るので、深さ優先に並べ替える。b は行として無い。
    let h = paths_host(&["a-b", "a/x/y", "a", "a/x", "b/c"]);
    let (head, body) = list_of(&h);
    assert!(head.contains("role=\"tree\""), "{head}");
    let pos = |p: &str| body.find(&format!("data-path=\"{p}\"")).unwrap();
    assert!(pos("a") < pos("a/x"), "{body}");
    assert!(pos("a/x") < pos("a/x/y"), "{body}");
    assert!(pos("a/x/y") < pos("a-b"), "子孫は親の直後に続く: {body}");
    assert_eq!(count(&body, "<li"), 5, "途中の行は合成しない: {body}");
    assert!(!body.contains("data-path=\"b\""));
    for (path, kind, level) in [
        ("a", "dir", "1"),
        ("a/x", "dir", "2"),
        ("a/x/y", "file", "3"),
        ("a-b", "file", "1"),
        ("b/c", "file", "1"),
    ] {
        let tag = li_tag(&body, path);
        assert!(tag.contains(&format!("data-kind=\"{kind}\"")), "{tag}");
        assert!(tag.contains(&format!("aria-level=\"{level}\"")), "{tag}");
        assert_eq!(
            tag.contains("aria-expanded=\"true\""),
            kind == "dir",
            "{tag}"
        );
    }
    // 兄弟は同じ親の中で数える（根は a・a-b・b/c の 3 つ）。
    assert!(li_tag(&body, "a").contains("aria-setsize=\"3\""));
    assert!(li_tag(&body, "a/x").contains("aria-setsize=\"1\""));
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}
