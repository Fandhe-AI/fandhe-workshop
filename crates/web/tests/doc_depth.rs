//! 描画の深さの上限と、コードの言語名の `class`（SAFE-12。TASK-150 2/3）の適合試験。
//!
//! 引用・リスト・強調の入れ子が 32 段で省略の文言になること、木を単独で読む経路（`serde_json` の再帰の上限 128）が
//! 深い木をスタックを使い切らずに拒否すること、T-09 の言語名が属性を注入できないことを確かめる。
//! 製品の読み込みの経路では、JSON の深さ 64 の関門が 50・60 段の引用を先に拒否する（`core` の `doc_load` 試験）。
//! 3/3 では、IR の経路で読める最も深い木が描画の上限（32 段）に届かず省略にならないこと、compile した教材パックの
//! 木が省略なしで描けることを確かめる。
//! 試験では `unwrap`・`expect` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::render;
use fandhe_workshop_contract::doc::{Block, Doc, Inline, MAX_DOC_DEPTH, TREE_VERSION};
use fandhe_workshop_contract::limits::{LoadEntry, MAX_JSON_DEPTH};
use fandhe_workshop_core::json_depth::json_depth;
use fandhe_workshop_core::load_gate::load_json;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_web::doc_view::{
    lang_class, render_doc_node, DEPTH_OMITTED_TEXT, MAX_RENDER_DEPTH, VERSION_REJECTED_TEXT,
};
use serde_json::Value;
use std::path::PathBuf;

fn html_of(doc: &Doc) -> String {
    render(&render_doc_node(doc))
}

fn doc(blocks: Vec<Block>) -> Doc {
    Doc {
        version: TREE_VERSION,
        blocks,
    }
}

/// 引用を `n` 段入れ子にした木。
fn quote_nest_doc(n: usize) -> Doc {
    let mut blocks = Vec::new();
    for _ in 0..n {
        blocks = vec![Block::Quote { children: blocks }];
    }
    doc(blocks)
}

