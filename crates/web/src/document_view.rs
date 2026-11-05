//! 左の成果物エリアの文書のビュー、変種 (b)（TASK-476 1/2。ART-1・ART-7）。
//!
//! 製品の左エリアの差し替え口 `product_items`（TASK-799）は `engine_parts` から呼ばれ、世界状態の経路ごとの行（`doc_element_item`。
//! 題・状態・補足の 3 つの `span`）を返す。ブロックの行の部品（`doc_item`・`doc_items`・変種 (a)）は製品からは呼ばれない。
//!
//! 文書の行の中身は、ビルド時に安全な木へ変えた Markdown（`contract::doc::Block`）である。このモジュールは
//! その木を、`doc_view::render_block_node`（TASK-149・TASK-150 のスキームの許可・外部画像の拒否・深さの上限）で描き、
//! keyed list の項目 1 件にする。Markdown を実行時に解釈する処理は持たない（方式 (c)。web の通常の依存に
//! Markdown の解釈器は無く、`tests/no_runtime_markdown.rs` が機械で確かめる）。
//!
//! ブロックの行の呼び出し元は、検査器の代役の部品（`view_fixture`）と試験だけである。製品へは結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から `Vec<Block>` を読む規則も未決のためである。
//! 変種 (a) は製品の契約が拒否する（`ViewKind::allows`）。(a) は下の「変種 (a)」の対照としてだけ持つ（TASK-476 2/2）。
//!
//! # 固定するもの
//!
//! - 項目の形は、ブロックの種類・状態・前後・印のどれでも変わらない。根の子は、記号・文字（変化の印）と、
//!   `data-testid="doc-block"` の包みの 3 つだけで、包みの中にブロックの要素が 1 つ入る。
//!   keyed list が同じ key の項目を in-place に更新する前提である（ART3-4 の延長）
//! - `depth` は `None` で渡す。`marked_item` が深さのクラスを根に足すため、呼び出し側が `class` を渡すと重複して
//!   a11y の違反になる。入れ子の字下げは木の要素（`ul`・`blockquote` など）が持つ
//! - `role`・`aria-*` は付けない。指せる行の属性（`data-row`・`data-action`・`data-payload`・`tabindex`）は `pointed_item` で、宛先のある行にだけ付ける（TASK-480 1/2）。
//!   宛先は呼び出し側が注入する。行の識別子（文書の経路 + `#` + 位置）と `Input::Ui.path` の対応は固定しない（spec 側の確認事項。`docs/testing.md` の 5.111）
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である
//! - 経路ごとの行（製品）の根の子は、変化の印、題・状態・補足の `span`（値が空でも必ず出す）、本文の区画（条件つき）の順で、
//!   `data-kind="item"` を持つ。`state` は閉じた語彙に読まず文字のまま出す
//!
//! # 変種 (a)（対照専用）
//!
//! `doc_item_a`・`doc_items_a` は、`wireframe-ui` の `text`・`paragraph` でブロックを平文の行に落とす方式の対照である。
//! 製品では選べない（`ViewKind::allows` が拒否する）。呼び出し元は、代役の部品（`view_fixture`）の対照の口と統合試験だけである。
//! 観察として固定するのは、表・引用・一覧が構造を失うこと（見出し・表・一覧・引用・コード・リンクの要素が出ず、
//! 文字だけが段落の行になる）である。見出しと表の見出し行は `text`（大・太字）、それ以外は `paragraph` で描く。
//! 入れ子をたどる再帰は `MAX_RENDER_DEPTH` で止め、省略の文言の行にする。信頼境界は (b) と同じで、文字は
//! `text()` の文字ノードにだけ入り、属性には入らない（リンクの宛先・画像の URL は捨てる）。
//!
//! # 信頼境界
//!
//! ブロックの中身は教材由来の信頼できない値である。文字は `doc_view` を通って `text()` の文字ノードにだけ入り、
//! `render` の既定のエスケープを通る（SAFE-11）。タグ・属性名・クラス・testid・`data-block` の値は、
//! 閉じた集合の `&'static str` だけである。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_wireframe_ui::{paragraph as wire_paragraph, text as wire_text, Bold, Size};
use fandhe_workshop_contract::doc::{Block, Doc, Inline};
use fandhe_workshop_contract::view_body::body_doc;
use fandhe_workshop_core::world::Element;

