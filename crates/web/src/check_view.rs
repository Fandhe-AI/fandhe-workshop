//! 左の成果物エリアのチェックの一覧のビュー、変種 (a)・(b)（TASK-478）。ART-1・ART-17。
//!
//! 呼び出し元は、今は検査器の代役の部品（`view_fixture`）だけである。製品の `view()`（`engine_parts::ArtifactPart`）へは
//! まだ結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から `Row<Cell>` を作る規則も未決のためである
//! （TASK-470・TASK-347 の後続）。
//!
//! # 固定するもの
//!
//! - 特定の教材の部品は作らない。ラベル（`label`）・補足（`detail`）・状態（`state`）は行データ（[`Cell`]）から受け取り、
//!   型・識別子に固有の語を含めない（`tests/artifact_view_generic_names.rs` が機械的に確かめる）
//! - 項目の形（タグ・属性名・子の数と種別）は、前後の状態でも印の値でも変わらない（ART3-4 の延長）。
//!   状態が変わっても先頭の記号・状態の文字の要素は必ず出し、子の数を変えない
//! - 状態は記号つきの文字で示し、色に頼らない。key は `k` + 行の識別子で、計算量は行数に対して O(n)
//! - 状態が変わった行は、行単位の「変更」の印だけで示し、欄ごとの印は持たない（D-49・ART-17）
//!
//! # 変種 (a)
//!
//! 外枠は `marked_item(.., Variant::A, ..)` の `li` と内側の `div`（印の宿主）で、中身は `wireframe_ui::rich_text`
//! （先頭に記号の `span`、末尾に状態の `tag`）と補足の `span` である。
//!
//! - 先頭は `wireframe_ui::icon::*` を使わず、自作の `span`（[`ICON_CLASS`]）に記号の文字を入れる。`icon::*` は `svg` に
//!   `aria-hidden` を必ず出し、a11y の契約（版 2）の違反になるため
//! - 先頭の `span` はどの状態でも出す。出さない状態があると前後で子の数が変わり、keyed list の in-place 更新が崩れる
//! - 強調（`Primary`）は [`CheckState::Failure`] のときだけで、class が変わるだけで形は変えない
//! - `wireframe_css()` は呼ばない。固定のライト配色の CSS の配信は TASK-490 の担当である
//!
//! # 変種 (b)
//!
//! 項目の根は `marked_item("li", Variant::B, ..)` で、印の宿主は項目自身である。子は次の 3 つで、数と種別は固定する。
//!
//! - ラベルの `span`（[`CHECK_LABEL_TESTID`]）
//! - 状態のバッジ。`headless-ui` の `marker::root`（`div`）と `marker::content`（`span`）で、文字は [`CheckState::label`]
//! - 補足の `span`（[`CHECK_DETAIL_TESTID`]）。空でも出す
//!
//! spec は行に `role=listitem`、親に `role=list` を挙げるが、a11y の契約（版 2）の閉じた一覧に無いため付けない。
//! `li`（`ul` の暗黙の listitem）で意味を保つ。契約の版 3 に足すかは spec 側の確認事項である（判断は `docs/testing.md`）。
//! `marker::icon` は呼ばない（`aria-hidden` を固定で出すため）。
//!
//! # 信頼境界
//!
//! ラベル・補足・状態は教材由来で信頼できない。`text()` の文字のノードにだけ入れ、`render` の既定の
//! エスケープを通す（SAFE-11）。属性・class に入るのは閉じた集合の `&'static str` だけで、教材由来の文字列を
//! `data-state` のような属性へ入れない。状態の文字・記号も教材の値ではなく [`CheckState`] の固定の文字列である。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_headless_ui::marker::{self, MarkerRootProps, MarkerTone, MarkerVariant};
use fandhe_frontend_wireframe_ui::{rich_text, tag, Bold, Orientation, Primary, Size};

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{view_list, Cell, ItemRow, Phase, Variant, ViewKind};
use crate::list_policy::ListError;

/// 変種 (a) の先頭の記号の `span` の class。
pub const ICON_CLASS: &str = "icon";
/// 変種 (a) の補足の `span` の class。
pub const DETAIL_CLASS: &str = "check-detail";
/// 変種 (b) のラベルの `data-testid`。
pub const CHECK_LABEL_TESTID: &str = "check-label";
/// 変種 (b) の補足の `data-testid`。
pub const CHECK_DETAIL_TESTID: &str = "check-detail";

/// チェックの状態の閉じた語彙。
///
/// この語彙・記号・色調は spec に定めが無く、PoC-8 の試作由来の仮置きである。一覧の外の値は既知の状態へ寄せず
/// [`CheckState::Unknown`] にする（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    /// 成功。
    Success,
    /// 失敗。
    Failure,
    /// 実行中。
    Running,
    /// 待機中。
    Pending,
    /// 省略。
    Skipped,
    /// 語彙の外。
    Unknown,
}

impl CheckState {
    /// 語彙の全件。
    pub const ALL: [CheckState; 6] = [
        CheckState::Success,
        CheckState::Failure,
        CheckState::Running,
        CheckState::Pending,
        CheckState::Skipped,
        CheckState::Unknown,
    ];