fn quote_nest_json(n: usize) -> String {
    let open = "{\"t\":\"quote\",\"children\":[".repeat(n);
    let close = "]}".repeat(n);
    format!("{{\"version\":1,\"blocks\":[{open}{close}]}}")
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

fn testdata_case(id: &str) -> Value {
    for rel in [
        "test-data-addendum/tree-cases.json",
        "test-data-addendum-2/tree-cases-2.json",
    ] {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../testdata/untrusted-input-safety")
            .join(rel);
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
        for c in v["cases"].as_array().expect("cases") {
            if c["id"] == id {
                return c.clone();
            }
        }
    }
    panic!("case {id} not found");
}

fn quote_nest_n(id: &str) -> usize {
    testdata_case(id)["gen"]["n"].as_u64().expect("n") as usize
}

#[test]
fn 変換器の深さの上限は描画の上限より小さい() {
    // 変換器の出力は省略にならない。
    const { assert!(MAX_DOC_DEPTH < MAX_RENDER_DEPTH) };
    assert_eq!(MAX_RENDER_DEPTH, 32);
}

#[test]
fn 引用の入れ子は_32_段で省略になる() {
    for (n, want_quotes, want_omitted) in [
        (1, 1, 0),
        (31, 31, 0),
        (32, 32, 1),
        (33, 32, 1),
        (50, 32, 1),
        (60, 32, 1),
        (200, 32, 1),
    ] {
        let html = html_of(&quote_nest_doc(n));
        assert_eq!(count(&html, "<blockquote>"), want_quotes, "n={n}");
        assert_eq!(count(&html, DEPTH_OMITTED_TEXT), want_omitted, "n={n}");
    }
}

#[test]
fn 省略の文言は_p_に入る() {
    let html = html_of(&quote_nest_doc(40));
    assert!(
        html.contains("<p>（入れ子が深すぎるため省略）</p>"),
        "{html}"
    );
}

#[test]
fn リストの入れ子も_32_段で省略になる() {
    let mut blocks = Vec::new();
    for _ in 0..40 {
        blocks = vec![Block::List {
            ordered: false,
            items: vec![blocks],
        }];
    }
    let html = html_of(&doc(blocks));
    assert_eq!(count(&html, "<ul>"), 32);
    assert_eq!(count(&html, DEPTH_OMITTED_TEXT), 1);
}

#[test]
fn 強調の入れ子は上限でインラインの文字が省略になる() {
    let mut inlines = vec![Inline::Text {
        text: "deep".into(),
    }];
    for _ in 0..40 {
        inlines = vec![Inline::Emph { children: inlines }];
    }
    let html = html_of(&doc(vec![Block::Paragraph { children: inlines }]));
    assert_eq!(count(&html, "<em>"), 32);
    assert_eq!(count(&html, DEPTH_OMITTED_TEXT), 1);
    assert!(!html.contains("deep"), "{html}");
}

#[test]
fn 浅い木は省略にならない() {
    let html = html_of(&doc(vec![Block::Paragraph {
        children: vec![Inline::Strong {
            children: vec![Inline::Text { text: "ok".into() }],
        }],
    }]));
    assert_eq!(
        html,
        "<div data-testid=\"doc\"><p><strong>ok</strong></p></div>"
    );
}

#[test]
fn 木を単独で読む経路は_128_段で拒否しスタックを使い切らない() {
    // JSON の入れ子は 2 + 2n 段。serde_json の再帰の上限は 128 で、127 段までが通る。
    // 版上げや feature の変化で上限が変われば、ここで検出する。
    assert!(serde_json::from_str::<Doc>(&quote_nest_json(62)).is_ok()); // 126 段
    assert!(serde_json::from_str::<Doc>(&quote_nest_json(63)).is_err()); // 128 段
}

#[test]
fn 試験データの深い入れ子は単独の経路で期待どおりになる() {
    // T-11（10,000 段）・T-12（100 段 = JSON 202 段）は 128 の上限で拒否される。
    for id in ["T-11", "T-12"] {
        let json = quote_nest_json(quote_nest_n(id));
        assert!(serde_json::from_str::<Doc>(&json).is_err(), "{id}");
    }
    // T-15（50 段 = 102 段）・T-16（60 段 = 122 段）は読め、描画で 32 段の省略になる。
    for id in ["T-15", "T-16"] {
        let json = quote_nest_json(quote_nest_n(id));
        let doc: Doc = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{id}: {e}"));
        let html = html_of(&doc);
        assert_eq!(count(&html, "<blockquote>"), 32, "{id}");
        assert_eq!(count(&html, DEPTH_OMITTED_TEXT), 1, "{id}");
    }
}

#[test]
fn 版が違う木は_rust_から直接渡されても描画しない() {
    let bad = Doc {
        version: TREE_VERSION + 1,
        blocks: vec![Block::Paragraph {
            children: vec![Inline::Text {
                text: "secret".into(),
            }],
        }],
    };
    let html = html_of(&bad);
    assert!(!html.contains("secret"), "{html}");
    assert!(html.contains("未対応の版"), "{html}");
}

#[test]
fn 言語名は安全な文字だけのとき_class_になる() {
    for (lang, want) in [
        ("rust", Some("language-rust")),
        ("c++", Some("language-c++")),
        ("objective-c", Some("language-objective-c")),
        ("x86_64.asm", Some("language-x86_64.asm")),
        ("", None),
        ("rust\" onmouseover=\"x", None),
        ("a b", None),
        ("a<b", None),
        ("日本語", None),
        ("a'b", None),
        ("a\nb", None),
        (
            &"a".repeat(32),
            Some(&*format!("language-{}", "a".repeat(32))),
        ),
        (&"a".repeat(33), None),
    ] {
        assert_eq!(lang_class(lang).as_deref(), want, "{lang:?}");
    }
}

#[test]
fn 言語名の注入_t09_で_class_も_on_属性も出ない() {
    let case = testdata_case("T-09");
    let doc: Doc = serde_json::from_str(&case["tree"].to_string()).unwrap();
    let html = html_of(&doc);
    assert!(!html.contains("class="), "{html}");
    assert!(!html.contains("onmouseover="), "{html}");
    assert!(html.contains("<pre><code>x</code></pre>"), "{html}");
}

#[test]
fn 言語名が安全ならコードに_class_が付く() {
    let doc = doc(vec![Block::Code {
        lang: Some("rust".into()),
        text: "fn main() {}".into(),
    }]);
    let html = html_of(&doc);
    assert!(html.contains("<code class=\"language-rust\">"), "{html}");
}

#[test]
fn 任意の言語名で_class_が無いか安全な形になる() {
    let samples = [
        "rust",
        "a\"b",
        "x y",
        "\u{0}",
        "<script>",
        "a;b",
        "é",
        "a/b",
        "a\\b",
        "a=b",
        "javascript:alert(1)",
        "-",
        ".",
        "+",
        "_",
    ];
    for lang in samples {
        let html = html_of(&doc(vec![Block::Code {
            lang: Some(lang.to_owned()),
            text: "x".into(),
        }]));
        let tag = html
            .split("<code")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        if let Some(rest) = tag.strip_prefix(" class=\"") {
            let value = rest.trim_end_matches('"');
            assert!(value.starts_with("language-"), "{lang:?}: {tag}");
            assert!(
                value["language-".len()..]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "_+.-".contains(c)),
                "{lang:?}: {tag}"
            );
        } else {
            assert_eq!(tag, "", "{lang:?}: {tag}");
        }
    }
}