use crate::a11y::{ChangeMark, TESTID_ATTR};
use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item, PointOf};
use crate::artifact_view::{
    list_attrs, view_list, DocRow, Phase, PointTarget, RowId, RowKind, Variant, ViewKind,
};
use crate::doc_view::{render_block_node, render_doc, DEPTH_OMITTED_TEXT, MAX_RENDER_DEPTH};
use crate::list_policy::ListError;
use crate::product_rows::{
    read_cell, ProductCtx, ProductItems, ARTIFACT_KEY_PATH_MAX, DOC_BODY_TESTID,
};

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
pub fn doc_item(row: &DocRow<Block>, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
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
    Some(pointed_item(
        "li",
        Variant::B,
        shown.mark,
        None,
        vec![("data-kind", row.kind().as_str())],
        target,
        content,
    ))
}

/// 行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn doc_items(rows: &[DocRow<Block>], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            doc_item(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// 平文の行 1 本。`true` なら見出しの行（`text` の大・太字）、`false` なら段落の行（`paragraph`）。
type Line = (bool, String);

/// インラインの文字だけを `out` へ足す（構造・リンクの宛先・画像の URL は捨てる）。深さの上限で打ち切る。
fn inline_text(inlines: &[Inline], depth: usize, out: &mut String) {
    if depth >= MAX_RENDER_DEPTH {
        out.push_str(DEPTH_OMITTED_TEXT);
        return;
    }
    for inline in inlines {
        match inline {
            Inline::Text { text } | Inline::Code { text } => out.push_str(text),
            Inline::Emph { children }
            | Inline::Strong { children }
            | Inline::Link { children, .. } => inline_text(children, depth + 1, out),
            Inline::Image { alt, .. } => out.push_str(alt),
            Inline::Break => out.push(' '),
        }
    }
}

fn inlines_string(inlines: &[Inline], depth: usize) -> String {
    let mut s = String::new();
    inline_text(inlines, depth, &mut s);
    s
}

