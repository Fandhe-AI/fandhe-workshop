//! Markdown から安全な木（[`fandhe_workshop_contract::doc`]）を作る変換器。
//!
//! `compile` が Step の本文と、`.md` の練習データの内容を木に変えるときに呼ぶ。実行時のエンジンは
//! Markdown を解釈せず、ここで作った木だけを IR から読む（方式 (c)、D-51）。HTML は一切出さない。
//!
//! - `Event::Html`・`Event::InlineHtml`・HTML ブロックは構文として解釈せず、テキストにする。木の種別に
//!   `html`・`script` は無く、`<script>` は文字列として段落に入る。
//! - リンク・画像の URL は、ここでは検査せずそのまま木へ入れる。スキームの許可は表示側（TASK-149）が持つ。
//!   方式 (c) では変換器を通らない木も実行時に読むため、検査は実行時の側に置く。
//! - コードの言語名は、情報文字列の空白の前の最初の語だけを取る。
//! - 入れ子の深さが [`MAX_DOC_DEPTH`] を超えた部分は、構造を捨ててテキストだけの段落にまとめる。再帰は
//!   この上限で止まり、深い入力でもスタックを使い果たさない（上限は IR の JSON の入れ子の上限から導く）。
//! - 行の途中の改行（soft break）は `"\n"` のテキストにする。ただし直前のテキストの最後の文字と直後の
//!   テキストの最初の文字がともに CJK（かな・漢字・全角記号。範囲は [`is_cjk`]）なら連結し、節を出さない
//!   （D-51・PACK-27）。Chrome は `"\n"` を空白として描くため、日本語の文の途中に空白が見えるのを防ぐ。
//!   - 限界: 境界がコード・強調・リンクなどインラインの節にあるときは連結しない（試作の規則と同じ）。
//!     隣り合うテキストの節も 1 つにまとめない。`skip_to_text`（深さ上限・画像の alt）は改行を空白にする。
//!   - hard break（`Inline::Break`）とコードブロックの中の改行は対象外。
//! - 出力は決定的で、同じ入力からは同じ木になる。

use fandhe_workshop_contract::doc::{Block, Doc, Inline, MAX_DOC_DEPTH, TREE_VERSION};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

type Iter<'a> = std::iter::Peekable<Parser<'a>>;

/// Markdown の文字列を木にする。失敗しない（解釈できない構文はテキストになる）。
pub fn parse(src: &str) -> Doc {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let mut it: Iter = Parser::new_ext(src, opts).peekable();
    let blocks = parse_blocks(&mut it, 0, None);
    Doc {
        version: TREE_VERSION,
        blocks,
    }
}

/// 解釈せずに全文を 1 つのコードブロックにした木にする。`.md` 以外の練習データ（ログ・JSON など）に使う。
///
/// Markdown として解釈すると、`#` が見出しになるなど内容が化けるため、解釈しない。
pub fn plain_text(text: &str) -> Doc {
    Doc {
        version: TREE_VERSION,
        blocks: vec![Block::Code {
            lang: None,
            text: text.to_string(),
        }],
    }
}

fn is_inline_start(e: &Event) -> bool {
    matches!(
        e,
        Event::Text(_)
            | Event::Code(_)
            | Event::Html(_)
            | Event::InlineHtml(_)
            | Event::SoftBreak
            | Event::HardBreak
            | Event::Start(
                Tag::Emphasis
                    | Tag::Strong
                    | Tag::Strikethrough
                    | Tag::Link { .. }
                    | Tag::Image { .. }
            )
    )
}

fn text_para(text: String) -> Block {
    Block::Paragraph {
        children: vec![Inline::Text { text }],
    }
}

