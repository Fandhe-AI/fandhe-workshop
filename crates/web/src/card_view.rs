//! 左の成果物エリアのカードのビュー、変種 (a)・(b)（TASK-477）。ART-1・ART-17。
//!
//! 製品の左エリアの差し替え口 `product_items`（TASK-799）は `engine_parts` から呼ばれ、(b) の部品（ラベル・補足・状態のバッジ・
//! `tags`）で行を描く。行の種類は `card`（`data-kind`）、`li` に `data-path` を付け、キーは `p<経路>`（長い経路は `i<位置>`）で、
//! 親に `role` は付かない。状態は `CardState::parse` で読む。語彙の外の空でない値は教材の語のまま文字で出し、空・空白だけは「不明」になる（D-101。語彙は spec の判断。記録は #2059）。
//! `tags` は `read_cell` の規則で空である。
//!
//! 項目の根は `artifact_mark::pointed_item`（`marked_item` に指せる行の 4 属性を足す）で作る。宛先は呼び出し側が `PointOf` で注入し、
//! 宛先の無い行には 4 属性が付かない（TASK-480 1/2。不変条件は `artifact_mark` の doc）。
//!
//! (a) と前後の切り替え（ART-2）は製品から呼ばれない。`view_fixture`（試験の設定だけ）の検査器の枠が (a)・(b) を通る。
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
//! # 変種 (b)
//!
//! 項目の根は `marked_item("li", Variant::B, ..)` で、印の宿主は項目自身である。`ul` の直下に `article` を置くと HTML として
//! 不正になるため、`article` は `li` の内側に置く。`article` の子は次の 4 つで、数と種別は固定する。
//!
//! - ラベルの `span`（[`CARD_LABEL_TESTID`]）
//! - 状態のバッジ。`headless-ui` の `marker::root`（`div`）と `marker::content`（`span`）で、文字は [`CardState::text`]
//! - 補足の `p`（[`CARD_DETAIL_TESTID`]）。空でも出す
//! - ラベルの `ul`（[`CARD_TAGS_TESTID`]）。`li` は 1 個に固定し、文字は [`TAGS_SEPARATOR`] で連結する。1 件ごとに `li` を
//!   作ると、件数の違う前後で子孫の数が変わり、検査器の `deep`（項目の内側の保持）が違反になる。spec の「`ul` の自作」は満たす
//!
//! - 見出し要素は使わない。20 件で左の領域に見出しが 20 個増え、ページの見出しの構成を変えるため
//! - `marker::icon` は呼ばない。`aria-hidden` を固定で出し、a11y の契約（版 2）の閉じた一覧に無い
//! - 色調は [`CardState::tone`] の閉じた `match` で決める。状態は文字でも示し、色に頼らない
//!
//! # 信頼境界
//!
//! ラベル・補足・状態・`tags` は教材由来で信頼できない。`text()` の文字のノードにだけ入れ、`render` の既定の
//! エスケープを通す（SAFE-11）。属性・class に入るのは閉じた集合の `&'static str`（`data-kind` と部品の class）だけで、
//! 状態の文字は語彙の語なら [`CardState::text`] の固定の文字列で、語彙の外の空でない値だけが文字ノードとして出る
//! （属性・class・強調・色調には入れない）。製品が足す `data-path` は経路の値で、属性の値として
//! `render` のエスケープを通る（属性名は固定）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_headless_ui::marker::{self, MarkerRootProps, MarkerTone, MarkerVariant};
use fandhe_frontend_wireframe_ui::{card_basic, tag, Primary, Size};

use crate::a11y::TESTID_ATTR;
use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item, PointOf};
use crate::artifact_view::{
    list_attrs, other_state_word, view_list, Cell, ItemRow, Phase, PointTarget, RowKind, Variant,
    ViewKind,
};
use crate::list_policy::ListError;
use crate::product_rows::{item_rows, path_rows, ProductCtx, ProductItems, ARTIFACT_KEY_PATH_MAX};

/// ラベル（`tags`）を 1 個の `tag` にまとめるときの区切り。
pub const TAGS_SEPARATOR: &str = "・";
/// 変種 (b) のラベルの `data-testid`。
pub const CARD_LABEL_TESTID: &str = "card-label";
/// 変種 (b) の補足の `data-testid`。
pub const CARD_DETAIL_TESTID: &str = "card-detail";
/// 変種 (b) のラベルの一覧（`ul`）の `data-testid`。
pub const CARD_TAGS_TESTID: &str = "card-tags";

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

    /// 変種 (b) のバッジの色調。語彙の外は中立にする（既知の状態へ寄せない）。仮置きで、spec に定めは無い。
    #[must_use]
    pub fn tone(self) -> MarkerTone {
        match self {
            CardState::Draft => MarkerTone::Warning,
            CardState::Open | CardState::Unknown => MarkerTone::Neutral,
            CardState::Merged => MarkerTone::Info,
            CardState::Closed => MarkerTone::Danger,
        }
    }
}

