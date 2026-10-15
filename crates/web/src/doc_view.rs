//! 本文と練習データの木（[`Doc`]）を、画面の部品（[`Node`]）へ変える描画側。
//!
//! 呼び出し元は、Step の本文を画面に出す部品と、ブラウザ試験の口（`test_hooks`）である。pack の変換器
//! （`markdown::parse`）が作る木も、悪意のある教材パックが IR に直接書いた木も、同じ入口を通る
//! （方式 (c)。変換器では URL を検査しないので、スキームの許可は表示側のここが持つ。SAFE-9）。
//!
//! - リンクは [`is_allowed_link_href`] を通ったときだけ `a` にする。通らなければ、子の文字だけを `span` に出す。
//!   href の文字列は画面に出さない（fail-closed）。
//! - 表示基盤の `is_safe_url` は `//host` と `\\host` を通すため、ここでは独自の許可リストを先に掛ける。
//!   `is_safe_url` は 2 段目の守りとして残る。
//! - 属性値とテキストは表示基盤の既定のエスケープを通る。`RawHtml` は使わない。
//! - 画像は、この段階（TASK-149 の 1/2）では `img` を作らず、代替の文字だけを出す。相対のパスの画像の表示は
//!   2/2（Issue #143）がこの既定を置き換える。
//! - コードブロックの言語名は `class` に出さない。無害化した `class` は TASK-150 で入れる。
//! - 木の版の検査と描画の深さの上限は TASK-150 の範囲。再帰の深さは、読み込みの関門（JSON の深さ 64）が
//!   与える上限に頼る。関門を通さずにこの関数へ木を渡さないこと。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::doc::{Block, Doc, Inline};

/// 本文のリンクの href を許可するか。許可するのは `http`・`https`・スキームなし（相対）だけ。
///
/// 手順:
/// 1. タブ・改行（`\t`・`\n`・`\r`）を全位置から除き、先頭の C0 制御文字と空白を除く。ブラウザが URL の解析で
///    同じ文字を無視するので、`java\tscript:` や ` javascript:` の偽装をここで露出させる。
/// 2. `//`・`/\`・`\` で始まるものを拒否する（スキーム相対。外部ホストへの遷移になる）。
/// 3. 最初の `:` より前に `/`・`?`・`#`・`\` があればスキームなし（相対）とする。
/// 4. スキームがあれば `http`・`https`（ASCII の大文字小文字を区別しない）だけ許可する。
///    `javascript:`・`data:`・`vbscript:`・`mailto:`・`tel:` と未知のスキームは拒否する。
pub fn is_allowed_link_href(href: &str) -> bool {
    let cleaned: String = href
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let s = cleaned.trim_start_matches(|c: char| c <= ' ');
    if s.starts_with("//") || s.starts_with("/\\") || s.starts_with('\\') {
        return false;
    }
    match s.find(':') {
        None => true,
        Some(i) => {
            let prefix = &s[..i];
            if prefix.contains(['/', '?', '#', '\\']) {
                return true;
            }
            prefix.eq_ignore_ascii_case("http") || prefix.eq_ignore_ascii_case("https")
        }
    }
}

/// 木を画面の部品の列へ変える。
pub fn render_doc(doc: &Doc) -> Vec<Node> {
    doc.blocks.iter().map(render_block).collect()
}

/// 木を 1 つの `div` に包んで返す。試験用の口が `render` へ渡す単位で、画面に差し込む単位でもある。
pub fn render_doc_node(doc: &Doc) -> Node {
    el("div", vec![("data-testid", "doc")], render_doc(doc))
}

fn render_blocks(blocks: &[Block]) -> Vec<Node> {
    blocks.iter().map(render_block).collect()
}

fn render_inlines(inlines: &[Inline]) -> Vec<Node> {
    inlines.iter().map(render_inline).collect()
}

fn render_block(block: &Block) -> Node {
    match block {
        Block::Heading { level, children } => {
            let tag = match level {
                0 | 1 => "h1",
                2 => "h2",
                3 => "h3",
                4 => "h4",
                5 => "h5",
                _ => "h6",
            };
            el(tag, vec![], render_inlines(children))
        }
        Block::Paragraph { children } => el("p", vec![], render_inlines(children)),
        Block::Quote { children } => el("blockquote", vec![], render_blocks(children)),
        Block::List { ordered, items } => {
            let lis = items
                .iter()
                .map(|item| el("li", vec![], render_blocks(item)))
                .collect();
            el(if *ordered { "ol" } else { "ul" }, vec![], lis)
        }
        // 言語名は class に出さない（属性の注入の面を作らない）。
        Block::Code { text: body, .. } => {
            el("pre", vec![], vec![el("code", vec![], vec![text(body)])])
        }
        Block::Table { head, rows } => {
            let cells = |tag: &'static str, row: &[Vec<Inline>]| -> Node {
                el(
                    "tr",
                    vec![],
                    row.iter()
                        .map(|cell| el(tag, vec![], render_inlines(cell)))
                        .collect(),
                )
            };
            el(
                "table",
                vec![],
                vec![
                    el("thead", vec![], vec![cells("th", head)]),
                    el(
                        "tbody",
                        vec![],
                        rows.iter().map(|r| cells("td", r)).collect(),
                    ),
                ],
            )
        }
        Block::Rule => el("hr", vec![], vec![]),
    }
}

fn render_inline(inline: &Inline) -> Node {
    match inline {
        Inline::Text { text: t } => text(t),
        Inline::Code { text: t } => el("code", vec![], vec![text(t)]),
        Inline::Emph { children } => el("em", vec![], render_inlines(children)),
        Inline::Strong { children } => el("strong", vec![], render_inlines(children)),
        Inline::Link { href, children } => {
            if is_allowed_link_href(href) {
                // target は付けない。別タブを開かず、opener も渡さない。
                el(
                    "a",
                    vec![("href", href), ("rel", "noopener noreferrer")],
                    render_inlines(children),
                )
            } else {
                el("span", vec![], render_inlines(children))
            }
        }
        // src は読まない。通信も要素も作らない（2/2 で相対のパスだけ表示する）。
        Inline::Image { alt, .. } => el("span", vec![("data-image-blocked", "1")], vec![text(alt)]),
        Inline::Break => el("br", vec![], vec![]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 許可される_href() {
        for h in [
            "https://example.com/a",
            "http://x",
            "HTTPS://X",
            "/a/b",
            "./a",
            "a.md#x",
            "#top",
            "?q=1",
            "/a:b",
            "",
        ] {
            assert!(is_allowed_link_href(h), "{h:?}");
        }
    }

    #[test]
    fn 拒否される_href() {
        for h in [
            "javascript:alert(1)",
            "JaVaScRiPt:x",
            "java\tscript:x",
            "java\nscript:x",
            "  javascript:x",
            "\u{1}javascript:x",
            "data:text/html,x",
            "vbscript:x",
            "mailto:a@b",
            "tel:1",
            "foo:bar",
            "//example.invalid/x",
            "\\\\example.invalid\\x",
            "/\\example.invalid/x",
            "\\/example.invalid/x",
            "\t/\\example.invalid/x",
            "\t//example.invalid/x",
        ] {
            assert!(!is_allowed_link_href(h), "{h:?}");
        }
    }

    #[test]
    fn 表示基盤の検査は_スキーム相対を通す_だから独自の許可リストが先に要る() {
        assert!(fandhe_frontend_core::is_safe_url("//example.invalid/x"));
        assert!(!is_allowed_link_href("//example.invalid/x"));
    }
}
