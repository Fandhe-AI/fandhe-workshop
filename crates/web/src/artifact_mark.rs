//! 左の成果物ビューの項目 1 件を、変化の印つきで作る部品（TASK-469 1/2・2/2）。
//!
//! 呼び出し元は、7 種のビューの項目を描く側（今は `engine_parts` の仮の左の成果物と、検査器の代役の
//! `view_fixture`。TASK-473〜TASK-479 のビューの本体が続く）。keyed list の項目の根は、ここで作った要素をそのまま使う。
//! 出所は spec の ART-3・A11Y-22 と、PoC-8 の G-1（`wireframe-ui` の部品は属性を受け取らない）・G-7、
//! 訂正した手順 5（スタイルは同一オリジンの外部ファイル `app.css` に置く。SAFE-13・SAFE-15）である。
//!
//! # 固定するもの
//!
//! - 印は記号（`mark-glyph`）・文字（`mark-text`）・`data-mark` の 3 手段で同じ意味を出す。値は `a11y` が正で、再定義しない
//! - 印の宿主は 1 項目にちょうど 1 つ。変種 (b) は項目の根、変種 (a) は項目の中の外枠が持つ
//! - 記号と文字の 2 つの `span` は宿主の直接の子で、子は文字だけ（`a11y::check_mark` の条件）
//! - 項目の形（タグ・子の数と種別・属性名）は印の値によって変わらない（印なしでも記号・文字の 2 つの `span` を持つ）。keyed list は
//!   子ノード列の位置対応が一致するときだけ同じ key の項目を in-place に更新するため、前後の切り替えの前提になる（`artifact_mark_contract` と ART3-4）
//! - インデントのクラスは `d0`〜`d6`。7 以上は `d6` に丸める（無限にクラス名を作らない。fail-closed）
//!
//! # 固定しないもの
//!
//! - 色・コントラスト（`static/app.css`。検査は TASK-490・ART-20）
//! - `wireframe_css()` の書き出しと `app.css` への連結（変種 (a) で実際の部品を使うビューの担当）
//!
//! # 信頼境界
//!
//! タグ・属性名・クラス名・`data-mark` の値・`data-testid` は、閉じた集合の `&'static str` だけを使う。
//! 呼び出し側が渡す `attrs` の値（`data-path` など教材由来）と `content` は、`render` の既定のエスケープを通す。
//! 生の HTML の経路は使わない（SAFE-11）。印は束縛せず項目ごとに静的に描く（`data-bind-attr` で
//! `data-mark` を束縛すると `a11y` の `bound-mark` 違反になる）。記号の要素に `aria-hidden` は付けない
//! （`a11y` の閉じた一覧の外のため）。

use fandhe_frontend_core::{el, text, Node};

use crate::a11y::{
    ChangeMark, MarkValue, MARK_ATTR, MARK_GLYPH_TESTID, MARK_TEXT_TESTID, TESTID_ATTR,
};
use crate::artifact_view::{mark_value, Variant};

/// 記号の要素のクラス。`static/app.css` が配置を定義する。
pub const MARK_GLYPH_CLASS: &str = "mk-glyph";
/// 文字の要素のクラス。`static/app.css` が配置を定義する。
pub const MARK_TEXT_CLASS: &str = "mk-text";
/// インデントのクラス（深さ 0..=6）。`static/app.css` が同じ名前を定義する。
pub const DEPTH_CLASSES: [&str; 7] = ["d0", "d1", "d2", "d3", "d4", "d5", "d6"];

/// 深さに対応するインデントのクラス。表の外（7 以上）は最後の `d6` に丸める。
#[must_use]
pub fn depth_class(depth: u8) -> &'static str {
    let last = DEPTH_CLASSES.len() - 1;
    DEPTH_CLASSES[usize::from(depth).min(last)]
}

/// `data-mark` の値。印なしは空文字（`MARK_NONE_VALUE`）。
#[must_use]
pub fn mark_attr_value(v: MarkValue) -> &'static str {
    match v {
        MarkValue::None => "",
        MarkValue::Mark(m) => ChangeMark::data_value(m),
    }
}

/// 印の宿主に付ける属性（名前, 値）。
#[must_use]
pub fn mark_host_attr(v: MarkValue) -> (&'static str, &'static str) {
    (MARK_ATTR, mark_attr_value(v))
}

/// 記号と文字の 2 つの `span`。子は文字ノードだけで、印なしは 2 つとも空文字になる。
#[must_use]
pub fn mark_nodes(v: MarkValue) -> [Node; 2] {
    [
        el(
            "span",
            vec![
                (TESTID_ATTR, MARK_GLYPH_TESTID),
                ("class", MARK_GLYPH_CLASS),
            ],
            vec![text(v.glyph())],
        ),
        el(
            "span",
            vec![(TESTID_ATTR, MARK_TEXT_TESTID), ("class", MARK_TEXT_CLASS)],
            vec![text(v.text())],
        ),
    ]
}

/// 項目 1 件を作る。
///
/// - (b): `tag` の要素が `attrs`・`data-mark`・深さのクラスを持ち、子は記号・文字・`content`
/// - (a): `tag` の要素は `attrs` だけを持ち、中の外枠の `div` が `data-mark`・深さのクラスを持つ
///
/// `depth` が `None` ならクラスを付けない。
#[must_use]
pub fn marked_item(
    tag: &'static str,
    variant: Variant,
    mark: Option<ChangeMark>,
    depth: Option<u8>,
    attrs: Vec<(&'static str, &str)>,
    content: Vec<Node>,
) -> Node {
    let value = mark_value(mark);
    let mut host = vec![mark_host_attr(value)];
    if let Some(d) = depth {
        host.push(("class", depth_class(d)));
    }
    let [glyph, label] = mark_nodes(value);
    let mut children = vec![glyph, label];
    children.extend(content);
    match variant {
        Variant::B => {
            let mut all = attrs;
            all.extend(host);
            el(tag, all, children)
        }
        Variant::A => el(tag, attrs, vec![el("div", host, children)]),
    }
}