/// バッジの文字。語彙の外の空でない値は教材の語のまま、それ以外は固定の文字（D-101）。強調・色調は `parse` の結果だけで決める。
fn shown_text(state: CardState, raw: &str) -> &str {
    if state == CardState::Unknown {
        if let Some(word) = other_state_word(raw) {
            return word;
        }
    }
    state.text()
}

/// カード 1 件の中身（`card_basic` とラベルの `tag`）。
fn card_content(cell: &Cell) -> Vec<Node> {
    let state = CardState::parse(cell.state.as_str());
    let state_tag = tag(
        shown_text(state, cell.state.as_str()),
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
pub fn card_item_a(row: &ItemRow, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    let shown = row.shown(phase)?;
    Some(pointed_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        target,
        card_content(shown.content),
    ))
}

/// 行の並びから、変種 (a) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn card_items_a(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            card_item_a(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
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

/// 変種 (b) のカード 1 件の中身（`article` 1 個）。子の数と種別は固定する。
fn card_content_b(cell: &Cell) -> Vec<Node> {
    let state = CardState::parse(cell.state.as_str());
    let badge = marker::root(
        MarkerRootProps {
            variant: MarkerVariant::Note,
            tone: state.tone(),
        },
        vec![],
        vec![marker::content(
            vec![],
            vec![text(shown_text(state, cell.state.as_str()))],
        )],
    );
    let joined = cell.tags.join(TAGS_SEPARATOR);
    vec![el(
        "article",
        vec![],
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, CARD_LABEL_TESTID)],
                vec![text(cell.label.as_str())],
            ),
            badge,
            el(
                "p",
                vec![(TESTID_ATTR, CARD_DETAIL_TESTID)],
                vec![text(cell.detail.as_str())],
            ),
            el(
                "ul",
                vec![(TESTID_ATTR, CARD_TAGS_TESTID)],
                vec![el("li", vec![], vec![text(joined.as_str())])],
            ),
        ],
    )]
}

/// 変種 (b) の行 1 件を作る。その状態で出ない行は `None`。
#[must_use]
pub fn card_item_b(row: &ItemRow, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    card_item_b_with(row, phase, Vec::new(), target)
}

/// [`card_item_b`] に、属性の先頭へ置く製品の属性（`data-path` など）を足せる形。`data-kind` はその後に続く。
///
/// `lead` が空なら属性列は [`card_item_b`] と同じで、代役の部品の出力は変わらない。
#[must_use]
pub fn card_item_b_with(
    row: &ItemRow,
    phase: Phase,
    lead: Vec<(&'static str, &str)>,
    target: Option<&PointTarget>,
) -> Option<Node> {
    let shown = row.shown(phase)?;
    let mut attrs = lead;
    attrs.push(("data-kind", row.kind().as_str()));
    Some(pointed_item(
        "li",
        Variant::B,
        shown.mark,
        Some(row.depth()),
        attrs,
        target,
        card_content_b(shown.content),
    ))
}

/// 行の並びから、変種 (b) のその状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn card_items_b(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            card_item_b(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// 変種に応じた項目を返す。呼び出し元（`view_fixture`）の振り分けを一か所にまとめる。
#[must_use]
pub fn card_items(
    rows: &[ItemRow],
    phase: Phase,
    variant: Variant,
    point: PointOf,
) -> Vec<(String, Node)> {
    match variant {
        Variant::A => card_items_a(rows, phase, point),
        Variant::B => card_items_b(rows, phase, point),
    }
}

/// 製品の左エリアからの差し替え口。(b) の部品で行を描く。
///
/// `engine_parts::ArtifactPart::view` の振り分けだけが呼ぶ。行の検査に通らない経路（空・重複）や、出ない行が
/// あるときは行を黙って落とさず、共通の経路の行に退く（fail-closed）。行の順は経路の辞書順のままで、木ではないので
/// 並べ替えや木の属性は要らない。
pub(crate) fn product_items(ctx: &ProductCtx<'_>) -> ProductItems {
    let Ok(rows) = item_rows(ctx.world, ctx.world_rows, |_| RowKind::Card) else {
        return path_rows(ctx);
    };
    let mut out: Vec<ArtifactRow> = Vec::with_capacity(rows.len());
    for (i, ((path, _), row)) in ctx.world_rows.iter().zip(rows.iter()).enumerate() {
        let target = ctx.declared_id.and_then(|id| point_all(id)(row.id()));
        let Some(node) = card_item_b_with(
            row,
            Phase::After,
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
    ProductItems {
        rows: out,
        list_attrs: list_attrs(ViewKind::CardList, true),
    }
}