/// ブロックを平文の行へ落とす。引用・一覧は子を順に平文化し、入れ子の構造は捨てる。
fn block_lines(block: &Block, depth: usize, out: &mut Vec<Line>) {
    if depth >= MAX_RENDER_DEPTH {
        out.push((false, DEPTH_OMITTED_TEXT.to_owned()));
        return;
    }
    match block {
        Block::Heading { children, .. } => out.push((true, inlines_string(children, depth + 1))),
        Block::Paragraph { children } => out.push((false, inlines_string(children, depth + 1))),
        Block::Quote { children } => {
            for child in children {
                block_lines(child, depth + 1, out);
            }
        }
        Block::List { items, .. } => {
            for item in items {
                for child in item {
                    block_lines(child, depth + 1, out);
                }
            }
        }
        Block::Code { text, .. } => out.push((false, text.clone())),
        Block::Table { head, rows } => {
            let row_line = |cells: &[Vec<Inline>]| -> String {
                cells
                    .iter()
                    .map(|c| inlines_string(c, depth + 1))
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            if !head.is_empty() {
                out.push((true, row_line(head)));
            }
            for row in rows {
                out.push((false, row_line(row)));
            }
        }
        Block::Rule => {}
    }
}

fn flatten_lines(lines: &[Line]) -> Vec<Node> {
    lines
        .iter()
        .map(|(heading, s)| {
            if *heading {
                wire_text(s, Size::Lg, Bold(true))
            } else {
                wire_paragraph(s, Size::Md, Bold(false))
            }
        })
        .collect()
}

/// 変種 (a) の行 1 件を作る（対照専用。製品の構成では選べない）。その状態で出ない行は `None`。
#[must_use]
pub fn doc_item_a(row: &DocRow<Block>, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    let shown = row.shown(phase)?;
    let block = shown.content;
    let mut lines = Vec::new();
    block_lines(block, 0, &mut lines);
    let content = vec![el(
        "div",
        vec![
            (TESTID_ATTR, DOC_BLOCK_TESTID),
            ("class", DC_BLOCK_CLASS),
            ("data-block", block_kind(block)),
        ],
        flatten_lines(&lines),
    )];
    Some(pointed_item(
        "li",
        Variant::A,
        shown.mark,
        None,
        vec![("data-kind", row.kind().as_str())],
        target,
        content,
    ))
}

/// 変種 (a) の行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す（対照専用）。
#[must_use]
pub fn doc_items_a(rows: &[DocRow<Block>], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            doc_item_a(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
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

/// 経路ごとの行の題の `span` の `data-testid`。
pub const DOC_ROW_TITLE_TESTID: &str = "doc-row-title";
/// 経路ごとの行の状態の `span` の `data-testid`。
pub const DOC_ROW_STATE_TESTID: &str = "doc-row-state";
/// 経路ごとの行の補足の `span` の `data-testid`。
pub const DOC_ROW_DETAIL_TESTID: &str = "doc-row-detail";
/// 題の `span` のクラス。
pub const DR_TITLE_CLASS: &str = "dr-title";
/// 状態の `span` のクラス。
pub const DR_STATE_CLASS: &str = "dr-state";
/// 補足の `span` のクラス。
pub const DR_DETAIL_CLASS: &str = "dr-detail";

/// 経路ごとの行 1 件を作る（製品の差し替え口 `product_items` が呼ぶ）。
///
/// 根の子は、変化の印（`pointed_item` が付ける 2 つ）、題・状態・補足の `span`、本文の区画（条件つき）の順である。
/// 3 つの `span` は値が空でも必ず出し、印・前後で子の形を変えない。属性の値は教材由来で、`text()` の文字ノードにだけ入る。
#[must_use]
pub fn doc_element_item(
    path: &str,
    element: Option<&Element>,
    mark: Option<ChangeMark>,
    target: Option<&PointTarget>,
    body: Option<&Doc>,
) -> Node {
    let cell = read_cell(path, element);
    let span = |testid: &'static str, class: &'static str, value: &str| {
        el(
            "span",
            vec![(TESTID_ATTR, testid), ("class", class)],
            vec![text(value)],
        )
    };
    let mut kids = vec![
        span(DOC_ROW_TITLE_TESTID, DR_TITLE_CLASS, &cell.label),
        span(DOC_ROW_STATE_TESTID, DR_STATE_CLASS, &cell.state),
        span(DOC_ROW_DETAIL_TESTID, DR_DETAIL_CLASS, &cell.detail),
    ];
    if let Some(doc) = body {
        kids.push(el(
            "div",
            vec![
                (TESTID_ATTR, DOC_BODY_TESTID),
                ("class", "artifact-doc-body"),
            ],
            render_doc(doc),
        ));
    }
    let variant = Variant::B;
    pointed_item(
        "li",
        variant,
        mark,
        None,
        vec![("data-path", path), ("data-kind", RowKind::Item.as_str())],
        target,
        kids,
    )
}

/// 製品の左エリアからの差し替え口。世界状態の経路ごとに 1 行を返し、行に題・状態・補足を出す。
///
/// `engine_parts::ArtifactPart::view` の振り分けだけが呼ぶ。宛先・キー・固定の規則は
/// `product_rows::path_rows` と同じ（TASK-358 2/2・ART-18。上限の判断 2026-10-24）で、行の中身と本文の区画（文書のビューだけが持つ）が違う。
pub(crate) fn product_items(ctx: &ProductCtx<'_>) -> ProductItems {
    let rows: Vec<ArtifactRow> = ctx
        .world_rows
        .iter()
        .enumerate()
        .map(|(i, (path, mark))| {
            let target = ctx
                .declared_id
                .and_then(|id| point_all(id)(&RowId::new(path.as_str()).ok()?));
            let element = ctx.world.get(path);
            let body = ctx
                .declared_id
                .zip(ctx.session)
                .zip(element)
                .and_then(|((id, s), e)| body_doc(s.ir(), id, e.fixture.as_deref()));
            let node = doc_element_item(path, element, *mark, target.as_ref(), body);
            let key = if path.len() <= ARTIFACT_KEY_PATH_MAX {
                format!("p{path}")
            } else {
                format!("i{i}")
            };
            let pinned = target.is_some() && ctx.pinned_paths.contains(path.as_str());
            (path.clone(), key, node, pinned)
        })
        .collect();
    ProductItems {
        rows,
        list_attrs: list_attrs(ViewKind::Document, true),
    }
}
