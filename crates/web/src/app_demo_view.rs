//! 左の成果物エリアのアプリのデモのワイヤーのビュー、変種 (a)・(b)（TASK-479）。ART-1・ART-17。
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
//!   `Active`・`Disabled` は常に `false` で渡す。`wireframe-ui` はこの 2 つを真のときだけ `data-active`・`data-disabled`
//!   として出すため、行データで切り替えると同じ key の項目の属性名の集合が前後で変わる。状態は class の値（`Primary`）と
//!   文字だけで示す
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である。計算量は行数に対して O(n)
//!
//! # 変種 (a)
//!
//! 外枠は `marked_item(.., Variant::A, ..)` の `li` と内側の `div`（印の宿主）で、中身は行の種類（[`RowKind`]）ごとに
//! `wireframe_ui` の部品を 1 つ置く。種類は項目ごとに固定なので、前後で形は変わらない。
//!
//! - 見出し・文字: `text`。タブ: `tag`。欄: `rich_text` の中に `input`。ボタン: `button`。項目: `card_basic`
//! - 上記以外の種類（このビューの語彙の外）は、ラベルを文字として出すだけにする（fail-closed）
//! - タブの横並びは、行ごとの項目（`li`）のまま `static/app.css` の 1 規則で補う。領域ごとの配置（ART-23）は扱わない
//! - `card_basic` の `secondary` は常に `Some`、`leading`・`trailing` は `None` に固定する
//! - `wireframe_ui::icon::*` は使わない。`svg` に `aria-hidden` を必ず出し、a11y の契約（版 2）の違反になる
//! - 状態は文字でも示し、色に頼らない。状態が変わった行は、行単位の「変更」の印だけで示し、欄ごとの印は持たない（ART-17）
//! - `wireframe_css()` は呼ばない。固定のライト配色の CSS の配信は TASK-490 の担当である
//!
//! # 変種 (b)
//!
//! 項目の根は `marked_item("li", Variant::B, ..)` で、印の宿主は項目自身である。子は行の種類ごとに 1 要素（項目は 2 要素）で、
//! 種類は項目ごとに固定なので前後で形は変わらない。
//!
//! - 見出し: `p`。タブ: `span`（[`APP_DEMO_TAB_TESTID`]）。ボタン: `button`（`type=button`）
//! - 欄: `headless-ui` の `field::root` > `field::label` + `field::input`（読み取り専用。`value` は空でも必ず出す）
//! - 項目: ラベルの `span` と補足の `span`（[`APP_DEMO_ITEM_LABEL_TESTID`]・[`APP_DEMO_ITEM_DETAIL_TESTID`]）
//! - 上記以外の種類は、ラベルを `p` の文字として出すだけにする（fail-closed）
//! - `role`・`aria-*` は 1 つも出さない。spec は自作の `role=tab`・`role=listitem` と親の `role=tablist` を挙げるが、
//!   a11y の契約（版 2）では `tablist` はページに 1 個（`pane-tabs`）で、`tab` は `pane-tab` の予約 id と `aria-controls` を
//!   要し、`listitem` は閉じた一覧に無いため、付けると適合試験の違反になる。意味は `ul`・`li`・`button` の暗黙の役割で保つ。
//!   契約の版 3 に足すかは spec 側の確認事項である（判断は `docs/testing.md`）
//! - 選択中・無効は文字（[`WireState::text`]）で示し、`aria-selected`・`disabled` は出さない（`disabled` は同じ key の
//!   項目の属性名の集合を前後で変えるため）
//! - 欄の `id` は `app-demo-field-{行の添字}` で、行の識別子（信頼できない値）から作らない。予約 id（`pane-*`）と衝突しない
//!
//! # 信頼境界
//!
//! ラベル・補足・状態は教材由来で信頼できない。`text()` の文字のノードにだけ入れ、`render` の既定のエスケープを通す
//! （SAFE-11）。属性・class に入るのは閉じた集合の `&'static str`（`data-kind` と部品の class）だけで、`data-state` は作らない。
//! 状態の文字も教材の値ではなく [`WireState::text`] の固定の文字列である。
//!
//! 唯一の例外が変種 (b) の欄の `input` の `value`（= 補足）である。属性名は固定、`render` の既定のエスケープを通る、
//! URL・イベントハンドラの属性ではない、の 3 点から許容し、エスケープの試験で確かめる。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_headless_ui::field::{self, FieldIds, FieldProps};
use fandhe_frontend_wireframe_ui::{
    button, card_basic, input, rich_text, tag, text as wire_text, Active, Bold, Disabled,
    Orientation, Primary, Size,
};

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{view_list, Cell, ItemRow, Phase, RowKind, Variant, ViewKind};
use crate::list_policy::ListError;

