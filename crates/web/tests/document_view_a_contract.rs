//! 文書のビュー（変種 (a)。対照専用）の契約試験（TASK-476 2/2。ART-1・ART-7）。
//!
//! 固定データ（`doc-before.md`・`doc-after.md`）をビルド時の変換器（pack の `markdown::parse`。dev-dependency）で木にし、
//! `align_blocks` で行にして `doc_items_a` を通す。項目数・印の宿主・構造の喪失（表・引用・一覧が平文の行になる）・
//! 文字が残ること・エスケープ・深さの上限・a11y の閉じた一覧を確かめる。期待値は Markdown と手で数えた値から作り、
//! 実装の関数から導かない。規模の確認は構成だけで時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::render;
use fandhe_workshop_contract::doc::{Block, Inline};
use fandhe_workshop_pack::markdown::parse;
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_doc_rows::align_blocks;
use fandhe_workshop_web::artifact_view::{DocRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::doc_view::{DEPTH_OMITTED_TEXT, MAX_RENDER_DEPTH};
use fandhe_workshop_web::document_view::{doc_items_a, doc_list};

const BEFORE_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/doc-before.md"
));
const AFTER_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/doc-after.md"
));

fn sample_rows() -> Vec<DocRow<Block>> {
    let before = parse(BEFORE_MD).blocks;
    let after = parse(AFTER_MD).blocks;
    align_blocks("document", &before, &after).expect("行")
}

fn htmls(rows: &[DocRow<Block>], phase: Phase) -> Vec<String> {
    doc_items_a(rows, phase)
        .iter()
        .map(|(_, n)| render(n))
        .collect()
}

fn joined(rows: &[DocRow<Block>], phase: Phase) -> String {
    htmls(rows, phase).concat()
}

fn n(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

fn text(s: &str) -> Inline {
    Inline::Text { text: s.to_owned() }
}

fn one_row(block: Block) -> Vec<DocRow<Block>> {
    vec![Row::new(
        RowId::new("x#0").expect("id"),
        0,
        RowKind::Block,
        Some(block.clone()),
        Some(block),
    )
    .expect("行")]
}

/// 開きタグの属性値と文字を落として、形だけにする。
fn shape(html: &str) -> String {
    let mut out = String::new();
    let mut in_val = false;
    let mut in_tag = false;
    for c in html.chars() {
        match (in_tag, in_val, c) {
            (_, false, '<') => {
                in_tag = true;
                out.push(c);
            }
            (true, false, '"') => in_val = true,
            (true, true, '"') => in_val = false,
            (true, true, _) => {}
            (true, false, '>') => {
                in_tag = false;
                out.push(c);
            }
            (true, false, _) => out.push(c),
            (false, _, _) => {}
        }
    }
    out
}

#[test]
fn a_項目数は前10_後13で_key_は一意() {
    let rows = sample_rows();
    for (phase, want) in [(Phase::Before, 10), (Phase::After, 13)] {
        let items = doc_items_a(&rows, phase);
        assert_eq!(items.len(), want);
        let mut keys: Vec<&str> = items.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), want);
        assert!(items.iter().all(|(k, _)| k.starts_with("kdocument#")));
    }
}

#[test]
fn a_項目の形は_li_の子が内側の_div_1つで_印の宿主はその_div() {
    let rows = sample_rows();
    for phase in [Phase::Before, Phase::After] {
        for (key, node) in doc_items_a(&rows, phase) {
            let html = render(&node);
            assert!(html.starts_with("<li data-kind=\"block\""), "{key} {html}");
            assert!(!html.contains("<li data-mark"), "{key}");
            assert!(
                html.contains("<div data-mark=\""),
                "{key}: 印の宿主は内側の div"
            );
            assert_eq!(n(&html, "data-mark="), 1, "{key}");
            assert_eq!(n(&html, "data-testid=\"doc-block\""), 1, "{key}");
            let glyph = html.find("mark-glyph").expect("glyph");
            let label = html.find("mark-text").expect("text");
            let wrap = html.find("doc-block").expect("wrap");
            assert!(glyph < label && label < wrap, "{key}");
        }
    }
}

