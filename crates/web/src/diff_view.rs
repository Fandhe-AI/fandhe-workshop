//! 左の成果物エリアの差分のビュー、変種 (b)（TASK-475 1/2。ART-1・ART-8）。
//!
//! 製品の左エリアの差し替え口 `product_items`（TASK-799）は `engine_parts` から呼ばれ、(b) の部品で行を描く。行の種類（`data-kind`）は
//! `state` が `hunk` なら `hunk`、それ以外は `line`、`li` に `data-path` を付け、キーは `p<経路>`（長い経路は `i<位置>`）で、
//! 親に `role` は付かない。深さのクラスは付けない（`pre` がインデントを持つ）。`detail` が「旧:新」の形でなければ行番号は空で、
//! 語彙の外の `state` は `?` になる（語彙は spec の判断。記録は #2059）。
//!
//! 変種 (a) の `wireframe-ui` の `text` は空白を畳むため、コードのインデントが失われる（PoC-8 の G-4）。
//! (b) は自作の部品（`pre`・行番号・記号の列）で描き、インデントを保つ。(a) は対照専用で、製品の構成では選べない
//! （[`diff_item_a`]。TASK-475 2/2）。
//!
//! (a) と前後の切り替え（ART-2）は製品から呼ばれない。`view_fixture`（試験の設定だけ）の検査器の枠が (a)・(b) を通る。
//!
//! # 変種 (a)（対照専用）
//!
//! 製品の契約は差分の (a) を拒否する（`ViewKind::allows`）。(a) は G-4 の観察（インデントが失われる）を試験として
//! 固定するためだけに残し、呼び出し元は代役の部品の対照の口（`fixture_contrast`）と統合試験だけである。
//! (b) と違うのはコードの 1 列だけで、`wireframe_ui::text` の `span` を `data-testid="diff-code"` の `span` で包む。
//! 行番号・記号の列と信頼境界は (b) と同じで、根は `marked_item(.., Variant::A, ..)`（印は内側の `div`）である。
//! `wireframe_css()` は配信していない（TASK-490）ため実画面の `white-space` は `normal` だが、`nowrap` でも
//! 行頭の空白は畳まれるので、観察は wireframe の CSS の有無に依らない。
//!
//! # 印の 2 層
//!
//! - 層 1: 行の印（`+`・`-`・`~`）。前後の切り替えで付く、成果物の変化の印。`artifact_mark::marked_item` が作る
//! - 層 2: 差分そのものの `+`・`-` の列。`state`（`ctx`・`add`・`del`・`hunk`）から [`DiffSign`] を介して作る
//!
//! 2 層は testid（`mark-glyph` と `diff-sign`）・クラス（`mk-glyph` と `df-sign`）・要素のすべてを分ける。
//!
//! # 固定するもの
//!
//! - 項目の形は、行の種類・状態・前後・印のどれでも変わらない。根の子は、記号・文字（層 1）、旧の行番号、新の行番号、
//!   差分の記号（層 2）、`pre` の 6 つで、hunk の行も同じ 6 つを持つ（行番号と記号は空文字、`pre` に見出しを入れる）。
//!   keyed list が同じ key の項目を in-place に更新する前提である（ART3-4 の延長）
//! - `depth` は `None` で渡す。(b) の `marked_item` は深さのクラスを根に足すため、呼び出し側が `class` を渡すと
//!   `class` が重複して a11y の違反になる。インデントは `pre` の空白が持つ
//! - `role`・`aria-*` は付けない。a11y の契約（版 2）は表・行の `role` と `aria-hidden` を閉じた一覧に持たないため、
//!   列は `span` と CSS だけで作る
//! - 行番号は `旧:新` を `:` で分け、各側が空または ASCII の数字だけ（最大 9 桁）のときだけ出す。外れる側は空文字にし、
//!   生の値を番号の列に出さない（fail-closed）
//! - 未知の `state` は `?` にする。追加・削除とは主張しない（fail-closed）。生の値は出さない
//! - `tags` は出さない（件数で子の数が変わり、項目の形が変わるため）。指せる行の属性は `pointed_item` で、宛先のある行にだけ付ける（TASK-480 1/2）
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である。製品の差し替え口はキー `p<経路>` を使う。計算量は行数に対して O(n)
//!
//! `pre` の注意: HTML の構文解析では `pre` の直後の改行 1 つが落ちる。今の経路は CSR で文字ノードを直接作るため
//! 当たらないが、SSR の文字列を構文解析する経路を足すときは注意が要る。
//!
//! # 信頼境界
//!
//! `label`・`detail`・`state` は教材由来の信頼できない値である。`label` と行番号は `text()` の文字ノードにだけ入れ、
//! `render` の既定のエスケープを通す（SAFE-11）。`state` は完全一致で閉じた [`DiffSign`] に読む。
//! タグ・属性名・クラス・testid・`data-kind` の値は閉じた集合の `&'static str` だけである。製品が足す `data-path` は経路の値で、
//! 属性の値として `render` のエスケープを通る（属性名は固定）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_wireframe_ui::{text as wire_text, Bold, Size};