/// 欄の補足（入力済みの文字）が空のときに出す固定の文字。
pub const EMPTY_FIELD_TEXT: &str = "（未入力）";

/// 変種 (b) のタブの `span` の `data-testid`。
pub const APP_DEMO_TAB_TESTID: &str = "app-demo-tab";
/// 変種 (b) の項目のラベルの `data-testid`。
pub const APP_DEMO_ITEM_LABEL_TESTID: &str = "app-demo-item-label";
/// 変種 (b) の項目の補足の `data-testid`。
pub const APP_DEMO_ITEM_DETAIL_TESTID: &str = "app-demo-item-detail";
/// 変種 (b) の見出しの `p` の class。
pub const APP_DEMO_HEADING_CLASS: &str = "app-demo-heading";
/// 変種 (b) の欄の `id` の接頭辞。予約 id（`pane-tab-*`・`pane-*`）と衝突しない。
pub const APP_DEMO_FIELD_ID_PREFIX: &str = "app-demo-field-";

/// 部品の状態の閉じた語彙。
///
/// この語彙は spec に定めが無く、試験データと PoC-8 の試作由来の仮置きである。一覧の外の値は既知の状態へ寄せず
/// [`WireState::Unknown`] にする（fail-closed）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireState {
    /// 通常（空の値）。
    Normal,
    /// 選択中（`active`）。
    Active,
    /// 無効（`disabled`）。
    Disabled,
    /// 語彙の外。
    Unknown,
}

impl WireState {
    /// 語彙の全件。
    pub const ALL: [WireState; 4] = [
        WireState::Normal,
        WireState::Active,
        WireState::Disabled,
        WireState::Unknown,
    ];

    /// 行データの `state` を完全一致で読む。一覧の外・大文字小文字違い・空白つきは `Unknown`。
    #[must_use]
    pub fn parse(s: &str) -> WireState {
        match s {
            "" => WireState::Normal,
            "active" => WireState::Active,
            "disabled" => WireState::Disabled,
            _ => WireState::Unknown,
        }
    }

    /// ラベルに連結する固定の文字。通常は何も足さない。
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            WireState::Normal => "",
            WireState::Active => "（選択中）",
            WireState::Disabled => "（無効）",
            WireState::Unknown => "（不明）",
        }
    }
}

/// ラベルに状態の文字を連結する。
fn with_state(label: &str, state: WireState) -> String {
    format!("{label}{}", state.text())
}

/// 行の種類ごとの部品 1 つ。`Active`・`Disabled` は常に `false` で、形を変えない。
fn part(kind: RowKind, cell: &Cell) -> Node {
    let state = WireState::parse(cell.state.as_str());
    match kind {
        RowKind::Heading => wire_text(cell.label.as_str(), Size::Lg, Bold(true)),
        RowKind::Tab => tag(
            with_state(cell.label.as_str(), state).as_str(),
            Size::Md,
            Primary(state == WireState::Active),
            None,
        ),
        RowKind::Field => {
            let shown = if cell.detail.is_empty() {
                EMPTY_FIELD_TEXT
            } else {
                cell.detail.as_str()
            };
            rich_text(
                cell.label.as_str(),
                None,
                Some(input(shown, None, Size::Md, Active(false), Disabled(false))),
                Size::Md,
                Bold(false),
                Orientation::Horizontal,
            )
        }
        RowKind::Button => button(
            with_state(cell.label.as_str(), state).as_str(),
            None,
            Size::Md,
            Primary(state != WireState::Disabled),
            Disabled(false),
        ),
        RowKind::Item => card_basic(
            cell.label.as_str(),
            Some(cell.detail.as_str()),
            None,
            None,
            Size::Md,
        ),
        RowKind::Text
        | RowKind::Dir
        | RowKind::File
        | RowKind::Hunk
        | RowKind::Line
        | RowKind::Task
        | RowKind::Card
        | RowKind::Check
        | RowKind::Block => wire_text(cell.label.as_str(), Size::Md, Bold(false)),
    }
}