#[test]
fn a_構造の要素は1つも出ず_平文の部品だけが出る() {
    let rows = sample_rows();
    for phase in [Phase::Before, Phase::After] {
        let html = joined(&rows, phase);
        for tag in [
            "<h1",
            "<h2",
            "<h3",
            "<h4",
            "<h5",
            "<h6",
            "<ol",
            "<ul",
            "<table",
            "<thead",
            "<tbody",
            "<th",
            "<td",
            "<blockquote",
            "<pre",
            "<code",
            "<a ",
            "<img",
            "<hr",
        ] {
            assert_eq!(n(&html, tag), 0, "{tag}");
        }
        // li は項目の根だけである（本文の一覧は li にならない）。
        assert_eq!(n(&html, "<li"), htmls(&rows, phase).len());
    }
}

#[test]
fn a_行の数は_markdown_を目で数えた値と一致する() {
    let rows = sample_rows();
    // 前: 見出しの行 5（h1・準備・保存の仕組み・表の見出し行・次の手順）。
    //     段落の行 10（導入 1・準備の一覧 3・保存の本文 1・表の本体 2・引用 1・次の手順の一覧 2）。
    let b = joined(&rows, Phase::Before);
    assert_eq!(n(&b, "fw-wire-text"), 5);
    assert_eq!(n(&b, "fw-wire-paragraph"), 10);
    // 後: 削除と追加が組になる。見出しの行 6（表の見出し行が 2 つ）、
    //     段落の行 20（導入 1・一覧 3+4・保存の本文 1・表の本体 2+3・引用 1・一覧 2+3）。
    let a = joined(&rows, Phase::After);
    assert_eq!(n(&a, "fw-wire-text"), 6);
    assert_eq!(n(&a, "fw-wire-paragraph"), 20);
}

#[test]
fn a_文字は残り_表は縦棒で連結される() {
    let rows = sample_rows();
    let b = joined(&rows, Phase::Before);
    for want in [
        "項目 | 内容",
        "形式 | Markdown",
        "場所 | notes/",
        "cargo run で起動します。",
        "注意: 保存先を変えると、古いメモは見えなくなります。",
    ] {
        assert_eq!(n(&b, want), 1, "{want}");
    }
    assert_eq!(n(&b, "UTF-8"), 0);
    let a = joined(&rows, Phase::After);
    assert_eq!(n(&a, "文字コード | UTF-8"), 1);
    assert_eq!(n(&a, "項目 | 内容"), 2);
}

#[test]
fn a_印は前に無く_後は追加3_削除3_変更0_印なし7() {
    let rows = sample_rows();
    let before = joined(&rows, Phase::Before);
    for m in ["add", "remove", "change"] {
        assert_eq!(n(&before, &format!("data-mark=\"{m}\"")), 0, "{m}");
    }
    let after = joined(&rows, Phase::After);
    assert_eq!(n(&after, "data-mark=\"add\""), 3);
    assert_eq!(n(&after, "data-mark=\"remove\""), 3);
    assert_eq!(n(&after, "data-mark=\"change\""), 0);
    assert_eq!(n(&after, "data-mark=\"\""), 7);
}

#[test]
fn a_同じ_key_の項目の形は前後で変わらない() {
    let rows = sample_rows();
    let before = doc_items_a(&rows, Phase::Before);
    let after = doc_items_a(&rows, Phase::After);
    for (k, b) in &before {
        let a = after.iter().find(|(ak, _)| ak == k).expect("key");
        assert_eq!(shape(&render(b)), shape(&render(&a.1)), "{k}");
    }
}

