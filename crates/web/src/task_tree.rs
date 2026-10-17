//! タスクツリーの変種 (b) の本体（TASK-474 1/2。ART-1・ART-9・ART-13・A11Y-22・PoC-8）。
//!
//! 呼び出し元は今のところ `view_fixture`（試験の設定だけ）で、検査器の枠が (b) の本体を通る。製品の画面
//! （`engine_parts`）への結線は、世界状態からタスクのラベルと状態を読む `read` と前後の切り替えの口が決まる
//! 後続の作業で行う。
//!
//! # 信頼境界
//!
//! `Cell` の値は教材由来で信頼できない。ラベルと詳細は文字ノード（`text()`）にだけ入れ、`render` の既定の
//! エスケープを通す。状態は完全一致で閉じた [`TaskState`] に読み、記号と文字は固定の `&'static str` である。
//! 未知の値は既知の状態へ寄せず [`TaskState::Unknown`] にする（fail-closed）。属性・クラス・testid に
//! 教材由来の文字列は入れない。
//!
//! # 固定するもの
//!
//! - 状態の語彙（`open`・`progress`・`done`）は spec に定めが無く、試験データ由来の仮置きである。
//! - 項目の形（タグと子の数）は、状態・印・前後で変えない。keyed list が同じ key の項目を in-place に更新する
//!   条件は形が同じことだからである。`tags` は表示しない（件数で子が増え、形が変わるため）。
//! - 木の位置は描画 1 回につき 1 回、O(n) で求める。
//! - `data-row` 等の指せる行の属性は付けない（TASK-480 の担当）。

use crate::list_policy::ListError;
use fandhe_frontend_core::{el, text, Node};

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{tree_view_list, ItemRow, Phase, Variant, ViewKind};
use crate::tree_attrs::shown_tree_attrs;

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

/// タスクの状態。閉じた集合で、色だけに頼らず記号と文字の両方で示す（A11Y-22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 未着手。
    Open,
    /// 進行中。
    InProgress,
    /// 完了。
    Done,
    /// 語彙の外の値。
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
    el(
        "span",
        vec![(TESTID_ATTR, STATE_BADGE_TESTID)],
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, STATE_GLYPH_TESTID)],
                vec![text(state.glyph())],
            ),
            el(
                "span",
                vec![(TESTID_ATTR, STATE_TEXT_TESTID)],
                vec![text(state.text())],
            ),
        ],
    )
}

/// その状態で出る行を、key（`k<行の ID>`）つきの (b) の項目にする。行の並びは入力の順のまま。
#[must_use]
pub fn task_items_b(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    let attrs = shown_tree_attrs(rows, phase);
    rows.iter()
        .zip(attrs)
        .filter_map(|(row, attrs)| {
            let shown = row.shown(phase)?;
            let attrs = attrs?;
            let cell = shown.content;
            let node = marked_item(
                "li",
                Variant::B,
                shown.mark,
                Some(row.depth()),
                attrs.pairs(),
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
                    state_badge(TaskState::parse(&cell.state)),
                ],
            );
            Some((format!("k{}", row.id().as_str()), node))
        })
        .collect()
}

/// タスクツリー (b) の keyed list の親要素を作る。親の属性は `view()` から呼ぶこの関数で書く（FEND-15）。
///
/// # Errors
///
/// 上流の安全弁が拒否した場合は `ListError`。空の一覧に落とさず呼び出し側へ返す。
pub fn task_tree_list_b(rows: &[ItemRow], phase: Phase, selected: bool) -> Result<Node, ListError> {
    tree_view_list(
        "ul",
        ViewKind::TaskTree,
        Variant::B,
        selected,
        task_items_b(rows, phase),
    )
}
