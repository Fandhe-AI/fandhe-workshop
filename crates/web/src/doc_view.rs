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
//! - 画像は [`is_allowed_image_src`] を通ったとき（教材に同梱する相対のパス）だけ `img` と代替文にする。通らない
//!   （外部ホスト・危険なスキーム）ときは `img` を作らず、代替の文字だけを `span` に出す。src は画面に出さない
//!   （fail-closed）。外部の画像は表示するだけで外部への通信になるため（SAFE-10。TASK-149 2/2）。
//! - コードブロックの言語名は、許可した文字だけから成るときに限り `class="language-…"` に出す（[`lang_class`]）。
//!   そうでなければ `class` を付けない（属性の注入の面を作らない）。
//! - 描画の再帰は [`MAX_RENDER_DEPTH`] 段で止め、それより深い部分は [`DEPTH_OMITTED_TEXT`] の文字にする（SAFE-12）。
//!   読み込みの関門（JSON の深さ 64）に頼らず、この関数自身が深さを抑える（多層防御）。
//! - 木の版が [`TREE_VERSION`] と違う `Doc` は描画せず、固定の文言だけを出す。読み込みの段で拒否されるので、
//!   Rust から直接作った木が渡されたときの 2 段目の守りである。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::doc::{Block, Doc, Inline, TREE_VERSION};

/// 描画の再帰の深さの上限。最上位のブロック列を 0 段とし、引用の子・リストの項目・強調やリンクの子へ
/// 入るたびに 1 段増やす。上限に達した列は、子を描かず省略の文言だけを出す。
///
/// 変換器の上限（contract の `MAX_DOC_DEPTH` = 12）より大きいので、変換器の出力は省略にならない。
pub const MAX_RENDER_DEPTH: usize = 32;

/// 深さの上限で省略した部分に出す文言。
pub const DEPTH_OMITTED_TEXT: &str = "（入れ子が深すぎるため省略）";

/// 未対応の版の木の代わりに出す文言。
pub const VERSION_REJECTED_TEXT: &str = "（未対応の版の本文のため表示しません）";

/// コードの言語名の長さの上限（バイト）。
const MAX_LANG_LEN: usize = 32;

/// コードの言語名から `class` の値を作る。許可した文字だけから成るときに限り `language-<lang>` を返す。
///
/// 許可するのは ASCII の英数字と `_`・`+`・`.`・`-`、1〜32 バイト。1 文字でも外れれば `None`（文字を
/// 間引いて残さない）。`class` の値はさらに表示基盤の既定のエスケープを通る。
pub fn lang_class(lang: &str) -> Option<String> {
    let ok = !lang.is_empty()
        && lang.len() <= MAX_LANG_LEN
        && lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '.' | '-'));
    ok.then(|| format!("language-{lang}"))
}

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

/// 本文の画像の src を許可するか。許可するのは空でない相対のパスだけ。
///
/// 外部ホストの画像は、表示するだけで外部への通信（追跡・情報の持ち出しの経路）になるため許可しない
/// （SAFE-10）。外部画像を許す設定は SAFE-36（TASK-172・200）の範囲で、ここでは扱わない。
///
/// 手順:
/// 1. リンクと同じ正規化（`\t`・`\n`・`\r` を除き、先頭の C0 制御文字と空白を除く）をする。
/// 2. 空なら拒否する（空の src は文書自身を指し、意味がない）。
/// 3. [`is_allowed_link_href`] が拒否するもの（スキーム相対・`http`・`https` 以外のスキーム）を拒否する。
///    判定を 2 か所に分けないよう、リンクの規則を流用する。
/// 4. スキームを持つもの（最初の `:` より前に `/`・`?`・`#`・`\` が無い）を拒否する。`http`・`https` の外部画像はここで落ちる。
pub fn is_allowed_image_src(src: &str) -> bool {
    let cleaned: String = src
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let s = cleaned.trim_start_matches(|c: char| c <= ' ');
    if s.is_empty() || !is_allowed_link_href(s) {
        return false;
    }
    match s.find(':') {
        None => true,
        Some(i) => s[..i].contains(['/', '?', '#', '\\']),
    }
}

/// 木を画面の部品の列へ変える。
pub fn render_doc(doc: &Doc) -> Vec<Node> {
    if doc.version != TREE_VERSION {
        return vec![el("p", vec![], vec![text(VERSION_REJECTED_TEXT)])];
    }
    render_blocks(&doc.blocks, 0)
}

/// 木を 1 つの `div` に包んで返す。試験用の口が `render` へ渡す単位で、画面に差し込む単位でもある。
pub fn render_doc_node(doc: &Doc) -> Node {
    el("div", vec![("data-testid", "doc")], render_doc(doc))
}