#[test]
fn class_付きの_code_は_a11y_の未定義の項目を持たない() {
    let doc = doc(vec![Block::Code {
        lang: Some("rust".into()),
        text: "x".into(),
    }]);
    assert_eq!(
        fandhe_workshop_web::a11y::check_no_undefined(&render_doc_node(&doc)),
        Ok(())
    );
}

/// 木が載る IR の 3 か所。
#[derive(Clone, Copy, Debug)]
enum Site {
    BodyDoc,
    BodyLevelsDoc,
    FixtureDocs,
}

const SITES: [Site; 3] = [Site::BodyDoc, Site::BodyLevelsDoc, Site::FixtureDocs];

/// 契約の試験データの IR の `site` の木を `doc` に差し替えたバイト列。
/// `core` の `doc_load` 試験の `full_ir` と同じ処理の複製（試験のヘルパは crate をまたいで使えないため）。
fn ir_bytes_with(site: Site, doc: &Doc) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/ir_format_2/full.json");
    let mut ir: Value =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read ir")).expect("parse ir");
    // 契約の試験データは参照先の節が 1 つ足りない。参照を自身に向けて、読み込める IR にする。
    ir["nodes"][0]["step"]["next"][0]["to"] = Value::from("main/a");
    let mark = Value::from("@@TREE@@");
    match site {
        Site::BodyDoc => ir["nodes"][0]["body_doc"] = mark,
        Site::BodyLevelsDoc => {
            *ir["nodes"][0]["body_levels_doc"]
                .as_object_mut()
                .expect("levels")
                .values_mut()
                .next()
                .expect("one") = mark;
        }
        Site::FixtureDocs => {
            *ir["fixture_docs"]
                .as_object_mut()
                .expect("fixtures")
                .values_mut()
                .next()
                .expect("one") = mark;
        }
    }
    // 深い木は `Value` を経由せず文字列で差し込む（直列化は再帰の上限に掛からない）。
    ir.to_string()
        .replace(
            "\"@@TREE@@\"",
            &serde_json::to_string(doc).expect("serialize"),
        )
        .into_bytes()
}

fn tree_at(ir: &fandhe_workshop_contract::ir::Ir, site: Site) -> Doc {
    match site {
        Site::BodyDoc => ir.nodes[0].body_doc.clone().expect("body_doc"),
        Site::BodyLevelsDoc => ir.nodes[0]
            .body_levels_doc
            .values()
            .next()
            .expect("level")
            .clone(),
        Site::FixtureDocs => ir.fixture_docs.values().next().expect("fixture").clone(),
    }
}