    /// 行データの `state` を完全一致で読む。一覧の外・大文字小文字違い・空白つきは `Unknown`。
    #[must_use]
    pub fn parse(s: &str) -> CheckState {
        match s {
            "success" => CheckState::Success,
            "failure" => CheckState::Failure,
            "running" => CheckState::Running,
            "pending" => CheckState::Pending,
            "skipped" => CheckState::Skipped,
            _ => CheckState::Unknown,
        }
    }

    /// 状態の記号。実行中と待機中は同じ記号で、文字で区別する。
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            CheckState::Success => "✓",
            CheckState::Failure => "✗",
            CheckState::Running | CheckState::Pending => "…",
            CheckState::Skipped => "–",
            CheckState::Unknown => "?",
        }
    }

    /// 状態の固定の文字。
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            CheckState::Success => "成功",
            CheckState::Failure => "失敗",
            CheckState::Running => "実行中",
            CheckState::Pending => "待機中",
            CheckState::Skipped => "省略",
            CheckState::Unknown => "不明",
        }
    }

    /// 記号と文字を合わせた固定の文字。変種 (b) のバッジに出す。
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            CheckState::Success => "✓ 成功",
            CheckState::Failure => "✗ 失敗",
            CheckState::Running => "… 実行中",
            CheckState::Pending => "… 待機中",
            CheckState::Skipped => "– 省略",
            CheckState::Unknown => "? 不明",
        }
    }

    /// 変種 (b) のバッジの色調。語彙の外は中立にする（既知の状態へ寄せない）。仮置きで、spec に定めは無い。
    #[must_use]
    pub fn tone(self) -> MarkerTone {
        match self {
            CheckState::Success => MarkerTone::Info,
            CheckState::Failure => MarkerTone::Danger,
            CheckState::Running | CheckState::Pending => MarkerTone::Warning,
            CheckState::Skipped | CheckState::Unknown => MarkerTone::Neutral,
        }
    }
}

/// 補足の `span`（変種 (a)）。空でも必ず出す。
fn detail_a(cell: &Cell) -> Node {
    el(
        "span",
        vec![("class", DETAIL_CLASS)],
        vec![text(cell.detail.as_str())],
    )
}

/// 変種 (a) の中身（`rich_text` と補足）。
fn check_content(cell: &Cell) -> Vec<Node> {
    let state = CheckState::parse(cell.state.as_str());
    let glyph = el(
        "span",
        vec![("class", ICON_CLASS)],
        vec![text(state.glyph())],
    );
    let state_tag = tag(
        state.text(),
        Size::Sm,
        Primary(state == CheckState::Failure),
        None,
    );
    vec![
        rich_text(
            cell.label.as_str(),
            Some(glyph),
            Some(state_tag),
            Size::Md,
            Bold(false),
            Orientation::Horizontal,
        ),
        detail_a(cell),
    ]
}

/// 変種 (a) の行 1 件を作る。その状態で出ない行は `None`。木の属性は付けない。
#[must_use]
pub fn check_item_a(row: &ItemRow, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(marked_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        check_content(shown.content),
    ))
}

/// 行の並びから、変種 (a) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn check_items_a(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| check_item_a(row, phase).map(|n| (format!("k{}", row.id().as_str()), n)))
        .collect()
}

/// チェックの keyed list の親要素を作る。親に `role` は付かない（(a)・(b) とも）。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn check_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    view_list(tag, ViewKind::CheckList, selected, items)
}

/// 変種 (b) の中身。子の数と種別は固定する。
fn check_content_b(cell: &Cell) -> Vec<Node> {
    let state = CheckState::parse(cell.state.as_str());
    let badge = marker::root(
        MarkerRootProps {
            variant: MarkerVariant::Note,
            tone: state.tone(),
        },
        vec![],
        vec![marker::content(vec![], vec![text(state.label())])],
    );
    vec![
        el(
            "span",
            vec![(TESTID_ATTR, CHECK_LABEL_TESTID)],
            vec![text(cell.label.as_str())],
        ),
        badge,
        el(
            "span",
            vec![(TESTID_ATTR, CHECK_DETAIL_TESTID)],
            vec![text(cell.detail.as_str())],
        ),
    ]
}

/// 変種 (b) の行 1 件を作る。その状態で出ない行は `None`。
#[must_use]
pub fn check_item_b(row: &ItemRow, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(marked_item(
        "li",
        Variant::B,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        check_content_b(shown.content),
    ))
}

/// 行の並びから、変種 (b) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn check_items_b(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| check_item_b(row, phase).map(|n| (format!("k{}", row.id().as_str()), n)))
        .collect()
}

/// 変種に応じた項目を返す。呼び出し元（`view_fixture`）の振り分けを一か所にまとめる。
#[must_use]
pub fn check_items(rows: &[ItemRow], phase: Phase, variant: Variant) -> Vec<(String, Node)> {
    match variant {
        Variant::A => check_items_a(rows, phase),
        Variant::B => check_items_b(rows, phase),
    }
}
