//! 本文リンクの許可スキーム（SAFE-9。TASK-149 1/2）の適合試験。
//!
//! 変換器を通さずに手作りした木 16 件（T-01〜T-16。試験データは spec の写し）を、読み込みの関門に通してから
//! 描画し、許可外のスキームがリンクにならず文字として出ることと、危険な要素が出ないことを確かめる。
//! あわせて、変換器（pack の `markdown::parse`）を通した経路でも同じ結果になることを確かめる。
//! 試験では `unwrap`・`expect` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::render;
use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_core::load_gate::load_json;
use fandhe_workshop_pack::markdown;
use fandhe_workshop_web::doc_view::render_doc_node;
use serde_json::Value;
use std::path::PathBuf;

fn testdata(rel: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/untrusted-input-safety")
        .join(rel);
    let text = std::fs::read_to_string(&path).expect("read testdata");
    serde_json::from_str(&text).expect("parse testdata")
}

/// 試験データの `gen.kind == "quote_nest"` と同じ形（PoC の run.mjs と同じ）を文字列で作る。
fn quote_nest(n: u64) -> String {
    let open = "{\"t\":\"quote\",\"children\":[".repeat(n as usize);
    let close = "]}".repeat(n as usize);
    format!("{{\"version\":1,\"blocks\":[{open}{close}]}}")
}

/// 16 件の (ID, 木の JSON 文字列)。
fn cases() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for rel in [
        "test-data-addendum/tree-cases.json",
        "test-data-addendum-2/tree-cases-2.json",
    ] {
        let v = testdata(rel);
        for c in v["cases"].as_array().expect("cases") {
            let id = c["id"].as_str().expect("id").to_owned();
            let json = if let Some(tree) = c.get("tree") {
                tree.to_string()
            } else {
                assert_eq!(c["gen"]["kind"], "quote_nest", "{id}");
                quote_nest(c["gen"]["n"].as_u64().expect("n"))
            };
            out.push((id, json));
        }
    }
    out
}

fn load(json: &str) -> Result<Doc, ()> {
    load_json::<Doc>(LoadEntry::Ir, json.as_bytes()).map_err(|_| ())
}

fn html_of(doc: &Doc) -> String {
    render(&render_doc_node(doc))
}

#[test]
fn 試験データは16件ある() {
    let ids: Vec<String> = cases().into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids.len(), 16, "{ids:?}");
    assert_eq!(ids.first().map(String::as_str), Some("T-01"));
    assert_eq!(ids.last().map(String::as_str), Some("T-16"));
}

#[test]
fn 全件で危険な要素と属性が出ない() {
    for (id, json) in cases() {
        let Ok(doc) = load(&json) else { continue };
        let html = html_of(&doc);
        for needle in ["<img", "<script", "<iframe"] {
            assert!(!html.contains(needle), "{id}: {needle} in {html}");
        }
        // タグの中（`<` から `>` まで）に、イベント属性と class が無い。本文の文字は `&lt;` に逃がされている。
        for tag in html.split('<').skip(1) {
            let tag = tag.split('>').next().unwrap_or("");
            assert!(
                !tag.contains(" on") && !tag.contains("class="),
                "{id}: {html}"
            );
        }
        // 出る `a` は、許可された href を持つものだけ。
        for part in html.split("<a ").skip(1) {
            assert!(
                part.starts_with("href=\"") && !part.starts_with("href=\"javascript"),
                "{id}: {html}"
            );
        }
    }
}

#[test]
fn 許可外のリンクは文字として出る() {
    // T-01・T-02・T-13・T-14: 子の文字 x が残り、a は 0 件。
    for want in ["T-01", "T-02", "T-13", "T-14"] {
        let (id, json) = cases().into_iter().find(|(id, _)| id == want).unwrap();
        let doc = load(&json).unwrap_or_else(|()| panic!("{id} must pass the gate"));
        let html = html_of(&doc);
        assert!(!html.contains("<a"), "{id}: {html}");
        assert!(html.contains("<p><span>x</span></p>"), "{id}: {html}");
    }
}