/// IR の JSON の深さが上限に収まる最大の `n` で `make(n)` の木を作る。
fn max_fit(site: Site, make: impl Fn(usize) -> Doc) -> usize {
    let mut n = 0;
    while json_depth(&ir_bytes_with(site, &make(n + 1))) <= MAX_JSON_DEPTH {
        n += 1;
    }
    n
}

fn list_nest_doc(n: usize) -> Doc {
    let mut blocks = Vec::new();
    for _ in 0..n {
        blocks = vec![Block::List {
            ordered: false,
            items: vec![blocks],
        }];
    }
    doc(blocks)
}

fn emph_nest_doc(n: usize) -> Doc {
    let mut inlines = vec![Inline::Text { text: "x".into() }];
    for _ in 0..n {
        inlines = vec![Inline::Emph { children: inlines }];
    }
    doc(vec![Block::Paragraph { children: inlines }])
}

#[test]
fn ir_から読める最も深い引用は描画の上限に届かない() {
    for site in SITES {
        let n = max_fit(site, quote_nest_doc);
        assert!(n > 0 && n < MAX_RENDER_DEPTH, "{site:?}: n={n}");
        let ir = load_ir(&ir_bytes_with(site, &quote_nest_doc(n))).expect("loads");
        let html = html_of(&tree_at(&ir, site));
        assert_eq!(count(&html, "<blockquote>"), n, "{site:?}");
        assert_eq!(count(&html, DEPTH_OMITTED_TEXT), 0, "{site:?}");
    }
}

#[test]
fn ir_から読める木は構造の種類によらず描画の深さが上限未満() {
    // 最も浅い位置（fixture_docs）で、リストと強調の最大の入れ子を調べる。
    let site = Site::FixtureDocs;
    let lists = max_fit(site, list_nest_doc);
    let emphs = max_fit(site, emph_nest_doc);
    assert!(lists > 0 && lists < MAX_RENDER_DEPTH, "lists={lists}");
    assert!(emphs > 0 && emphs < MAX_RENDER_DEPTH, "emphs={emphs}");
    for (name, n, make, tag) in [
        ("list", lists, list_nest_doc as fn(usize) -> Doc, "<ul>"),
        ("emph", emphs, emph_nest_doc as fn(usize) -> Doc, "<em>"),
    ] {
        let ir = load_ir(&ir_bytes_with(site, &make(n))).expect("loads");
        let html = html_of(&tree_at(&ir, site));
        assert_eq!(count(&html, tag), n, "{name}");
        assert_eq!(count(&html, DEPTH_OMITTED_TEXT), 0, "{name}");
        // 1 段多いと IR の深さの段で拒否される。
        assert!(
            json_depth(&ir_bytes_with(site, &make(n + 1))) > MAX_JSON_DEPTH,
            "{name}"
        );
    }
}

#[test]
fn compile_した教材パックの木は単独の関門も通り省略なしで描ける() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../pack/tests/data/fixture-pack");
    let bytes = fandhe_workshop_pack::compile::compile_dir(&dir).expect("compiles");
    let ir = load_ir(&bytes).expect("loads");
    let mut docs: Vec<Doc> = Vec::new();
    for n in &ir.nodes {
        docs.extend(n.body_doc.clone());
        docs.extend(n.body_levels_doc.values().cloned());
    }
    docs.extend(ir.fixture_docs.values().cloned());
    assert!(!docs.is_empty());
    for d in &docs {
        let json = serde_json::to_vec(d).expect("serialize");
        load_json::<Doc>(LoadEntry::Ir, &json).expect("passes the gate on its own");
        let html = html_of(d);
        assert!(!html.contains(DEPTH_OMITTED_TEXT), "{html}");
        assert!(!html.contains(VERSION_REJECTED_TEXT), "{html}");
    }
}