/// ブロックの列を描く。`depth` が上限に達していれば、省略の段落を 1 つだけ返す。
fn render_blocks(blocks: &[Block], depth: usize) -> Vec<Node> {
    if depth >= MAX_RENDER_DEPTH {
        return vec![el("p", vec![], vec![text(DEPTH_OMITTED_TEXT)])];
    }
    blocks.iter().map(|b| render_block(b, depth)).collect()
}

/// インラインの列を描く。`depth` が上限に達していれば、省略の文字を 1 つだけ返す。
fn render_inlines(inlines: &[Inline], depth: usize) -> Vec<Node> {
    if depth >= MAX_RENDER_DEPTH {
        return vec![text(DEPTH_OMITTED_TEXT)];
    }
    inlines.iter().map(|i| render_inline(i, depth)).collect()
}

/// `depth` はこのブロックが並ぶ列の深さ。見出し・段落・表のセルのインラインも同じ深さから始める。
fn render_block(block: &Block, depth: usize) -> Node {
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
            el(tag, vec![], render_inlines(children, depth))
        }
        Block::Paragraph { children } => el("p", vec![], render_inlines(children, depth)),
        Block::Quote { children } => el("blockquote", vec![], render_blocks(children, depth + 1)),
        Block::List { ordered, items } => {
            let lis = items
                .iter()
                .map(|item| el("li", vec![], render_blocks(item, depth + 1)))
                .collect();
            el(if *ordered { "ol" } else { "ul" }, vec![], lis)
        }
        // 言語名は許可した文字だけのときに限り class に出す（属性の注入の面を作らない）。
        Block::Code { lang, text: body } => {
            let class = lang.as_deref().and_then(lang_class);
            let attrs = match class.as_deref() {
                Some(c) => vec![("class", c)],
                None => vec![],
            };
            el("pre", vec![], vec![el("code", attrs, vec![text(body)])])
        }
        Block::Table { head, rows } => {
            let cells = |tag: &'static str, row: &[Vec<Inline>]| -> Node {
                el(
                    "tr",
                    vec![],
                    row.iter()
                        .map(|cell| el(tag, vec![], render_inlines(cell, depth)))
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

/// `depth` はこのインラインが並ぶ列の深さ。強調・リンクの子は 1 段深くする。
fn render_inline(inline: &Inline, depth: usize) -> Node {
    match inline {
        Inline::Text { text: t } => text(t),
        Inline::Code { text: t } => el("code", vec![], vec![text(t)]),
        Inline::Emph { children } => el("em", vec![], render_inlines(children, depth + 1)),
        Inline::Strong { children } => el("strong", vec![], render_inlines(children, depth + 1)),
        Inline::Link { href, children } => {
            if is_allowed_link_href(href) {
                // target は付けない。別タブを開かず、opener も渡さない。
                el(
                    "a",
                    vec![("href", href), ("rel", "noopener noreferrer")],
                    render_inlines(children, depth + 1),
                )
            } else {
                el("span", vec![], render_inlines(children, depth + 1))
            }
        }
        // 相対のパスだけ img にする。それ以外は要素も通信も作らず、代替の文字だけを出す（src は出さない）。
        Inline::Image { src, alt } => {
            if is_allowed_image_src(src) {
                el("img", vec![("src", src), ("alt", alt)], vec![])
            } else {
                el("span", vec![("data-image-blocked", "1")], vec![text(alt)])
            }
        }
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

    #[test]
    fn 許可される画像の_src() {
        for h in [
            "./img/a.png",
            "/img/a.png",
            "img/a.png",
            "../a.png",
            "/a:b.png",
            "a.png?x=1",
        ] {
            assert!(is_allowed_image_src(h), "{h:?}");
        }
    }

    #[test]
    fn 拒否される画像の_src() {
        for h in [
            "https://example.invalid/a.png",
            "http://x/a.png",
            "HTTPS://X/a.png",
            "//example.invalid/a.png",
            "\\\\example.invalid\\a.png",
            "/\\example.invalid/a.png",
            "\t//example.invalid/a.png",
            "data:image/png;base64,AAAA",
            "javascript:x",
            "java\tscript:x",
            "  javascript:x",
            "\u{1}javascript:x",
            "",
            "   ",
        ] {
            assert!(!is_allowed_image_src(h), "{h:?}");
        }
    }

    #[test]
    fn 表示基盤の検査は_スキーム相対の画像も通す() {
        assert!(fandhe_frontend_core::is_safe_url("//example.invalid/a.png"));
        assert!(!is_allowed_image_src("//example.invalid/a.png"));
    }
}
