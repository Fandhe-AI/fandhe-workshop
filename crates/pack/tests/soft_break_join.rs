//! 行の途中の改行の連結の試験（04-behavior の PACK-27、D-51、TASK-61）。
//!
//! `markdown::parse` が、前後がともに CJK（かな・漢字・全角記号）の改行だけを連結し、英数字・混在・
//! インラインの境界の改行は `"\n"` のテキストとして残すことを確かめる。Chrome は `"\n"` を空白として
//! 描くため、連結しないと日本語の文の途中に空白が見える。描画幅の測定はブラウザ試験の範囲で、ここでは扱わない。

use std::path::Path;

use fandhe_workshop_contract::doc::{Block, Doc, Inline};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::markdown::parse;

fn inline_text(i: &Inline) -> String {
    match i {
        Inline::Text { text } | Inline::Code { text } => text.clone(),
        Inline::Break => "<br>".to_string(),
        Inline::Strong { children } | Inline::Emph { children } => {
            children.iter().map(inline_text).collect()
        }
        _ => "<?>".to_string(),
    }
}

fn para(doc: &Doc) -> Vec<Inline> {
    match &doc.blocks[0] {
        Block::Paragraph { children } => children.clone(),
        other => panic!("段落ではありません: {other:?}"),
    }
}

fn text_of(src: &str) -> String {
    para(&parse(src)).iter().map(inline_text).collect()
}

fn newline_nodes(children: &[Inline]) -> usize {
    children
        .iter()
        .filter(|i| matches!(i, Inline::Text { text } if text == "\n"))
        .count()
}

#[test]
fn pack27_japanese_soft_break_is_joined() {
    let d = parse("これは日本語の\n文章です。");
    assert_eq!(newline_nodes(&para(&d)), 0);
    assert_eq!(
        text_of("これは日本語の\n文章です。"),
        "これは日本語の文章です。"
    );
    for src in [
        "ひらがな\nカタカナ",
        "漢字\n漢字",
        "終わり。\n始まり",
        "「引用」\n「続き」",
        "ＡＢＣ\nＤＥＦ",
    ] {
        assert_eq!(newline_nodes(&para(&parse(src))), 0, "{src}");
        assert!(!text_of(src).contains('\n'), "{src}");
    }
}

#[test]
fn pack27_ascii_soft_break_is_kept() {
    assert_eq!(text_of("hello\nworld"), "hello\nworld");
    assert_eq!(newline_nodes(&para(&parse("hello\nworld"))), 1);
}

#[test]
fn pack27_mixed_soft_break_is_kept() {
    assert_eq!(text_of("日本語の\nabc"), "日本語の\nabc");
    assert_eq!(text_of("abc\n日本語の"), "abc\n日本語の");
}

#[test]
fn pack27_hard_break_is_unchanged() {
    for src in ["日本語の  \n文章", "日本語の\\\n文章"] {
        let c = para(&parse(src));
        assert!(c.contains(&Inline::Break), "{src}");
        assert_eq!(newline_nodes(&c), 0, "{src}");
    }
}

#[test]
fn pack27_inline_boundary_is_kept() {
    // 試作の規則の限界: 境界がコード・強調の節にあるときは連結しない。
    assert_eq!(text_of("`code`\n日本語"), "code\n日本語");
    assert_eq!(text_of("**強調**\n日本語"), "強調\n日本語");
}

#[test]
fn pack27_joins_inside_emphasis_list_and_heading() {
    assert_eq!(text_of("*日本語の\n文章*"), "日本語の文章");
    for src in ["- 日本語の\n  文章\n", "見出しの\n続き\n===\n"] {
        let j = serde_json::to_string(&parse(src)).unwrap();
        assert!(!j.contains("\\n"), "{j}");
    }
}

#[test]
fn pack27_compiled_pack_joins_cjk_and_keeps_ascii() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fixture-pack");
    let bytes = compile_dir(&dir).unwrap();
    let ir = load_ir(&bytes).unwrap();
    let a = ir.nodes[0].body_doc.as_ref().unwrap();
    let j = serde_json::to_string(a).unwrap();
    assert!(!j.contains("日本語の\"},{\"t\":\"text\",\"text\":\"\\n"));
    assert!(j.contains("alpha line\"},{\"t\":\"text\",\"text\":\"\\n\"}"));
}