/// `end` に当たるまで（`None` は入力の終わりまで）ブロックを読む。`end` のイベントは消費する。
fn parse_blocks(it: &mut Iter, depth: usize, end: Option<TagEnd>) -> Vec<Block> {
    let mut out = Vec::new();
    let mut pending: Vec<Inline> = Vec::new();
    let flush = |pending: &mut Vec<Inline>, out: &mut Vec<Block>| {
        if !pending.is_empty() {
            out.push(Block::Paragraph {
                children: std::mem::take(pending),
            });
        }
    };
    while let Some(ev) = it.peek() {
        if let Event::End(t) = ev {
            if Some(*t) == end {
                it.next();
                break;
            }
        }
        if is_inline_start(ev) {
            let mut inl = parse_inlines(it, depth + 1, None);
            pending.append(&mut inl);
            continue;
        }
        let Some(ev) = it.next() else { break };
        flush(&mut pending, &mut out);
        match ev {
            Event::Start(tag) => {
                if depth >= MAX_DOC_DEPTH {
                    out.push(text_para(skip_to_text(it)));
                    continue;
                }
                match tag {
                    Tag::Paragraph => {
                        let c = parse_inlines(it, depth + 1, Some(TagEnd::Paragraph));
                        out.push(Block::Paragraph { children: c });
                    }
                    Tag::Heading { level, .. } => {
                        let c = parse_inlines(it, depth + 1, Some(TagEnd::Heading(level)));
                        out.push(Block::Heading {
                            level: level as u8,
                            children: c,
                        });
                    }
                    Tag::BlockQuote(k) => {
                        let c = parse_blocks(it, depth + 1, Some(TagEnd::BlockQuote(k)));
                        out.push(Block::Quote { children: c });
                    }
                    Tag::CodeBlock(kind) => {
                        let lang = match &kind {
                            CodeBlockKind::Fenced(l) => l
                                .split_whitespace()
                                .next()
                                .filter(|w| !w.is_empty())
                                .map(str::to_string),
                            CodeBlockKind::Indented => None,
                        };
                        let mut text = String::new();
                        for e in it.by_ref() {
                            match e {
                                Event::End(TagEnd::CodeBlock) => break,
                                Event::Text(t)
                                | Event::Html(t)
                                | Event::InlineHtml(t)
                                | Event::Code(t) => text.push_str(&t),
                                _ => {}
                            }
                        }
                        out.push(Block::Code { lang, text });
                    }
                    Tag::List(first) => {
                        let mut items = Vec::new();
                        while let Some(e) = it.next() {
                            match e {
                                Event::Start(Tag::Item) => {
                                    items.push(parse_blocks(it, depth + 1, Some(TagEnd::Item)));
                                }
                                Event::End(TagEnd::List(_)) => break,
                                _ => {}
                            }
                        }
                        out.push(Block::List {
                            ordered: first.is_some(),
                            items,
                        });
                    }
                    Tag::Table(_) => out.push(parse_table(it, depth)),
                    Tag::HtmlBlock => {
                        // 生の HTML のブロックは、テキストとして段落に入れる。
                        let mut text = String::new();
                        for e in it.by_ref() {
                            match e {
                                Event::End(TagEnd::HtmlBlock) => break,
                                Event::Html(t) | Event::Text(t) => text.push_str(&t),
                                _ => {}
                            }
                        }
                        out.push(text_para(text));
                    }
                    _ => out.push(text_para(skip_to_text(it))),
                }
            }
            Event::Rule => out.push(Block::Rule),
            _ => {}
        }
    }
    flush(&mut pending, &mut out);
    out
}

