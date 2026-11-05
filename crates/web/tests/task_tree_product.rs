//! 製品の左エリアのタスクツリー（TASK-799・D-100。#2054）。
//!
//! (b) の部品で行を描くこと、親の `role=tree` の条件、畳みが働く行数で木の意味を親と項目の両方から外すことを、
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

/// タスクツリーの親の一覧の開始タグと本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"task_tree\"").unwrap();
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
    h.update(HostAction::SelectView(ViewKind::TaskTree));
    let (head, body) = list_of(&h);
    assert!(
        !head.contains("role="),
        "0 件の親に role は付けない: {head}"
    );
    assert_eq!(count(&body, "<li"), 0);
    approve(&mut h);
    h.update(HostAction::SelectView(ViewKind::TaskTree));
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
        "data-kind=\"task\"",
        "data-testid=\"task-label\"",
        "data-testid=\"task-detail\"",
        "data-testid=\"state-badge\"",
        "data-testid=\"state-glyph\"",
        "data-testid=\"state-text\"",
    ] {
        assert_eq!(count(&body, pat), n, "{pat}");
    }
    assert!(body.contains("data-key=\"pnotes/draft\""), "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

/// `(Step, 経路の接頭辞, 件数)` の列をログに積み、タスクツリーを選んだホスト。
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
    h.update(HostAction::SelectView(ViewKind::TaskTree));
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
    assert_eq!(count(&body, "data-kind=\"task\""), 401);
    assert_eq!(count(&body, "data-testid=\"task-label\""), 401);
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

/// 経路をそのままログに積み、タスクツリーを選んだホスト。
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
    h.update(HostAction::SelectView(ViewKind::TaskTree));
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
fn 祖先が欠けた経路は実在する祖先だけから木の位置を求める() {
    // work・work/deep・work/deep/leaf は行として無い。notes/draft と無関係な行を親子にしない。
    let h = paths_host(&["notes/draft", "work/deep/leaf/item"]);
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "<li"), 2, "{body}");
    for path in ["notes/draft", "work/deep/leaf/item"] {
        let tag = li_tag(&body, path);
        assert!(tag.contains("aria-level=\"1\""), "{tag}");
        assert!(tag.contains("aria-setsize=\"2\""), "{tag}");
        assert!(!tag.contains("aria-expanded"), "{tag}");
    }

    // 実在する祖先がある場合は、間の欠けた祖先を飛ばしてその子になる。
    let h = paths_host(&["work", "work/deep/leaf/item", "work/x"]);
    let (_, body) = list_of(&h);
    let top = li_tag(&body, "work");
    assert!(top.contains("aria-expanded=\"true\""), "{top}");
    assert!(top.contains("aria-level=\"1\""), "{top}");
    let deep = li_tag(&body, "work/deep/leaf/item");
    assert!(deep.contains("aria-level=\"2\""), "{deep}");
    assert!(deep.contains("aria-setsize=\"2\""), "{deep}");
    assert!(!deep.contains("aria-expanded"), "{deep}");
}

#[test]
fn 区切り文字と接頭辞が競合しても行は親の直後に子孫が続く深さ優先順で出る() {
    // 文字列の辞書順では a・a-b・a/x となり、a/x が a-b の子に読まれてしまう。
    let h = paths_host(&["a-b", "a/x", "a"]);
    let (_, body) = list_of(&h);
    let pos = |p: &str| body.find(&format!("data-path=\"{p}\"")).unwrap();
    assert!(pos("a") < pos("a/x"), "{body}");
    assert!(pos("a/x") < pos("a-b"), "子孫は親の直後に続く: {body}");
    let parent = li_tag(&body, "a");
    assert!(parent.contains("aria-expanded=\"true\""), "{parent}");
    assert!(parent.contains("aria-level=\"1\""), "{parent}");
    let child = li_tag(&body, "a/x");
    assert!(child.contains("aria-level=\"2\""), "{child}");
    let other = li_tag(&body, "a-b");
    assert!(other.contains("aria-level=\"1\""), "{other}");
    assert!(!other.contains("aria-expanded"), "{other}");
}
