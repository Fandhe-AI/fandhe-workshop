//! タスクツリーの変種 (a)・(b) の本体（TASK-474。ART-1・ART-9・ART-13・A11Y-22・PoC-8）。
//!
//! 製品の左エリアの差し替え口 `product_items`（TASK-799）は `engine_parts` から呼ばれ、(b) の部品（ラベル・補足・状態のバッジ・
//! 木の属性）で行を描く。行の種類は `task`（`data-kind`）、深さは経路の `/` の数、キーは `p<経路>`（長い経路は `i<位置>`）で、
//! `li` に `data-path` を付ける。親の `role=tree` は `tree_list_attrs` の条件（選択中・項目 1 件以上）に従う。
//! 状態は `TaskState::parse` で読む。語彙の外の空でない値は、教材の語のまま文字で出す（記号は `◇`。D-101）。空・空白だけは「不明」（`?`）になる（語彙は spec の判断。記録は #2059）。
//!
//! # 畳みが働く行数の扱い（fail-closed）
//!
//! 行数が `LIST_ITEMS_TARGET` を超えると `fold_rows` が古い側を要約の行に畳む。要約の行は木の項目ではなく、
//! 畳む前の全行で数えた `aria-posinset`・`aria-setsize`・`aria-expanded` は見えている行と食い違う。
//! a11y の検査はこの食い違いを見つけないため、畳みが働くときは親の `role=tree` と項目の木の属性を同じ判定で両方外す
//! （親だけ外すと `treeitem` が木の外に出る）。行の中身・`data-path`・`data-kind` は残す。
//!
//! (a) は wireframe 系のチェックボックス（完了をチェック済みで示す）・状態の文字・入れ子のクラスで描き、木の意味
//! （`role=tree`・`treeitem`）を持たない。(b) は木の意味を持つ。
//!
//! (a) と前後の切り替え（ART-2）は製品から呼ばれない。`view_fixture`（試験の設定だけ）の検査器の枠が (a)・(b) の本体を通る。
//!
//! # 信頼境界
//!
//! `Cell` の値は教材由来で信頼できない。ラベルと詳細は文字ノード（`text()`）にだけ入れ、`render` の既定の
//! エスケープを通す。状態は完全一致で閉じた [`TaskState`] に読み、語彙の語の記号と文字は固定の `&'static str` である。
//! 未知の値は既知の状態へ寄せず [`TaskState::Unknown`] にする（fail-closed）。語彙の外の空でない値は
//! 状態の文字の文字ノードにだけ入れ、属性・クラス・testid・`data-checked` には入れない。空白だけの値は空と同じく固定の
//! 「不明」にする（文字が見た目の上で空になるのを避ける）。
//!
//! # 固定するもの
//!
//! - 状態の語彙（`open`・`progress`・`done`）は spec に定めが無く、試験データ由来の仮置きである。
//! - 項目の形（タグと子の数）は、状態・印・前後で変えない。keyed list が同じ key の項目を in-place に更新する
//!   条件は形が同じことだからである。`tags` は表示しない（件数で子が増え、形が変わるため）。
//! - (a) のチェックは自作の `span` で、子は文字ノード 1 つに固定する（完了だけ `✓`、他は空文字。`Unknown` も
//!   チェックなしで、完了と主張しない）。`fandhe-frontend-wireframe-ui` の `checkbox` は、チェックの有無で子の数が
//!   変わることと、グリフの `aria-hidden` が a11y 契約の閉じた一覧に無いことから使わない。
//! - 木の位置は描画 1 回につき 1 回、O(n) で求める。
//! - `data-row` 等の指せる行の属性は `pointed_item` で、宛先のある行にだけ付ける（TASK-480 1/2）。宛先は呼び出し側が注入する。

use crate::list_policy::ListError;
use fandhe_frontend_core::{el, text, Node};

use crate::a11y::TESTID_ATTR;
use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item, PointOf};
use crate::artifact_view::{
    other_state_word, tree_list_attrs, tree_view_list, Cell, ItemRow, Phase, PointTarget, RowKind,
    Variant, ViewKind, OTHER_STATE_GLYPH,
};
use crate::list_policy::LIST_ITEMS_TARGET;
use crate::product_rows::{
    item_rows, path_dfs_order, path_rows, path_tree_attrs, ProductCtx, ProductItems,
    ARTIFACT_KEY_PATH_MAX,
};
use crate::tree_attrs::{shown_tree_attrs, TreeItemAttrs};

