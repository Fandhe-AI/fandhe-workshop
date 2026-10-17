//! 左の成果物エリアの差分のビュー、変種 (b)（TASK-475 1/2。ART-1・ART-8）。
//!
//! 変種 (a) の `wireframe-ui` の `text` は空白を畳むため、コードのインデントが失われる（PoC-8 の G-4）。
//! (b) は自作の部品（`pre`・行番号・記号の列）で描き、インデントを保つ。
//!
//! 呼び出し元は、今は検査器の代役の部品（`view_fixture`）だけである。製品の `view()`（`engine_parts::ArtifactPart`）へは
//! まだ結線しない。製品には前後の切り替え（ART-2）の口が無く、世界状態から差分の行（`Row<Cell>`）を作る規則も
//! 未決のためである。変種 (a) の扱いは別の担当（TASK-475 2/2）である。
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
//! - `tags` は出さない（件数で子の数が変わり、項目の形が変わるため）。指せる行の属性は付けない（TASK-480）
//! - key は `k` + 行の識別子で、検査器の `keyOf` と同じ形である。計算量は行数に対して O(n)
//!
//! `pre` の注意: HTML の構文解析では `pre` の直後の改行 1 つが落ちる。今の経路は CSR で文字ノードを直接作るため
//! 当たらないが、SSR の文字列を構文解析する経路を足すときは注意が要る。
//!
//! # 信頼境界
//!
//! `label`・`detail`・`state` は教材由来の信頼できない値である。`label` と行番号は `text()` の文字ノードにだけ入れ、
//! `render` の既定のエスケープを通す（SAFE-11）。`state` は完全一致で閉じた [`DiffSign`] に読む。
//! タグ・属性名・クラス・testid・`data-kind` の値は閉じた集合の `&'static str` だけである。

use fandhe_frontend_core::{el, text, Node};

use crate::a11y::TESTID_ATTR;
use crate::artifact_mark::marked_item;
use crate::artifact_view::{view_list, ItemRow, Phase, Variant, ViewKind};
use crate::list_policy::ListError;

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
pub fn diff_item(row: &ItemRow, phase: Phase) -> Option<Node> {
    let shown = row.shown(phase)?;
    let cell = shown.content;
    let (old, new) = line_numbers(cell.detail.as_str());
    let sign = DiffSign::parse(cell.state.as_str()).glyph();
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
    Some(marked_item(
        "li",
        Variant::B,
        shown.mark,
        None,
        vec![("data-kind", row.kind().as_str())],
        content,
    ))
}

/// 行の並びから、その状態で出る項目の `(key, 要素)` を入力の順に返す。
#[must_use]
pub fn diff_items(rows: &[ItemRow], phase: Phase) -> Vec<(String, Node)> {
    rows.iter()
        .filter_map(|row| diff_item(row, phase).map(|n| (format!("k{}", row.id().as_str()), n)))
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
