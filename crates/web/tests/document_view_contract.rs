//! 文書のビュー（変種 (b)）の契約試験（TASK-476 1/2。ART-1・ART-7）。
//!
//! 固定データ（`doc-before.md`・`doc-after.md`）をビルド時の変換器（pack の `markdown::parse`。dev-dependency）で木にし、
//! `align_blocks` で行にして `doc_items` を通す。項目数・印の件数・構造の要素の数・項目の形の不変・a11y の閉じた一覧・
//! エスケープ・TASK-149/150 の規則が経路で効くことを確かめる。期待値は Markdown と手で数えた値から作り、実装の関数から導かない。
//! サンプルに無い構造（囲みのコード・区切り線）は手作りの行で確かめる。
//! 規模の確認は構成だけで時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::render;
use fandhe_workshop_contract::doc::{Block, Inline};
use fandhe_workshop_pack::markdown::parse;
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_doc_rows::align_blocks;
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{DocRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::doc_view::{DEPTH_OMITTED_TEXT, MAX_RENDER_DEPTH};
use fandhe_workshop_web::document_view::{doc_items, doc_list};

const BEFORE_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/doc-before.md"
));
const AFTER_MD: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/doc-after.md"
));
const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css"));

fn sample_rows() -> Vec<DocRow<Block>> {
    let before = parse(BEFORE_MD).blocks;
    let after = parse(AFTER_MD).blocks;
    align_blocks("document", &before, &after).expect("行")
}

fn htmls(rows: &[DocRow<Block>], phase: Phase) -> Vec<String> {
    doc_items(rows, phase, &no_point)
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

fn text(s: &str) -> Inline {
    Inline::Text { text: s.to_owned() }
}

#[test]
fn 項目数は前10_後13で_key_は一意() {
    let rows = sample_rows();
    for (phase, want) in [(Phase::Before, 10), (Phase::After, 13)] {
        let items = doc_items(&rows, phase, &no_point);
        assert_eq!(items.len(), want);
        let mut keys: Vec<&str> = items.iter().map(|(k, _)| k.as_str()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), want);
        assert!(items.iter().all(|(k, _)| k.starts_with("kdocument#")));
    }
}

