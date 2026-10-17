//! 左の成果物エリアのカードのビュー、変種 (a)（TASK-477 1/2）。ART-1・ART-17。
//!
//! 呼び出し元は、今は検査器の代役の部品（`view_fixture`）だけである。製品の `view()`（`engine_parts::ArtifactPart`）へは
//! まだ結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から `Row<Cell>` を作る規則も未決のためである
//! （TASK-470・TASK-347 の後続）。変種 (b)（`article`・`ul`・`marker`）は TASK-477 2/2 の担当で、ここでは触れない。
//!
//! # 固定するもの
//!
//! - 特定の教材の部品は作らない。ラベル（`tags`）と状態（`state`）は行データ（[`Cell`]）から受け取り、型・識別子に
//!   固有の語を含めない（`tests/artifact_view_generic_names.rs` が機械的に確かめる）
//! - 項目の形（タグ・属性名・子の数と種別）は、前後の状態でも印の値でも変わらない（ART3-4 の延長）。
//!   動的に変わる部品は keyed list の項目の内側に置く
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である。計算量は行数に対して O(n)
//!
//! # 変種 (a)
//!
//! 外枠は `marked_item(.., Variant::A, ..)` の `li` と内側の `div`（印の宿主）で、中身は `wireframe_ui::card_basic`
//! （ラベル・補足・状態の `tag`）と、ラベルを連結した `tag` 1 個である。
//!
//! - `card_basic` の `secondary`・`trailing` は常に `Some` で渡す。`None` だと要素自体が消え、前後で子の列が変わる
//! - `tags` は区切り（[`TAGS_SEPARATOR`]）で連結した `tag` 1 個にし、空でも必ず出す。1 件ごとに `tag` にすると
//!   件数の違う前後で子の数が変わり、keyed list の in-place 更新の前提が崩れる
//! - `leading` は `None` に固定する。`wireframe_ui::icon::*` は `svg` に `aria-hidden` を必ず出し、a11y の契約（版 2）の
//!   違反になる。採るには契約の版 3 と spec の更新が先に要る
//! - 状態は `tag` の文字で示し、色に頼らない。強調（`Primary`）は [`CardState::Open`] のときだけで、class が変わるだけで
//!   形は変えない。状態やラベルが変わった行は、行単位の「変更」の印だけで示し、欄ごとの印は持たない（ART-17）
//! - `wireframe_css()` は呼ばない。固定のライト配色の CSS の配信は TASK-490 の担当である
//!
//! # 信頼境界
//!
//! ラベル・補足・状態・`tags` は教材由来で信頼できない。`text()` の文字のノードにだけ入れ、`render` の既定の
//! エスケープを通す（SAFE-11）。属性・class に入るのは閉じた集合の `&'static str`（`data-kind` と部品の class）だけで、
//! 状態の文字も教材の値ではなく [`CardState::text`] の固定の文字列である。

use fandhe_frontend_core::Node;
use fandhe_frontend_wireframe_ui::{card_basic, tag, Primary, Size};

use crate::artifact_mark::marked_item;
use crate::artifact_view::{view_list, Cell, ItemRow, Phase, Variant, ViewKind};
use crate::list_policy::ListError;

/// ラベル（`tags`）を 1 個の `tag` にまとめるときの区切り。
pub const TAGS_SEPARATOR: &str = "・";

/// カードの状態の閉じた語彙。
///
/// この語彙は spec に定めが無く、試験データと PoC-8 の試作由来の仮置きである。一覧の外の値は既知の状態へ寄せず
/// [`CardState::Unknown`] にする（fail-closed）。変種 (b) の `marker`（TASK-477 2/2）もこの列挙を再利用する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardState {
    /// 下書き。
    Draft,
    /// オープン。
    Open,
    /// マージ済み。
    Merged,
    /// クローズ。
    Closed,
    /// 語彙の外。
    Unknown,
}

impl CardState {
    /// 語彙の全件。
    pub const ALL: [CardState; 5] = [
        CardState::Draft,
        CardState::Open,
        CardState::Merged,
        CardState::Closed,
        CardState::Unknown,
    ];

    /// 行データの `state` を完全一致で読む。一覧の外・大文字小文字違い・空白つきは `Unknown`。
    #[must_use]
    pub fn parse(s: &str) -> CardState {
        match s {
            "draft" => CardState::Draft,
            "open" => CardState::Open,
            "merged" => CardState::Merged,
            "closed" => CardState::Closed,
            _ => CardState::Unknown,
        }
    }

    /// 画面に出す固定の文字。
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            CardState::Draft => "下書き",
            CardState::Open => "オープン",
            CardState::Merged => "マージ済み",
            CardState::Closed => "クローズ",
            CardState::Unknown => "不明",
        }
    }
}

/// カード 1 件の中身（`card_basic` とラベルの `tag`）。
fn card_content(cell: &Cell) -> Vec<Node> {
    let state = CardState::parse(cell.state.as_str());
    let state_tag = tag(
        state.text(),
        Size::Sm,
        Primary(state == CardState::Open),
        None,
    );
    let card = card_basic(
        cell.label.as_str(),
        Some(cell.detail.as_str()),
        None,
        Some(state_tag),
        Size::Md,
    );
    let labels = tag(
        cell.tags.join(TAGS_SEPARATOR).as_str(),
        Size::Xs,
        Primary(false),
        None,
    );
    vec![card, labels]
}

/// 変種 (a) の行 1 件を作る。その状態で出ない行は `None`。木の属性は付けない。
#[must_use]
pub fn card_item_a(row: &ItemRow, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(marked_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        card_content(shown.content),
    ))
}

/// 行の並びから、変種 (a) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn card_items_a(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| card_item_a(row, phase).map(|n| (format!("k{}", row.id().as_str()), n)))
        .collect()
}

/// カードの keyed list の親要素を作る。親に `role` は付かない（(a)・(b) とも）。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn card_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    view_list(tag, ViewKind::CardList, selected, items)
}