/// 変種 (a) の行 1 件を作る。その状態で出ない行は `None`。木の属性は付けない。
#[must_use]
pub fn app_demo_item_a(row: &ItemRow, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(marked_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        vec![part(row.kind(), shown.content)],
    ))
}

/// 行の並びから、変種 (a) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn app_demo_items_a(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            app_demo_item_a(row, phase).map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// アプリのデモの keyed list の親要素を作る。親に `role` は付かない。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn app_demo_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    view_list(tag, ViewKind::AppDemo, selected, items)
}

/// 変種 (b) の欄。読み取り専用で、`value` は空でも必ず出す（属性名の集合を前後で変えない）。
fn field_b(cell: &Cell, index: usize) -> Node {
    let id = format!("{APP_DEMO_FIELD_ID_PREFIX}{index}");
    let props = FieldProps {
        id: id.as_str(),
        ids: FieldIds::default(),
        disabled: false,
        invalid: false,
        required: false,
        readonly: true,
        has_helper_text: false,
    };
    field::root(
        &props,
        vec![],
        vec![
            field::label(&props, vec![], vec![text(cell.label.as_str())]),
            field::input(
                &props,
                vec![
                    ("type", "text"),
                    ("value", cell.detail.as_str()),
                    ("placeholder", EMPTY_FIELD_TEXT),
                ],
            ),
        ],
    )
}

/// 変種 (b) の中身。行の種類ごとに子の数と種別を固定する。
fn content_b(kind: RowKind, cell: &Cell, index: usize) -> Vec<Node> {
    let state = WireState::parse(cell.state.as_str());
    match kind {
        RowKind::Heading => vec![el(
            "p",
            vec![("class", APP_DEMO_HEADING_CLASS)],
            vec![text(cell.label.as_str())],
        )],
        RowKind::Tab => vec![el(
            "span",
            vec![(TESTID_ATTR, APP_DEMO_TAB_TESTID)],
            vec![text(with_state(cell.label.as_str(), state).as_str())],
        )],
        RowKind::Field => vec![field_b(cell, index)],
        RowKind::Button => vec![el(
            "button",
            vec![("type", "button")],
            vec![text(with_state(cell.label.as_str(), state).as_str())],
        )],
        RowKind::Item => vec![
            el(
                "span",
                vec![(TESTID_ATTR, APP_DEMO_ITEM_LABEL_TESTID)],
                vec![text(cell.label.as_str())],
            ),
            el(
                "span",
                vec![(TESTID_ATTR, APP_DEMO_ITEM_DETAIL_TESTID)],
                vec![text(cell.detail.as_str())],
            ),
        ],
        RowKind::Text
        | RowKind::Dir
        | RowKind::File
        | RowKind::Hunk
        | RowKind::Line
        | RowKind::Task
        | RowKind::Card
        | RowKind::Check
        | RowKind::Block => vec![el("p", vec![], vec![text(cell.label.as_str())])],
    }
}

/// 変種 (b) の行 1 件を作る。その状態で出ない行は `None`。`index` は行の並びの添字（欄の `id` に使う）。
#[must_use]
pub fn app_demo_item_b(row: &ItemRow, index: usize, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(marked_item(
        "li",
        Variant::B,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        content_b(row.kind(), shown.content, index),
    ))
}

/// 行の並びから、変種 (b) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn app_demo_items_b(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .enumerate()
        .filter_map(|(i, row)| {
            app_demo_item_b(row, i, phase).map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// 変種に応じた項目を返す。呼び出し元（`view_fixture`）の振り分けを一か所にまとめる。
#[must_use]
pub fn app_demo_items(rows: &[ItemRow], phase: Phase, variant: Variant) -> Vec<(String, Node)> {
    match variant {
        Variant::A => app_demo_items_a(rows, phase),
        Variant::B => app_demo_items_b(rows, phase),
    }
}