use crate::a11y::TESTID_ATTR;
use crate::artifact_fold::ArtifactRow;
use crate::artifact_mark::{point_all, pointed_item, PointOf};
use crate::artifact_view::{
    list_attrs, view_list, ItemRow, Phase, PointTarget, RowKind, Variant, ViewKind,
};
use crate::list_policy::ListError;
use crate::product_rows::{
    item_rows, path_rows, read_cell, ProductCtx, ProductItems, ARTIFACT_KEY_PATH_MAX,
};

/// 旧の行番号の `data-testid`。
pub const DIFF_OLD_NO_TESTID: &str = "diff-old-no";
/// 新の行番号の `data-testid`。
pub const DIFF_NEW_NO_TESTID: &str = "diff-new-no";
/// 差分の記号（層 2）の `data-testid`。
pub const DIFF_SIGN_TESTID: &str = "diff-sign";
/// コードの 1 行の `data-testid`。
pub const DIFF_CODE_TESTID: &str = "diff-code";
/// 旧の行番号のクラス。`static/app.css` が配置を定義する。
pub const DF_OLD_CLASS: &str = "df-old";
/// 新の行番号のクラス。
pub const DF_NEW_CLASS: &str = "df-new";
/// 差分の記号のクラス。
pub const DF_SIGN_CLASS: &str = "df-sign";
/// コードの 1 行のクラス。
pub const DF_CODE_CLASS: &str = "df-code";

/// 行番号の最大桁数。これを超える側は空にする。
const MAX_DIGITS: usize = 9;

/// 差分そのものの記号（層 2）。`state` を完全一致で読む閉じた集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffSign {
    /// 変更なしの文脈の行（`ctx`）。
    Context,
    /// 追加の行（`add`）。
    Add,
    /// 削除の行（`del`）。
    Delete,
    /// 範囲の見出し（`hunk`）。
    Hunk,
    /// 上記以外。
    Unknown,
}

impl DiffSign {
    /// 綴りから読む。完全一致で、一覧外は [`DiffSign::Unknown`]。
    #[must_use]
    pub fn parse(s: &str) -> DiffSign {
        match s {
            "ctx" => DiffSign::Context,
            "add" => DiffSign::Add,
            "del" => DiffSign::Delete,
            "hunk" => DiffSign::Hunk,
            _ => DiffSign::Unknown,
        }
    }

    /// 記号の文字。未知は `?` で、`+`・`-` に寄せない。
    #[must_use]
    pub fn glyph(self) -> &'static str {
        match self {
            DiffSign::Context | DiffSign::Hunk => "",
            DiffSign::Add => "+",
            DiffSign::Delete => "-",
            DiffSign::Unknown => "?",
        }
    }
}

fn digits_or_empty(s: &str) -> &str {
    if s.len() <= MAX_DIGITS && s.bytes().all(|b| b.is_ascii_digit()) {
        s
    } else {
        ""
    }
}

/// `旧:新` の補足から、旧と新の行番号を取り出す。形が崩れた側は空文字にする。
#[must_use]
pub fn line_numbers(detail: &str) -> (&str, &str) {
    match detail.split_once(':') {
        Some((old, new)) => (digits_or_empty(old), digits_or_empty(new)),
        None => ("", ""),
    }
}

fn cell_span(testid: &'static str, class: &'static str, value: &str) -> Node {
    el(
        "span",
        vec![(TESTID_ATTR, testid), ("class", class)],
        vec![text(value)],
    )
}

/// 行 1 件を作る。その状態で出ない行は `None`。
#[must_use]
pub fn diff_item(row: &ItemRow, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    diff_item_with(row, phase, Vec::new(), target)
}