fn parse_table(it: &mut Iter, depth: usize) -> Block {
    let mut head: Vec<Vec<Inline>> = Vec::new();
    let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
    let mut cur: Vec<Vec<Inline>> = Vec::new();
    let mut in_head = false;
    while let Some(e) = it.next() {
        match e {
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                head = std::mem::take(&mut cur);
                in_head = false;
            }
            Event::Start(Tag::TableRow) => cur = Vec::new(),
            Event::End(TagEnd::TableRow) => {
                if !in_head {
                    rows.push(std::mem::take(&mut cur));
                }
            }
            Event::Start(Tag::TableCell) => {
                cur.push(parse_inlines(it, depth + 1, Some(TagEnd::TableCell)));
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }
    Block::Table { head, rows }
}

/// `end` に当たるまで（`None` はインラインでなくなるまで）インラインを読む。
fn parse_inlines(it: &mut Iter, depth: usize, end: Option<TagEnd>) -> Vec<Inline> {
    let mut out = Vec::new();
    while let Some(ev) = it.peek() {
        match ev {
            Event::End(t) => {
                if Some(*t) == end {
                    it.next();
                }
                break;
            }
            e if end.is_none() && !is_inline_start(e) => break,
            _ => {}
        }
        let Some(ev) = it.next() else { break };
        match ev {
            Event::Text(t) | Event::Html(t) | Event::InlineHtml(t) => out.push(Inline::Text {
                text: t.to_string(),
            }),
            Event::Code(t) => out.push(Inline::Code {
                text: t.to_string(),
            }),
            Event::SoftBreak => {
                if !joins_soft_break(out.last(), it.peek()) {
                    out.push(Inline::Text {
                        text: "\n".to_string(),
                    });
                }
            }
            Event::HardBreak => out.push(Inline::Break),
            Event::Start(tag) => {
                if depth >= MAX_DOC_DEPTH {
                    out.push(Inline::Text {
                        text: skip_to_text(it),
                    });
                    continue;
                }
                match tag {
                    Tag::Emphasis => out.push(Inline::Emph {
                        children: parse_inlines(it, depth + 1, Some(TagEnd::Emphasis)),
                    }),
                    Tag::Strong => out.push(Inline::Strong {
                        children: parse_inlines(it, depth + 1, Some(TagEnd::Strong)),
                    }),
                    Tag::Strikethrough => {
                        out.extend(parse_inlines(it, depth + 1, Some(TagEnd::Strikethrough)));
                    }
                    Tag::Link { dest_url, .. } => out.push(Inline::Link {
                        href: dest_url.to_string(),
                        children: parse_inlines(it, depth + 1, Some(TagEnd::Link)),
                    }),
                    Tag::Image { dest_url, .. } => {
                        let alt = skip_to_text(it);
                        out.push(Inline::Image {
                            src: dest_url.to_string(),
                            alt,
                        });
                    }
                    _ => out.push(Inline::Text {
                        text: skip_to_text(it),
                    }),
                }
            }
            _ => {}
        }
    }
    out
}

/// 行の途中の改行を連結するか。直前の出力が `Text` で末尾が CJK、直後のイベントが `Text` で先頭が CJK のときだけ
/// 真になる。`parse_inlines` が `SoftBreak` を見たときに呼ぶ。空文字列でも panic しない。
fn joins_soft_break(prev: Option<&Inline>, next: Option<&Event>) -> bool {
    let prev_cjk =
        matches!(prev, Some(Inline::Text { text }) if text.chars().next_back().is_some_and(is_cjk));
    let next_cjk = matches!(next, Some(Event::Text(t)) if t.chars().next().is_some_and(is_cjk));
    prev_cjk && next_cjk
}

/// 連結の対象にする「CJK の文字」。範囲は試作の変換器に合わせ、広げない。
fn is_cjk(c: char) -> bool {
    matches!(c,
        '\u{3000}'..='\u{303F}' // CJK の記号・句読点
        | '\u{3040}'..='\u{30FF}' // ひらがな・カタカナ
        | '\u{3400}'..='\u{4DBF}' // CJK 統合漢字拡張 A
        | '\u{4E00}'..='\u{9FFF}' // CJK 統合漢字
        | '\u{F900}'..='\u{FAFF}' // CJK 互換漢字
        | '\u{FF00}'..='\u{FFEF}' // 全角形・半角カナ
        | '\u{20000}'..='\u{2FFFF}' // 拡張 B 以降
    )
}

/// 開始タグの直後から、対応する終了タグまでを入れ子を数えながら読み飛ばし、含まれるテキストだけを返す。
/// 反復で読むので、入れ子が深くてもスタックを使わない。
fn skip_to_text(it: &mut Iter) -> String {
    let mut depth = 1usize;
    let mut s = String::new();
    for e in it.by_ref() {
        match e {
            Event::Start(_) => depth += 1,
            Event::End(_) => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            Event::Text(t) | Event::Code(t) | Event::Html(t) | Event::InlineHtml(t) => {
                s.push_str(&t);
            }
            Event::SoftBreak | Event::HardBreak => s.push(' '),
            _ => {}
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn first_para_text(doc: &Doc) -> String {
        match &doc.blocks[0] {
            Block::Paragraph { children } => children
                .iter()
                .map(|i| match i {
                    Inline::Text { text } => text.clone(),
                    _ => "<?>".to_string(),
                })
                .collect(),
            _ => panic!("段落ではありません"),
        }
    }

    #[test]
    fn raw_html_becomes_text() {
        let d = parse("<script>alert(1)</script>\n\ntext");
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("script"));
        assert!(!json.contains("\"t\":\"script\""));
        let d = parse("a <b onclick=x>b</b> c");
        let json = serde_json::to_string(&d).unwrap();
        assert!(!json.contains("\"t\":\"b\""));
    }

    #[test]
    fn soft_breaks_join_only_between_cjk() {
        assert_eq!(first_para_text(&parse("hello\nworld")), "hello\nworld");
        assert_eq!(
            first_para_text(&parse("これは日本語の\n文章です。")),
            "これは日本語の文章です。"
        );
        assert_eq!(first_para_text(&parse("日本語の\nabc")), "日本語の\nabc");
    }

    #[test]
    fn code_block_without_lang_has_no_null() {
        let d = parse("```\nx\n```\n\n    indented\n");
        let json = serde_json::to_string(&d).unwrap();
        assert!(!json.contains("null") && !json.contains("lang"), "{json}");
        let d = parse("```rust extra\nx\n```\n");
        assert_eq!(
            d.blocks[0],
            Block::Code {
                lang: Some("rust".into()),
                text: "x\n".into()
            }
        );
    }

    #[test]
    fn covers_the_vocabulary() {
        let src = "# H\n\npara **s** *e* `c` [l](https://e.com) ![a](i.png)\n\n> q\n\n1. a\n2. b\n\n---\n\n| h |\n|---|\n| r |\n";
        let d = parse(src);
        let kinds: Vec<&str> = d
            .blocks
            .iter()
            .map(|b| match b {
                Block::Heading { .. } => "heading",
                Block::Paragraph { .. } => "paragraph",
                Block::Quote { .. } => "quote",
                Block::List { .. } => "list",
                Block::Code { .. } => "code",
                Block::Table { .. } => "table",
                Block::Rule => "rule",
            })
            .collect();
        assert_eq!(
            kinds,
            ["heading", "paragraph", "quote", "list", "rule", "table"]
        );
    }

    #[test]
    fn same_input_gives_same_tree() {
        let src = "# a\n\n- x\n- y\n\n> z\n";
        assert_eq!(parse(src), parse(src));
    }

    #[test]
    fn plain_text_is_one_code_block() {
        let d = plain_text("# not a heading\n{\"a\":1}\n");
        assert_eq!(
            d.blocks,
            vec![Block::Code {
                lang: None,
                text: "# not a heading\n{\"a\":1}\n".into()
            }]
        );
    }

    fn tree_depth_blocks(bs: &[Block]) -> usize {
        bs.iter()
            .map(|b| match b {
                Block::Quote { children } => 1 + tree_depth_blocks(children),
                Block::List { items, .. } => {
                    1 + items
                        .iter()
                        .map(|i| tree_depth_blocks(i))
                        .max()
                        .unwrap_or(0)
                }
                _ => 1,
            })
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn deep_nesting_is_cut_at_the_limit() {
        for src in [
            format!("{}x\n", "> ".repeat(1000)),
            format!(
                "{}x\n",
                (0..1000).fold(String::new(), |acc, i| format!(
                    "{acc}{}- ",
                    " ".repeat(i % 2)
                ))
            ),
            format!("{}x{}\n", "*a ".repeat(1000), "*".repeat(1000)),
        ] {
            let d = parse(&src);
            assert!(tree_depth_blocks(&d.blocks) <= MAX_DOC_DEPTH + 2);
        }
    }
}
