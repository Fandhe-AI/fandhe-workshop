//! 左の成果物エリアのファイルツリーのビュー、変種 (b)（TASK-473 1/2）と変種 (a)（TASK-473 2/2）。ART-1・ART-2・ART-3・ART-9・ART-13。
//!
//! 項目の根は `artifact_mark::pointed_item`（`marked_item` に指せる行の 4 属性を足す）で作る。宛先は呼び出し側が `PointOf` で注入し、
//! 宛先の無い行には 4 属性が付かない（TASK-480 1/2。不変条件は `artifact_mark` の doc）。
//!
//! 呼び出し元は、今は検査器の代役の部品（`view_fixture`）だけである。製品の `view()`（`engine_parts::ArtifactPart`）へは
//! まだ結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から `Row<Cell>` を作る規則（前の取り直し）も
//! 未決のためである（TASK-470・TASK-347 の後続）。変種 (a) は `wireframe-ui` の `rich_text` で描く（下の「変種 (a)」）。
//!
//! 行は `artifact_mark::marked_item` を根にし、`tree_attrs` の木の属性と、`headless-ui` の `tree_view::item_text`
//! のラベルを持つ。親の `role=tree` は `artifact_view::tree_view_list` が付ける。
//!
//! # headless-ui の `tree_view` から `item_text` だけを採る理由
//!
//! 次の 5 点が a11y の契約（版 2）と TASK-469・TASK-472 の規約に反するためである。
//!
//! - `item`・`branch` は `aria-selected` を必ず出す。a11y は tab 以外の `aria-selected` を違反にし、呼び出し側から消せない
//! - `*_indicator` は `aria-hidden`、`branch_content` は `role=group` を出す。どちらも閉じた一覧に無い
//! - `item`・`branch` の根は `div` で、`ul` の子として不正であり、`marked_item` を根にする規約にも反する
//! - `branch` は開いた行すべてに `aria-expanded` を付ける。TASK-472 は子を持つ行にだけ付ける
//! - `data-value` に行の識別子（教材由来）が入る。教材由来の値が入る属性は `data-row`・`data-payload` だけと定めている
//!
//! 採る部品を広げるには、spec の更新と a11y の契約の版上げが先に要る。
//!
//! # 固定するもの
//!
//! - 項目の形（タグ・属性名・子の数と種別）は、前後の状態でも印の値でも変わらない（ART3-4 の延長）。補足の `span` は
//!   `detail` が空でも必ず出し、ラベルはディレクトリでもファイルでも `item_text` にそろえる。種類は `data-kind` で示す
//! - `aria-expanded`・`aria-setsize` は前後で変わりうるが、属性だけの変化で子の列は変わらないため in-place に更新される
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である
//! - 計算量は行数に対して O(n)
//!
//! # 変種 (a)
//!
//! 外枠は `marked_item(.., Variant::A, ..)` の `li` と内側の `div`（印の宿主）で、中身は `wireframe_ui::rich_text`
//! （ラベル・先頭の記号・末尾の補足）である。(b) の関数には触れない。
//!
//! - 末尾の補足の `span` は `detail` が空でも必ず出す。省くと前後で子の列が変わり、keyed list の in-place 更新が崩れる
//! - 先頭の `span` と太字は `row.kind()` だけで決め、前後や印では変えない。同じ key の項目の形は前後で変わらない
//! - 先頭に `wireframe_ui::icon::*` は使わない。`svg` に `aria-hidden` を必ず出し、a11y の契約（版 2）の違反になる。
//!   採るには契約の版 3 と spec の更新が先に要る。記号は自作の `span` の文字（`mark_nodes` の記号と同じ扱い）にする
//! - 木の属性（`role=treeitem`・`aria-level` など）は付けない。(a) は木の意味を持たない
//! - `wireframe_css()` は呼ばない。固定のライト配色の CSS の配信は TASK-490 の担当である
//!
//! # 信頼境界
//!
//! ラベル・補足は `text()` の文字のノードにだけ入れ、`render` の既定のエスケープを通す（SAFE-11）。
//! 属性名・クラス名・`data-kind` の値は閉じた集合の `&'static str` だけである。`TreeItemProps` の `value` などには
//! 教材由来の値を渡さず（`item_text` は使わないが）、固定の空文字列にして `data-value` の経路を作らない。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_headless_ui::tree_view::{item_text, TreeItemProps};
use fandhe_frontend_wireframe_ui::{rich_text, Bold, Orientation, Size};