#[test]
fn 語彙の外の木は型の段で拒否される() {
    for want in ["T-05", "T-06", "T-07"] {
        let (id, json) = cases().into_iter().find(|(id, _)| id == want).unwrap();
        assert!(load(&json).is_err(), "{id}");
    }
}

#[test]
fn テキストのタグは文字として出る() {
    let (_, json) = cases().into_iter().find(|(id, _)| id == "T-08").unwrap();
    let html = html_of(&load(&json).unwrap());
    assert!(html.contains("&lt;img"), "{html}");
}

#[test]
fn 画像は_img_を作らず代替の文字だけを出す() {
    for want in ["T-03", "T-04"] {
        let (id, json) = cases().into_iter().find(|(id, _)| id == want).unwrap();
        let html = html_of(&load(&json).unwrap());
        assert!(
            !html.contains("<img") && !html.contains("src="),
            "{id}: {html}"
        );
        assert!(html.contains(">x</span>"), "{id}: {html}");
    }
}

#[test]
fn 許可されたリンクはちょうど_1_件の_a_になる() {
    for href in ["https://example.com/a", "/guide/a.md#x", "./b"] {
        let json = format!(
            "{{\"version\":1,\"blocks\":[{{\"t\":\"paragraph\",\"children\":[{{\"t\":\"link\",\"href\":{},\"children\":[{{\"t\":\"text\",\"text\":\"x\"}}]}}]}}]}}",
            Value::String(href.to_owned())
        );
        let html = html_of(&load(&json).unwrap());
        assert_eq!(html.matches("<a ").count(), 1, "{html}");
        assert!(
            html.contains(&format!(
                "<a href=\"{href}\" rel=\"noopener noreferrer\">x</a>"
            )),
            "{html}"
        );
        assert!(!html.contains("target="), "{html}");
    }
}

#[test]
fn 変換器を通した経路でも許可外のリンクは_a_にならない() {
    // 実体参照は pulldown-cmark が解いた後の href を判定する。この試験が、その経路の唯一の観測点である。
    // XSS-13・14・15・17・18・26b・41・41b・45（xss-cases.md。`\u{..}` の表記は同じ文字をここに直接書く）。
    let cases = [
        "[クリック](javascript:__poc7_hit('XSS-13'))",
        "[クリック](JaVaScRiPt:__poc7_hit('XSS-14'))",
        "[クリック](data:text/html,<script>parent.__poc7_hit('XSS-15')</script>)",
        "[クリック](java\tscript:__poc7_hit('XSS-17'))",
        "[クリック](\u{1}javascript:__poc7_hit('XSS-18'))",
        "[クリック](<vbscript:x>)",
        "[b](//example.invalid/XSS-26b)",
        "[クリック](\\\\example.invalid\\x)",
        "[クリック](&#106;avascript:__poc7_hit('XSS-41'))",
        "[クリック](&#x6A;avascript&colon;__poc7_hit('XSS-41b'))",
        "[クリック](java\u{200B}script:__poc7_hit('XSS-45'))",
    ];
    for src in cases {
        let doc = markdown::parse(src);
        let html = html_of(&doc);
        assert!(!html.contains("<a"), "{src:?}: {html}");
    }
    // 対照: 許可された形は a になる（変換器か描画が壊れて常に 0 件になっていないことの確認）。
    let html = html_of(&markdown::parse(
        "[ok](https://example.com/a) [rel](./b.md)",
    ));
    assert_eq!(html.matches("<a ").count(), 2, "{html}");
}

#[test]
fn 描画は_a11y_の未定義の項目を持たない() {
    for (id, json) in cases() {
        if let Ok(doc) = load(&json) {
            assert_eq!(
                fandhe_workshop_web::a11y::check_no_undefined(&render_doc_node(&doc)),
                Ok(()),
                "{id}"
            );
        }
    }
}
