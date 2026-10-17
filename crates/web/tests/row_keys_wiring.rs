//! 左の行の Enter・Space の結線の静的な検査（TASK-358 1/2。A11Y-18・FEND-13・D-42）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、ソースを文字列として読んで照合する（`common::glue_code`）。
//! 振る舞い（click と同じ入力になること）は `tools/browser/point-keys.test.mjs` が確かめ、ここは結線の欠落と、
//! 借用を持ったままの click（`with_app` の呼び出し）の混入を native の試験で検出する。

mod common;

use common::{fn_body, glue_code};
use std::path::PathBuf;

/// 照合の結果を 1 件ずつ文にして返す。`glue`・`hooks` は（正規化前の）`glue.rs`・`test_hooks.rs` の本文。
fn violations(glue: &str, hooks: &str) -> Vec<String> {
    let mut out = Vec::new();
    let glue = glue_code(glue);
    let hooks = glue_code(hooks);
    match fn_body(&glue, "wire_row_keys") {
        None => out.push("glue.rs: wire_row_keys が無い".to_owned()),
        Some(body) => {
            for need in [
                "row_key_effect(",
                "HtmlLiElement",
                ".click()",
                "prevent_default",
            ] {
                if !body.contains(need) {
                    out.push(format!("wire_row_keys: {need} が無い"));
                }
            }
            if body.contains("with_app(") {
                out.push(
                    "wire_row_keys: with_app を呼ぶ（借用を持ったまま click しない）".to_owned(),
                );
            }
        }
    }
    if !fn_body(&glue, "mount_host").is_some_and(|b| b.contains("wire_row_keys(")) {
        out.push("mount_host: wire_row_keys を呼ばない".to_owned());
    }
    if !fn_body(&hooks, "debug_view_fixture_mount").is_some_and(|b| b.contains("wire_row_keys(")) {
        out.push("debug_view_fixture_mount: wire_row_keys を呼ばない".to_owned());
    }
    out
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel))
        .expect("ソースを読めない")
}

const GOOD_GLUE: &str = "fn wire_row_keys(r: &E) { let t: HtmlLiElement = x; \
    let e = row_key_effect(&i, f, true); if e.prevent_default { ev.prevent_default(); } \
    if e.click_row { t.click(); } } \
    fn mount_host() { wire_row_keys(root)?; }";
const GOOD_HOOKS: &str = "fn debug_view_fixture_mount() { wire_row_keys(rt.root()); }";

#[test]
fn 実ソースは違反なし() {
    let v = violations(&read("src/glue.rs"), &read("src/test_hooks.rs"));
    assert!(v.is_empty(), "違反: {v:#?}");
}

#[test]
fn 良い試料は違反にしない() {
    assert!(violations(GOOD_GLUE, GOOD_HOOKS).is_empty());
}

#[test]
fn 結線の欠落を検出する() {
    let hit = |v: &[String], n: &str| v.iter().any(|m| m.contains(n));
    let cases = [
        (
            GOOD_GLUE.replace(".click()", ""),
            GOOD_HOOKS.to_owned(),
            ".click()",
        ),
        (
            GOOD_GLUE.replace("row_key_effect(", "other("),
            GOOD_HOOKS.to_owned(),
            "row_key_effect(",
        ),
        (
            GOOD_GLUE.replace("HtmlLiElement", "Foo"),
            GOOD_HOOKS.to_owned(),
            "HtmlLiElement",
        ),
        (
            GOOD_GLUE.replace("let t:", "with_app(|a| a); let t:"),
            GOOD_HOOKS.to_owned(),
            "with_app",
        ),
        (
            GOOD_GLUE.replace(
                "fn mount_host() { wire_row_keys(root)?; }",
                "fn mount_host() {}",
            ),
            GOOD_HOOKS.to_owned(),
            "mount_host",
        ),
        (
            GOOD_GLUE.to_owned(),
            GOOD_HOOKS.replace("wire_row_keys(", "other("),
            "debug_view_fixture_mount",
        ),
    ];
    for (glue, hooks, needle) in cases {
        let v = violations(&glue, &hooks);
        assert!(hit(&v, needle), "{needle} の欠落を検出できない: {v:?}");
    }
}