use crate::artifact_mark::{pointed_item, PointOf};
use crate::artifact_view::{
    tree_view_list, ItemRow, Phase, PointTarget, RowKind, Variant, ViewKind,
};
use crate::list_policy::ListError;
use crate::tree_attrs::{shown_tree_attrs, TreeItemAttrs};

/// 補足の `span` のクラス。スタイルは持たない（見た目は TASK-490）。
pub const DETAIL_CLASS: &str = "detail";

/// 変種 (a) の先頭の記号の `span` のクラス。スタイルは持たない（見た目は TASK-490）。
pub const ICON_CLASS: &str = "icon";

/// ディレクトリの先頭の記号（開いた形）。ファイルは空文字だが、`span` は必ず出して形をそろえる。
const DIR_GLYPH: &str = "\u{25be}";

/// 教材由来の値を持たない、`item_text` に渡す固定の props。
const FIXED_PROPS: TreeItemProps<'static> = TreeItemProps {
    value: "",
    selected: false,
    disabled: false,
    level: "",
    posinset: "",
    setsize: "",
    depth: "",
};

/// 行 1 件を作る。その状態で出ない行は `None`。`tree` は [`shown_tree_attrs`] の、この行の結果である。
#[must_use]
pub fn file_tree_item(
    row: &ItemRow,
    phase: Phase,
    tree: Option<&TreeItemAttrs>,
    target: Option<&PointTarget>,
) -> Option<Node> {
    let shown = row.shown(phase)?;
    let mut attrs = tree.map(TreeItemAttrs::pairs).unwrap_or_default();
    attrs.push(("data-kind", row.kind().as_str()));
    let label = item_text(
        FIXED_PROPS,
        vec![],
        vec![text(shown.content.label.as_str())],
    );
    let detail = el(
        "span",
        vec![("class", DETAIL_CLASS)],
        vec![text(shown.content.detail.as_str())],
    );
    let content = vec![label, detail];
    Some(pointed_item(
        "li",
        Variant::B,
        shown.mark,
        Some(row.depth()),
        attrs,
        target,
        content,
    ))
}

/// 行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn file_tree_items(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    let attrs = shown_tree_attrs(rows, phase);
    rows.iter()
        .zip(attrs.iter())
        .filter_map(|(row, tree)| {
            file_tree_item(row, phase, tree.as_ref(), point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// ファイルツリー (b) の keyed list の親要素を作る。選ばれていなければ空の一覧になる。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn file_tree_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    tree_view_list(tag, ViewKind::FileTree, Variant::B, selected, items)
}

/// 変種 (a) の行 1 件を作る。その状態で出ない行は `None`。木の属性は付けない。
#[must_use]
pub fn file_tree_item_a(row: &ItemRow, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    let shown = row.shown(phase)?;
    let is_dir = row.kind() == RowKind::Dir;
    let leading = el(
        "span",
        vec![("class", ICON_CLASS)],
        vec![text(if is_dir { DIR_GLYPH } else { "" })],
    );
    let trailing = el(
        "span",
        vec![("class", DETAIL_CLASS)],
        vec![text(shown.content.detail.as_str())],
    );
    let rich = rich_text(
        shown.content.label.as_str(),
        Some(leading),
        Some(trailing),
        Size::Md,
        Bold(is_dir),
        Orientation::Horizontal,
    );
    Some(pointed_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        target,
        vec![rich],
    ))
}

/// 行の並びから、変種 (a) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn file_tree_items_a(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            file_tree_item_a(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// ファイルツリー (a) の keyed list の親要素を作る。親に `role` は付かない。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn file_tree_list_a(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    tree_view_list(tag, ViewKind::FileTree, Variant::A, selected, items)
}
