//! 左の成果物エリアのファイルツリーのビュー、変種 (b)（TASK-473 1/2）と変種 (a)（TASK-473 2/2）。ART-1・ART-2・ART-3・ART-9・ART-13。
//!
//! 製品の左エリアの差し替え口 `product_items`（TASK-799）は `engine_parts` から呼ばれ、(b) の部品（ラベル・補足・木の属性）で行を描く。
//! 行の種類は、ほかの行の経路が「この経路 + `/`」で始まれば `dir`、そうでなければ `file`（途中のディレクトリの行は合成しない）。
//! 深さは経路の `/` の数、キーは `p<経路>`（長い経路は `i<位置>`）で、`li` に `data-path` を付ける。親の `role=tree` は
//! `tree_list_attrs` の条件（選択中・項目 1 件以上）に従う。
//!
//! 行数が `LIST_ITEMS_TARGET` を超えて畳み（`fold_rows`）が働くときは、要約の行が木の項目でなく `aria-posinset` などが
//! 見えている行と食い違うため、親の `role=tree` と項目の木の属性を同じ判定で両方外す（fail-closed。`task_tree` と同じ）。
//! 行の中身・`data-path`・`data-kind` は残す。
//!
//! 項目の根は `artifact_mark::pointed_item`（`marked_item` に指せる行の 4 属性を足す）で作る。宛先は呼び出し側が `PointOf` で注入し、
//! 宛先の無い行には 4 属性が付かない（TASK-480 1/2。不変条件は `artifact_mark` の doc）。
//!
//! 変種 (a) と前後の切り替え（ART-2）は製品から呼ばれない。`view_fixture`（試験の設定だけ）の検査器の枠が (a)・(b) の
//! 本体を通る。製品には前後の切り替えの口が無い（TASK-470・TASK-347 の後続）。変種 (a) は `wireframe-ui` の `rich_text` で
//! 描く（下の「変種 (a)」）。
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
//! 製品が足す `data-path` は経路の値で、属性の値として `render` のエスケープを通る（属性名は固定）。
//! 属性名・クラス名・`data-kind` の値は閉じた集合の `&'static str` だけである。`TreeItemProps` の `value` などには
//! 教材由来の値を渡さず（`item_text` は使わないが）、固定の空文字列にして `data-value` の経路を作らない。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_headless_ui::tree_view::{item_text, TreeItemProps};
use fandhe_frontend_wireframe_ui::{rich_text, Bold, Orientation, Size};

use std::collections::HashSet;

use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item, PointOf};
use crate::artifact_view::{
    tree_list_attrs, tree_view_list, ItemRow, Phase, PointTarget, RowKind, Variant, ViewKind,
};
use crate::list_policy::{ListError, LIST_ITEMS_TARGET};
use crate::product_rows::{
    item_rows, path_dfs_order, path_rows, path_tree_attrs, ProductCtx, ProductItems,
    ARTIFACT_KEY_PATH_MAX,
};
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
    file_tree_item_with(row, phase, tree, Vec::new(), target)
}

/// [`file_tree_item`] に、属性の先頭へ置く製品の属性 `lead`（`data-path`）を足した形。`lead` が空なら同じ出力になる。
///
/// 製品の差し替え口 `product_items` が呼ぶ。属性の順は `lead`、木の属性、`data-kind` である。
#[must_use]
pub fn file_tree_item_with(
    row: &ItemRow,
    phase: Phase,
    tree: Option<&TreeItemAttrs>,
    lead: Vec<(&'static str, &str)>,
    target: Option<&PointTarget>,
) -> Option<Node> {
    let shown = row.shown(phase)?;
    let mut attrs = tree.map(TreeItemAttrs::pairs).unwrap_or_default();
    attrs.splice(0..0, lead);
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

/// 製品の左エリアからの差し替え口。(b) の部品で行を描く。
///
/// `engine_parts::ArtifactPart::view` の振り分けだけが呼ぶ。行の検査に通らない経路（空・重複）や、出ない行が
/// あるときは行を黙って落とさず、共通の経路の行に退く。畳みが働く行数の扱いは冒頭の説明を参照。
pub(crate) fn product_items(ctx: &ProductCtx<'_>) -> ProductItems {
    let order = path_dfs_order(
        &ctx.world_rows
            .iter()
            .map(|(p, _)| p.as_str())
            .collect::<Vec<_>>(),
    );
    // 「ほかの行の経路が `この経路 + /` で始まる」経路の集合。各経路の `/` の境界で切った接頭辞を集めて O(総文字数) で求める。
    let mut dirs: HashSet<&str> = HashSet::new();
    for (p, _) in ctx.world_rows {
        for (cut, _) in p.match_indices('/') {
            dirs.insert(&p[..cut]);
        }
    }
    let world_rows: Vec<(String, Option<crate::a11y::ChangeMark>)> =
        order.iter().map(|&i| ctx.world_rows[i].clone()).collect();
    let Ok(rows) = item_rows(ctx.world, &world_rows, |p| {
        if dirs.contains(p) {
            RowKind::Dir
        } else {
            RowKind::File
        }
    }) else {
        return path_rows(ctx);
    };
    let folded = rows.len() > LIST_ITEMS_TARGET;
    let attrs: Vec<Option<TreeItemAttrs>> = if folded {
        vec![None; rows.len()]
    } else {
        path_tree_attrs(world_rows.iter().map(|(p, _)| p.as_str()))
    };
    let mut out: Vec<ArtifactRow> = Vec::with_capacity(rows.len());
    for (i, ((path, _), (row, tree))) in world_rows
        .iter()
        .zip(rows.iter().zip(attrs.iter()))
        .enumerate()
    {
        let target = ctx.declared_id.and_then(|id| point_all(id)(row.id()));
        let Some(node) = file_tree_item_with(
            row,
            Phase::After,
            tree.as_ref(),
            vec![("data-path", path.as_str())],
            target.as_ref(),
        ) else {
            return path_rows(ctx);
        };
        let key = if path.len() <= ARTIFACT_KEY_PATH_MAX {
            format!("p{path}")
        } else {
            format!("i{i}")
        };
        let pinned = target.is_some() && ctx.pinned_paths.contains(path.as_str());
        out.push((path.clone(), key, node, pinned));
    }
    let has_tree = !out.is_empty() && !folded;
    let list_attrs = tree_list_attrs(ViewKind::FileTree, Variant::B, true, has_tree);
    ProductItems {
        rows: out,
        list_attrs,
    }
}