/// ラベルの `data-testid`。
pub const TASK_LABEL_TESTID: &str = "task-label";
/// 詳細の `data-testid`。
pub const TASK_DETAIL_TESTID: &str = "task-detail";
/// 状態のバッジの `data-testid`。
pub const STATE_BADGE_TESTID: &str = "state-badge";
/// 状態の記号の `data-testid`。印の記号（`mark-glyph`）とは別にする。
pub const STATE_GLYPH_TESTID: &str = "state-glyph";
/// 状態の文字の `data-testid`。印の文字（`mark-text`）とは別にする。
pub const STATE_TEXT_TESTID: &str = "state-text";

/// (a) のチェックの `data-testid`。
pub const TASK_CHECK_TESTID: &str = "task-check";
/// (a) のチェックのクラス。見た目は `app.css` が決める。
pub const TASK_CHECK_CLASS: &str = "tk-check";
/// チェック済みの印。
const CHECK_GLYPH: &str = "✓";

/// タスクの状態。閉じた集合で、色だけに頼らず記号と文字の両方で示す（A11Y-22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 未着手。
    Open,
    /// 進行中。
    InProgress,
    /// 完了。
    Done,
    /// 語彙の外の値と空の値。教材の語をそのまま出すかは、バッジの組み立て側（[`state_badge_of`]）で分ける。
    Unknown,
}

impl TaskState {
    /// 全種類。
    pub const ALL: [TaskState; 4] = [
        TaskState::Open,
        TaskState::InProgress,
        TaskState::Done,
        TaskState::Unknown,
    ];

    /// 綴りから読む。完全一致だけで、一覧外は [`TaskState::Unknown`]。
    #[must_use]
    pub fn parse(s: &str) -> TaskState {
        match s {
            "open" => TaskState::Open,
            "progress" => TaskState::InProgress,
            "done" => TaskState::Done,
            _ => TaskState::Unknown,
        }
    }

    /// 状態の記号。印の記号（`+`・`-`・`~`）とは重ならない。
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            TaskState::Open => "○",
            TaskState::InProgress => "◐",
            TaskState::Done => "●",
            TaskState::Unknown => "?",
        }
    }

    /// 状態の文字。
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            TaskState::Open => "未着手",
            TaskState::InProgress => "進行中",
            TaskState::Done => "完了",
            TaskState::Unknown => "不明",
        }
    }
}

/// 状態のバッジ。子は記号と文字の 2 つの `span` に固定し、それぞれ文字ノード 1 つだけを持つ。
#[must_use]
pub fn state_badge(state: TaskState) -> Node {
    badge_of(state.glyph(), state.text())
}

/// 行の生の `state` から作る状態のバッジ。語彙の語は [`state_badge`] と同じ。語彙の外の空でない値は、中立の記号
/// [`OTHER_STATE_GLYPH`] と値の文字のまま出す（D-101）。空・空白だけは固定の「不明」にする。
/// 値は文字ノードにだけ入れ、色調・強調・チェックは閉じた `parse` の結果だけで決まる。
#[must_use]
pub fn state_badge_of(raw: &str) -> Node {
    let state = TaskState::parse(raw);
    if state == TaskState::Unknown {
        if let Some(word) = other_state_word(raw) {
            return badge_of(OTHER_STATE_GLYPH, word);
        }
    }
    state_badge(state)
}

fn badge_of(glyph: &str, word: &str) -> Node {
    el(
        "span",
        vec![(TESTID_ATTR, STATE_BADGE_TESTID)],
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, STATE_GLYPH_TESTID)],
                vec![text(glyph)],
            ),
            el(
                "span",
                vec![(TESTID_ATTR, STATE_TEXT_TESTID)],
                vec![text(word)],
            ),
        ],
    )
}

/// (a) のチェックボックス。完了だけチェック済みにする。子は文字ノード 1 つで、形は状態で変えない。
#[must_use]
pub fn task_check(state: TaskState) -> Node {
    let checked = state == TaskState::Done;
    el(
        "span",
        vec![
            (TESTID_ATTR, TASK_CHECK_TESTID),
            ("class", TASK_CHECK_CLASS),
            ("data-checked", if checked { "true" } else { "false" }),
        ],
        vec![text(if checked { CHECK_GLYPH } else { "" })],
    )
}

/// 変種に共通の中身（ラベル・詳細・状態のバッジ）。
fn task_content(cell: &Cell) -> Vec<Node> {
    vec![
        el(
            "span",
            vec![(TESTID_ATTR, TASK_LABEL_TESTID)],
            vec![text(cell.label.as_str())],
        ),
        el(
            "span",
            vec![(TESTID_ATTR, TASK_DETAIL_TESTID)],
            vec![text(cell.detail.as_str())],
        ),
        state_badge_of(&cell.state),
    ]
}