#[test]
fn a_教材由来の文字は要素にならずエスケープされ_リンクと画像の宛先は出ない() {
    let evil = "<script>alert(\"x\")</script> & <img src=x onerror=y>";
    let block = Block::Paragraph {
        children: vec![
            text(evil),
            Inline::Link {
                href: "javascript:alert(1)".to_owned(),
                children: vec![text("リンク")],
            },
            Inline::Image {
                src: "https://example.invalid/a.png".to_owned(),
                alt: "外部\" onload=\"x".to_owned(),
            },
        ],
    };
    let html = joined(&one_row(block), Phase::After);
    assert_eq!(n(&html, "<script"), 0, "{html}");
    assert_eq!(n(&html, "<img"), 0, "{html}");
    assert_eq!(n(&html, "<a "), 0, "{html}");
    assert_eq!(n(&html, "&lt;script&gt;"), 1, "{html}");
    assert_eq!(n(&html, "javascript:"), 0, "{html}");
    assert_eq!(n(&html, "example.invalid"), 0, "{html}");
    assert_eq!(n(&html, "onload=\"x"), 0, "{html}");
    assert!(html.contains("リンク"), "{html}");
}

#[test]
fn a_囲みのコードは文字が1行に残り_区切り線は行が無い() {
    let code = Block::Code {
        lang: Some("rust".to_owned()),
        text: "fn main() {\n    let x = 1;\n}".to_owned(),
    };
    let html = joined(&one_row(code), Phase::After);
    assert_eq!(n(&html, "<pre"), 0, "{html}");
    assert_eq!(n(&html, "<code"), 0, "{html}");
    assert_eq!(n(&html, "language-"), 0, "{html}");
    assert_eq!(n(&html, "fw-wire-paragraph"), 1, "{html}");
    assert!(html.contains("fn main() {\n    let x = 1;\n}"), "{html}");
    let rule = joined(&one_row(Block::Rule), Phase::After);
    assert_eq!(n(&rule, "<hr"), 0, "{rule}");
    assert_eq!(n(&rule, "fw-wire-"), 0, "{rule}");
    assert_eq!(n(&rule, "data-block=\"rule\""), 1, "{rule}");
}

#[test]
fn a_引用の入れ子が上限を超えても_panic_せず省略の文言が1回だけ出る() {
    let mut block = Block::Paragraph {
        children: vec![text("深い")],
    };
    for _ in 0..(MAX_RENDER_DEPTH + 5) {
        block = Block::Quote {
            children: vec![block],
        };
    }
    let html = joined(&one_row(block), Phase::After);
    assert_eq!(n(&html, DEPTH_OMITTED_TEXT), 1, "{html}");
    assert_eq!(n(&html, "深い"), 0, "{html}");
}

#[test]
fn a_a11y_は前後とも違反0で_role_と_aria_は無い() {
    let rows = sample_rows();
    for phase in [Phase::Before, Phase::After] {
        let list = doc_list("ul", true, doc_items_a(&rows, phase)).expect("一覧");
        if let Err(v) = check_no_undefined(&list) {
            panic!("a11y の違反: {v:?}");
        }
        let html = render(&list);
        assert_eq!(n(&html, "role="), 0, "{html}");
        assert_eq!(n(&html, "aria-"), 0, "{html}");
        assert_eq!(n(&html, "style="), 0, "{html}");
    }
}

#[test]
fn a_規模の確認_80ブロックで項目は80件で構造の要素は0() {
    let mut md = String::new();
    for i in 0..80 {
        match i % 6 {
            0 => md.push_str(&format!("## 見出し {i}\n\n")),
            1 => md.push_str(&format!("段落 {i} の本文です。\n\n")),
            2 => md.push_str(&format!("- 項目 {i} a\n- 項目 {i} b\n\n")),
            3 => md.push_str(&format!(
                "| 列 | 値 |\n|----|----|\n| a | {i} |\n| b | {i} |\n\n"
            )),
            4 => md.push_str(&format!("> 引用 {i}\n\n")),
            _ => md.push_str(&format!("```rust\nlet x = {i};\n```\n\n")),
        }
    }
    let blocks = parse(&md).blocks;
    assert_eq!(blocks.len(), 80);
    let rows = align_blocks("document", &blocks, &blocks).expect("行");
    let items = doc_items_a(&rows, Phase::After);
    assert_eq!(items.len(), 80);
    let html = joined(&rows, Phase::After);
    for tag in [
        "<h2",
        "<table",
        "<blockquote",
        "<pre",
        "<code",
        "<ul",
        "<ol",
    ] {
        assert_eq!(n(&html, tag), 0, "{tag}");
    }
}
