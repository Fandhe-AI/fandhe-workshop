//! 左の成果物エリアの文書のビュー、変種 (b)（TASK-476 1/2。ART-1・ART-7）。
//!
//! 文書の行の中身は、ビルド時に安全な木へ変えた Markdown（`contract::doc::Block`）である。このモジュールは
//! その木を、`doc_view::render_block_node`（TASK-149・TASK-150 のスキームの許可・外部画像の拒否・深さの上限）で描き、
//! keyed list の項目 1 件にする。Markdown を実行時に解釈する処理は持たない（方式 (c)。web の通常の依存に
//! Markdown の解釈器は無く、`tests/no_runtime_markdown.rs` が機械で確かめる）。
//!
//! 呼び出し元は、今は検査器の代役の部品（`view_fixture`）だけである。製品の `view()`（`engine_parts::ArtifactPart`）へは
//! まだ結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から `Vec<Block>` を読む規則も未決のためである。
//! 変種 (a) は製品の契約が拒否する（`ViewKind::allows`）。(a) を対照として固定するのは TASK-476 2/2 の担当である。
//!
//! # 固定するもの
//!
//! - 項目の形は、ブロックの種類・状態・前後・印のどれでも変わらない。根の子は、記号・文字（変化の印）と、
//!   `data-testid="doc-block"` の包みの 3 つだけで、包みの中にブロックの要素が 1 つ入る。
//!   keyed list が同じ key の項目を in-place に更新する前提である（ART3-4 の延長）
//! - `depth` は `None` で渡す。`marked_item` が深さのクラスを根に足すため、呼び出し側が `class` を渡すと重複して
//!   a11y の違反になる。入れ子の字下げは木の要素（`ul`・`blockquote` など）が持つ
//! - `role`・`aria-*` は付けない。指せる行の属性（`data-row`・`data-action`・`tabindex`）も付けない（TASK-480）
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である
//!
//! # 信頼境界
//!
//! ブロックの中身は教材由来の信頼できない値である。文字は `doc_view` を通って `text()` の文字ノードにだけ入り、
//! `render` の既定のエスケープを通る（SAFE-11）。タグ・属性名・クラス・testid・`data-block` の値は、
//! 閉じた集合の `&'static str` だけである。

use fandhe_frontend_core::{el, Node};
use fandhe_workshop_contract::doc::Block;

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{view_list, DocRow, Phase, Variant, ViewKind};
use crate::doc_view::render_block_node;
use crate::list_policy::ListError;

/// ブロックの包みの `data-testid`。
pub const DOC_BLOCK_TESTID: &str = "doc-block";
/// ブロックの包みのクラス。`static/app.css` が配置を定義する。
pub const DC_BLOCK_CLASS: &str = "dc-block";

/// ブロックの種類の名前。`data-block` の値で、閉じた集合の静的な文字列だけを返す。
#[must_use]
pub fn block_kind(block: &Block) -> &'static str {
    match block {
        Block::Heading { .. } => "heading",
        Block::Paragraph { .. } => "paragraph",
        Block::Quote { .. } => "quote",
        Block::List { .. } => "list",
        Block::Code { .. } => "code",
        Block::Table { .. } => "table",
        Block::Rule => "rule",
    }
}

/// 行 1 件を作る。その状態で出ない行は `None`。
#[must_use]
pub fn doc_item(row: &DocRow<Block>, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    let block = shown.content;
    let content = vec![el(
        "div",
        vec![
            (TESTID_ATTR, DOC_BLOCK_TESTID),
            ("class", DC_BLOCK_CLASS),
            ("data-block", block_kind(block)),
        ],
        vec![render_block_node(block)],
    )];
    Some(marked_item(
        "li",
        Variant::B,
        shown.mark,
        None,
        vec![("data-kind", row.kind().as_str())],
        content,
    ))
}

/// 行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn doc_items(rows: &[DocRow<Block>], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| doc_item(row, phase).map(|n| (format!("k{}", row.id().as_str()), n)))
        .collect()
}

/// 文書の keyed list の親要素を作る。選ばれていなければ空の一覧になる。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn doc_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    view_list(tag, ViewKind::Document, selected, items)
}