/// [`diff_item`] に、属性の先頭へ置く製品の属性（`data-path` など）を足せる形。`data-kind` はその後に続く。
///
/// `lead` が空なら属性列は [`diff_item`] と同じで、代役の部品の出力は変わらない。
#[must_use]
pub fn diff_item_with(
    row: &ItemRow,
    phase: Phase,
    lead: Vec<(&'static str, &str)>,
    target: Option<&PointTarget>,
) -> Option<Node> {
    let shown = row.shown(phase)?;
    let cell = shown.content;
    let (old, new) = line_numbers(cell.detail.as_str());
    let sign = DiffSign::parse(cell.state.as_str()).glyph();
    let mut attrs = lead;
    attrs.push(("data-kind", row.kind().as_str()));
    let content = vec![
        cell_span(DIFF_OLD_NO_TESTID, DF_OLD_CLASS, old),
        cell_span(DIFF_NEW_NO_TESTID, DF_NEW_CLASS, new),
        cell_span(DIFF_SIGN_TESTID, DF_SIGN_CLASS, sign),
        el(
            "pre",
            vec![(TESTID_ATTR, DIFF_CODE_TESTID), ("class", DF_CODE_CLASS)],
            vec![text(cell.label.as_str())],
        ),
    ];
    Some(pointed_item(
        "li",
        Variant::B,
        shown.mark,
        None,
        attrs,
        target,
        content,
    ))
}

/// 変種 (a) の行 1 件を作る（対照専用。製品の構成では選べない）。(b) とはコードの列だけが違う。
///
/// コードは `wireframe_ui::text` で描くため、空白が畳まれインデントが表示で失われる（G-4）。
/// 文字列としては空白が残る。その状態で出ない行は `None`。
#[must_use]
pub fn diff_item_a(row: &ItemRow, phase: Phase, target: Option<&PointTarget>) -> Option<Node> {
    let shown = row.shown(phase)?;
    let cell = shown.content;
    let (old, new) = line_numbers(cell.detail.as_str());
    let sign = DiffSign::parse(cell.state.as_str()).glyph();
    let content = vec![
        cell_span(DIFF_OLD_NO_TESTID, DF_OLD_CLASS, old),
        cell_span(DIFF_NEW_NO_TESTID, DF_NEW_CLASS, new),
        cell_span(DIFF_SIGN_TESTID, DF_SIGN_CLASS, sign),
        el(
            "span",
            vec![(TESTID_ATTR, DIFF_CODE_TESTID), ("class", DF_CODE_CLASS)],
            vec![wire_text(cell.label.as_str(), Size::Md, Bold(false))],
        ),
    ];
    Some(pointed_item(
        "li",
        Variant::A,
        shown.mark,
        Some(row.depth()),
        vec![("data-kind", row.kind().as_str())],
        target,
        content,
    ))
}

/// 変種 (a) の行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す（対照専用）。
#[must_use]
pub fn diff_items_a(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            diff_item_a(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// 行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn diff_items(rows: &[ItemRow], phase: Phase, point: PointOf) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| {
            diff_item(row, phase, point(row.id()).as_ref())
                .map(|n| (format!("k{}", row.id().as_str()), n))
        })
        .collect()
}

/// 差分の keyed list の親要素を作る。選ばれていなければ空の一覧になる。
///
/// # Errors
///
/// 上流の安全弁（項目数・キーの重複）が拒否した場合は `ListError`。握りつぶさず呼び出し側へ返す。
pub fn diff_list(
    tag: &'static str,
    selected: bool,
    items: Vec<(String, Node)>,
) -> Result<Node, ListError> {
    view_list(tag, ViewKind::Diff, selected, items)
}

/// 製品の左エリアからの差し替え口。(b) の部品で行を描く。
///
/// `engine_parts::ArtifactPart::view` の振り分けだけが呼ぶ。行の種類は `read_cell` で読んだ `state` が `hunk` のものだけ `Hunk`、
/// それ以外（世界状態から消えた行を含む）は `Line` である。行の検査に通らない経路（空・重複）や、出ない行があるときは
/// 行を黙って落とさず、共通の経路の行に退く（fail-closed）。行の順は経路の辞書順のままで、木ではないので並べ替えない。
pub(crate) fn product_items(ctx: &ProductCtx<'_>) -> ProductItems {
    let kind_of = |path: &str| {
        let state = read_cell(path, ctx.world.get(path)).state;
        if DiffSign::parse(state.as_str()) == DiffSign::Hunk {
            RowKind::Hunk
        } else {
            RowKind::Line
        }
    };
    let Ok(rows) = item_rows(ctx.world, ctx.world_rows, kind_of) else {
        return path_rows(ctx);
    };
    let mut out: Vec<ArtifactRow> = Vec::with_capacity(rows.len());
    for (i, ((path, _), row)) in ctx.world_rows.iter().zip(rows.iter()).enumerate() {
        let target = ctx.declared_id.and_then(|id| point_all(id)(row.id()));
        let Some(node) = diff_item_with(
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
        list_attrs: list_attrs(ViewKind::Diff, true),
    }
}
