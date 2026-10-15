//! 受講者の文字列を非解釈で表示する（SAFE-11。TASK-150）の試験。
//!
//! 描画のエスケープ、履歴の関門を通した復元（ブラウザでの保存・復元は未実装のため native で代える）、
//! 生 HTML の出力口を使っていないことの静的検査を持つ。変異 SAFE11-1 の対象。

use std::path::PathBuf;

use fandhe_frontend_core::render;
use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::load_gate::load_json;
use fandhe_workshop_web::learner_text::learner_line;

/// 記法を要素にしてはならない入力の表。
const CASES: &[&str] = &[
    "# 見出し",
    "[リンク](https://example.invalid/SAFE-11-link)",
    "![画像](https://example.invalid/SAFE-11-img.png)",
    "<https://example.invalid/SAFE-11-auto>",
    "```\nfn main() {}\n```",
    "`inline`",
    "**強調** *斜体*",
    "> 引用",
    "- 一覧\n- 二",
    "| a | b |\n|---|---|\n| 1 | 2 |",
    "---",
    "<b>太字</b>",
    "<img src=\"https://example.invalid/SAFE-11-html\" onerror=\"x()\">",
    "&lt;b&gt;",
    "末尾に空白 2 つ  \n改行",
];

/// 子の要素になりうる開始タグ。
const TAGS: &[&str] = &[
    "<a ",
    "<img",
    "<h1",
    "<h2",
    "<pre",
    "<code",
    "<strong",
    "<em",
    "<b>",
    "<i>",
    "<blockquote",
    "<ul",
    "<table",
    "<hr",
    "<script",
    "<iframe",
];

/// 1 件を描いた HTML が、`<li>` の中に本文のエスケープ形だけを持つことを確かめる。
fn assert_literal(s: &str) {
    let html = render(&learner_line(s));
    assert!(
        html.starts_with("<li>") && html.ends_with("</li>"),
        "{html}"
    );
    let inner = &html["<li>".len()..html.len() - "</li>".len()];
    assert!(!inner.contains('<'), "子の要素がある: {html}");
    for tag in TAGS {
        assert!(!inner.contains(tag), "{tag}: {html}");
    }
    let unescaped = inner
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&");
    assert_eq!(unescaped, s, "{html}");
}

#[test]
fn 記法と_html_は文字のまま描かれ_子の要素にならない() {
    for s in CASES {
        assert_literal(s);
    }
}

#[test]
fn 履歴から復元した入力も文字のまま描かれる() {
    let json = r##"[{"text":"# 見出し\n[リンク](https://example.invalid/r)"},{"command":"/note ![x](https://example.invalid/i)"}]"##;
    let inputs: Vec<Input> = load_json(LoadEntry::History, json.as_bytes()).expect("history loads");
    let expected = [
        "# 見出し\n[リンク](https://example.invalid/r)",
        "/note ![x](https://example.invalid/i)",
    ];
    assert_eq!(inputs.len(), expected.len());
    for (input, want) in inputs.iter().zip(expected) {
        let (Input::Text(got) | Input::Command(got)) = input else {
            panic!("想定外の種別: {input:?}");
        };
        assert_eq!(got.as_bytes(), want.as_bytes());
        assert_literal(got);
    }
}

/// コメントを除く（`//` は行末まで、`/* */` は入れ子を数える）。改行は残す。
fn strip_comments(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == '/' && b.get(i + 1) == Some(&'/') {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
        } else if b[i] == '/' && b.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && b.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && b.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            out.push(' ');
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

#[test]
fn web_のソースは_raw_html_を使わない() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut checked = 0;
    for entry in std::fs::read_dir(&dir).expect("src") {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        let code = strip_comments(&std::fs::read_to_string(&path).expect("read"));
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default();
        assert_eq!(
            code.matches("raw_html").count(),
            0,
            "{name} が raw_html を使っている"
        );
        let raw_variant = code.matches("RawHtml").count();
        if name == "a11y.rs" {
            assert!(raw_variant > 0, "a11y.rs の検査が消えている");
        } else {
            assert_eq!(raw_variant, 0, "{name} が RawHtml を扱っている");
        }
        checked += 1;
    }
    assert!(checked >= 5, "走査したファイルが少なすぎる: {checked}");
}