#[test]
fn 印は前に無く_後は追加3_削除3_変更0_印なし7() {
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
fn 構造の要素の数は_markdown_を目で数えた値と一致する() {
    let rows = sample_rows();
    // 前: h1 1・h2 3・番号つき一覧 1・番号なし一覧 1・表 1（行 2）・引用 1・行内のコード 3（cargo run・notes/ ×2）。
    let b = joined(&rows, Phase::Before);
    assert_eq!(n(&b, "<h1"), 1);
    assert_eq!(n(&b, "<h2"), 3);
    assert_eq!(n(&b, "<ol"), 1);
    assert_eq!(n(&b, "<ul"), 1);
    assert_eq!(n(&b, "<table"), 1);
    assert_eq!(n(&b, "<thead"), 1);
    assert_eq!(n(&b, "<tbody"), 1);
    assert_eq!(n(&b, "<th>"), 2);
    assert_eq!(n(&b, "<td>"), 4);
    assert_eq!(n(&b, "<blockquote"), 1);
    assert_eq!(n(&b, "<code"), 3);
    assert_eq!(n(&b, "<li>"), 5);
    // 後: 削除と追加が組になるため、番号つき一覧・番号なし一覧・表は前後の 2 つが並ぶ。
    let a = joined(&rows, Phase::After);
    assert_eq!(n(&a, "<h1"), 1);
    assert_eq!(n(&a, "<h2"), 3);
    assert_eq!(n(&a, "<ol"), 2);
    assert_eq!(n(&a, "<ul"), 2);
    assert_eq!(n(&a, "<table"), 2);
    assert_eq!(n(&a, "<th>"), 4);
    assert_eq!(n(&a, "<td>"), 4 + 6);
    assert_eq!(n(&a, "<blockquote"), 1);
    assert_eq!(n(&a, "<li>"), 3 + 4 + 2 + 3);
    assert_eq!(n(&a, "UTF-8"), 1);
    assert_eq!(n(&a, "<th>項目</th><th>内容</th>"), 2);
}

#[test]
fn 全項目の根の子は記号_文字_包みの3つで_包みはブロックを1つ持つ() {
    let rows = sample_rows();
    for phase in [Phase::Before, Phase::After] {
        for (key, node) in doc_items(&rows, phase, &no_point) {
            let html = render(&node);
            // (b) は印を項目自身が持つ。(a) の形（内側の div が持つ）にならない。
            assert!(
                html.starts_with("<li data-kind=\"block\" data-mark=\""),
                "{key}"
            );
            assert_eq!(n(&html, "data-testid=\"mark-glyph\""), 1, "{key}");
            assert_eq!(n(&html, "data-testid=\"mark-text\""), 1, "{key}");
            assert_eq!(n(&html, "data-testid=\"doc-block\""), 1, "{key}");
            // 記号・文字・包みの順で、包みが最後の子。
            let glyph = html.find("mark-glyph").expect("glyph");
            let text = html.find("mark-text").expect("text");
            let wrap = html.find("doc-block").expect("wrap");
            assert!(glyph < text && text < wrap, "{key}");
        }
    }
}

#[test]
fn 同じ_key_の項目の形は前後で変わらない() {
    let rows = sample_rows();
    let strip = |html: &str| -> String {
        // 開きタグの先頭の要素名の列と、包みの直下までの前置きだけを比べる。
        let at = html.find("data-testid=\"doc-block\"").expect("wrap");
        html[at..]
            .split_once('>')
            .map(|(head, _)| head.to_owned())
            .expect(">")
    };
    let before = doc_items(&rows, Phase::Before, &no_point);
    let after = doc_items(&rows, Phase::After, &no_point);
    for (k, b) in &before {
        let a = after.iter().find(|(ak, _)| ak == k).expect("key");
        assert_eq!(strip(&render(b)), strip(&render(&a.1)), "{k}");
    }
}

#[test]
fn data_block_の値はブロックの種類に対応する() {
    let rows = sample_rows();
    let a = joined(&rows, Phase::After);
    assert_eq!(n(&a, "data-block=\"heading\""), 4);
    assert_eq!(n(&a, "data-block=\"paragraph\""), 2);
    assert_eq!(n(&a, "data-block=\"list\""), 4);
    assert_eq!(n(&a, "data-block=\"table\""), 2);
    assert_eq!(n(&a, "data-block=\"quote\""), 1);
    assert_eq!(n(&a, "data-block=\"code\""), 0);
}

#[test]
fn 囲みのコードは_pre_code_で空白と改行が残り_言語名は許可された文字だけ_class_になる() {
    let code = |lang: Option<&str>| Block::Code {
        lang: lang.map(str::to_owned),
        text: "fn main() {\n    let x = 1;\n}".to_owned(),
    };
    let ok = joined(&one_row(code(Some("rust"))), Phase::After);
    assert_eq!(n(&ok, "<pre><code class=\"language-rust\">"), 1, "{ok}");
    assert!(ok.contains("fn main() {\n    let x = 1;\n}"), "{ok}");
    assert_eq!(n(&ok, "data-block=\"code\""), 1);
    let bad = joined(&one_row(code(Some("rust\" onload=\"x"))), Phase::After);
    assert_eq!(n(&bad, "class=\"language-"), 0, "{bad}");
    assert_eq!(n(&bad, "onload"), 0, "{bad}");
    let none = joined(&one_row(code(None)), Phase::After);
    assert_eq!(n(&none, "<pre><code>"), 1, "{none}");
}

#[test]
fn 区切り線は_hr_が1つ() {
    let html = joined(&one_row(Block::Rule), Phase::After);
    assert_eq!(n(&html, "<hr"), 1, "{html}");
    assert_eq!(n(&html, "data-block=\"rule\""), 1);
}

#[test]
fn 教材由来の文字は要素にならずエスケープされる() {
    let evil = "<script>alert(\"x\")</script> & <img src=x>";
    let rows = one_row(Block::Table {
        head: vec![vec![text(evil)]],
        rows: vec![vec![vec![text(evil)]]],
    });
    let mut all = rows;
    all.extend(one_row(Block::Heading {
        level: 2,
        children: vec![text(evil)],
    }));
    all.extend(one_row(Block::Code {
        lang: None,
        text: evil.to_owned(),
    }));
    // key が衝突しないよう ID を振り直す。
    let all: Vec<DocRow<Block>> = all
        .into_iter()
        .enumerate()
        .map(|(i, r)| {
            Row::new(
                RowId::new(&format!("e#{i}")).expect("id"),
                0,
                RowKind::Block,
                r.before().cloned(),
                r.after().cloned(),
            )
            .expect("行")
        })
        .collect();
    let html = joined(&all, Phase::After);
    assert_eq!(n(&html, "<script"), 0, "{html}");
    assert_eq!(n(&html, "<img"), 0, "{html}");
    assert_eq!(n(&html, "&lt;script&gt;"), 4, "{html}");
    assert_eq!(n(&html, "&amp;"), 4, "{html}");
}

#[test]
fn 許可されないリンクは_a_にならず_外部の画像は_img_にならない() {
    let rows = one_row(Block::Paragraph {
        children: vec![
            Inline::Link {
                href: "javascript:alert(1)".to_owned(),
                children: vec![text("悪いリンク")],
            },
            Inline::Image {
                src: "https://example.invalid/a.png".to_owned(),
                alt: "外部".to_owned(),
            },
        ],
    });
    let html = joined(&rows, Phase::After);
    assert_eq!(n(&html, "<a "), 0, "{html}");
    assert_eq!(n(&html, "<img"), 0, "{html}");
    assert_eq!(n(&html, "javascript:"), 0, "{html}");
}

#[test]
fn 引用の入れ子が上限を超えると省略の文言になる() {
    let mut block = Block::Paragraph {
        children: vec![text("深い")],
    };
    for _ in 0..(MAX_RENDER_DEPTH + 4) {
        block = Block::Quote {
            children: vec![block],
        };
    }
    let html = joined(&one_row(block), Phase::After);
    assert_eq!(n(&html, DEPTH_OMITTED_TEXT), 1, "{html}");
    assert_eq!(n(&html, "深い"), 0, "{html}");
}

#[test]
fn a11y_は前後とも違反0で_role_と_aria_は無い() {
    let rows = sample_rows();
    for phase in [Phase::Before, Phase::After] {
        let items = doc_items(&rows, phase, &no_point);
        let list = doc_list("ul", true, items).expect("一覧");
        if let Err(v) = check_no_undefined(&list) {
            panic!("a11y の違反: {v:?}");
        }
        let html = render(&list);
        assert_eq!(n(&html, "role="), 0, "{html}");
        assert_eq!(n(&html, "aria-"), 0, "{html}");
        assert_eq!(n(&html, "data-row"), 0, "{html}");
        assert_eq!(n(&html, "tabindex"), 0, "{html}");
    }
}

#[test]
fn 選ばれていない一覧は空で_hidden() {
    let list = doc_list(
        "ul",
        false,
        doc_items(&sample_rows(), Phase::After, &no_point),
    )
    .expect("一覧");
    let html = render(&list);
    assert!(html.contains("hidden"), "{html}");
    assert_eq!(n(&html, "<li"), 0, "{html}");
}

#[test]
fn 規模の確認_80ブロックで構造の数が組み立てた数と一致する() {
    // 見出し・段落・一覧・表・引用・囲みのコードを循環させ、約 200 行の Markdown を作る。
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
    assert!(md.lines().count() >= 200);
    let blocks = parse(&md).blocks;
    assert_eq!(blocks.len(), 80);
    let rows = align_blocks("document", &blocks, &blocks).expect("行");
    let items = doc_items(&rows, Phase::After, &no_point);
    assert_eq!(items.len(), 80);
    let mut keys: Vec<&str> = items.iter().map(|(k, _)| k.as_str()).collect();
    keys.sort_unstable();
    keys.dedup();
    assert_eq!(keys.len(), 80);
    let html = joined(&rows, Phase::After);
    // i % 6 の各値は 80 の中で 14 回（0〜1 は 14、2〜5 は 13）。
    assert_eq!(n(&html, "<h2"), 14);
    assert_eq!(n(&html, "<ul"), 13);
    assert_eq!(n(&html, "<table"), 13);
    assert_eq!(n(&html, "<blockquote"), 13);
    assert_eq!(n(&html, "<pre><code class=\"language-rust\">"), 13);
}

#[test]
fn 後の状態で削除の直後は追加で_組は3つ_種類が一致する() {
    let rows = sample_rows();
    let after = htmls(&rows, Phase::After);
    let kind = |s: &str| -> String {
        let key = "data-block=\"";
        let at = s.find(key).expect("data-block") + key.len();
        s[at..].split('"').next().expect("値").to_owned()
    };
    let mut pairs: Vec<String> = Vec::new();
    for (i, h) in after.iter().enumerate() {
        if h.contains("data-mark=\"remove\"") {
            let next = after.get(i + 1).expect("削除の次の項目");
            assert!(next.contains("data-mark=\"add\""), "削除の直後は追加: {i}");
            assert_eq!(kind(h), kind(next), "組の種類が一致: {i}");
            assert!(h.contains("mk-glyph\">-<"), "{h}");
            assert!(next.contains("mk-glyph\">+<"), "{next}");
            pairs.push(kind(h));
        }
        if h.contains("data-mark=\"add\"") {
            let prev = after.get(i.wrapping_sub(1)).expect("追加の前の項目");
            assert!(
                prev.contains("data-mark=\"remove\""),
                "追加の直前は削除: {i}"
            );
        }
    }
    assert_eq!(pairs, ["list", "table", "list"]);
    let before = joined(&rows, Phase::Before);
    assert_eq!(n(&before, "mk-glyph\">-<"), 0);
    assert_eq!(n(&before, "mk-glyph\">+<"), 0);
}

#[test]
fn app_css_にブロックの包みのクラスがある() {
    assert!(APP_CSS.contains(".dc-block"));
}