/// その状態で出る行を、key（`k<行の ID>`）つきの (a) の項目にする。木の属性は付けない。
#[must_use]
pub fn task_items_a(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            let shown = row.shown(phase)?;
            let cell = shown.content;
            let mut content = vec![task_check(TaskState::parse(&cell.state))];
            content.extend(task_content(cell));
            let node = pointed_item(
                "li",
                Variant::A,
                shown.mark,
                Some(row.depth()),
                vec![],
                point(row.id()).as_ref(),
                content,
            );
            Some((format!("k{}", row.id().as_str()), node))
        })
        .collect()
}

/// その状態で出る行を、key（`k<行の ID>`）つきの (b) の項目にする。行の並びは入力の順のまま。
#[must_use]
pub fn task_items_b(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    let attrs = shown_tree_attrs(rows, phase);
    rows.iter()
        .zip(attrs)
        .filter_map(|(row, attrs)| {
            let shown = row.shown(phase)?;
            let attrs = attrs?;
            let node = pointed_item(
                "li",
                Variant::B,
                shown.mark,
                Some(row.depth()),
                attrs.pairs(),
                point(row.id()).as_ref(),
                task_content(shown.content),
            );
            Some((format!("k{}", row.id().as_str()), node))
        })
        .collect()
}

/// 変種に応じた項目にする。
#[must_use]
pub fn task_items(
    rows: &[ItemRow],
    phase: Phase,
    variant: Variant,
    point: PointOf,
) -> Vec<(String, Node)> {
    match variant {
        Variant::A => task_items_a(rows, phase, point),
        Variant::B => task_items_b(rows, phase, point),
    }
}

/// タスクツリーの keyed list の親要素を作る。親の属性は `view()` から呼ぶこの関数で書く（FEND-15）。
/// `role=tree` は (b) のときだけ付く。
///
/// # Errors
///
/// 上流の安全弁が拒否した場合は `ListError`。空の一覧に落とさず呼び出し側へ返す。
pub fn task_tree_list(
    rows: &[ItemRow],
    phase: Phase,
    variant: Variant,
    selected: bool,
    point: PointOf,
) -> Result<Node, ListError> {
    tree_view_list(
        "ul",
        ViewKind::TaskTree,
        variant,
        selected,
        task_items(rows, phase, variant, point),
    )
}

/// タスクツリー (b) の keyed list の親要素を作る。
///
/// # Errors
///
/// [`task_tree_list`] と同じ。
pub fn task_tree_list_b(
    rows: &[ItemRow],
    phase: Phase,
    selected: bool,
    point: PointOf,
) -> Result<Node, ListError> {
    task_tree_list(rows, phase, Variant::B, selected, point)
}

/// タスクツリー (a) の keyed list の親要素を作る。
///
/// # Errors
///
/// [`task_tree_list`] と同じ。
pub fn task_tree_list_a(
    rows: &[ItemRow],
    phase: Phase,
    selected: bool,
    point: PointOf,
) -> Result<Node, ListError> {
    task_tree_list(rows, phase, Variant::A, selected, point)
}

/// このビューの種類。
const KIND: ViewKind = ViewKind::TaskTree;

/// (b) の行 1 件を作る。`lead` は属性の先頭に置く製品の属性、`tree` は木の属性（畳みが働くときは `None`）。
/// その状態で出ない行は `None`。
#[must_use]
pub fn task_item_b(
    row: &ItemRow,
    phase: Phase,
    tree: Option<&TreeItemAttrs>,
    lead: Vec<(&'static str, &str)>,
    target: Option<&PointTarget>,
) -> Option<Node> {
    let shown = row.shown(phase)?;
    let mut attrs = lead;
    if let Some(t) = tree {
        attrs.extend(t.pairs());
    }
    Some(pointed_item(
        "li",
        Variant::B,
        shown.mark,
        Some(row.depth()),
        attrs,
        target,
        task_content(shown.content),
    ))
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
    let world_rows: Vec<(String, Option<crate::a11y::ChangeMark>)> =
        order.iter().map(|&i| ctx.world_rows[i].clone()).collect();
    let Ok(rows) = item_rows(ctx.world, &world_rows, |_| RowKind::Task) else {
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
        let Some(node) = task_item_b(
            row,
            Phase::After,
            tree.as_ref(),
            vec![
                ("data-path", path.as_str()),
                ("data-kind", RowKind::Task.as_str()),
            ],
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
    let list_attrs = tree_list_attrs(KIND, Variant::B, true, has_tree);
    ProductItems {
        rows: out,
        list_attrs,
    }
}
