//! 文書のビューが練習データの本文を出す宣言（`views.<ID>.body`）の閉じた集合と読み取り（TASK-66 1/2）。
//!
//! 出所は語彙の文書の EP-13（`view.document_body`）と spec の PACK-38・D-50・D-51 である。呼び出し元は、
//! pack の `view_body`（宣言の検査と `E-PARSE` への写し）と、web の文書ビュー（`engine_parts::ArtifactPart`。ART-18）である。
//!
//! contract に置く理由: 画面と教材の検査は同じ閉じた集合・同じ解決の規則を使うが、web から pack への
//! normal 依存は禁じられている（`docs/architecture.md` 第 4 節。前例は `left_pane`・`appearance`）。
//!
//! `Ir::views` は値をそのまま通す欄のため、`body` を足しても IR の型は変わらない。既定は `hidden` で、
//! 宣言しない教材の動作と IR は変わらない。I/O も状態も持たない純関数だけで構成し、wasm32 でも動く。

use serde_json::Value;

use crate::doc::Doc;
use crate::ir::Ir;

/// 文書のビューの本文の表示（EP-13）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewBody {
    /// 出さない（既定）。題・属性・注記だけを出す今の動作。
    #[default]
    Hidden,
    /// 行の要素が持つ練習データの本文を出す。
    Shown,
}

impl ViewBody {
    /// 全種類。
    pub const ALL: [ViewBody; 2] = [ViewBody::Hidden, ViewBody::Shown];

    /// 語彙の綴り。
    pub const fn as_str(self) -> &'static str {
        match self {
            ViewBody::Hidden => "hidden",
            ViewBody::Shown => "shown",
        }
    }

    /// 語彙の綴りから読む。大文字小文字を区別して完全一致だけを通す。
    pub fn from_vocab(s: &str) -> Option<ViewBody> {
        ViewBody::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

/// `views.<ID>.body` が読めない理由。誤りの型は入力の値を持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewBodyReadError {
    /// 文字列ではない（`null` を含む）。
    NotAString,
    /// 2 値のどれでもない。
    Unknown,
}

/// 1 つのビューの宣言から `body` を読む。
///
/// ビューが表でない、または `body` の鍵が無いときは既定（`Hidden`）を返す。ビューの形そのものの検査は
/// この語の持ち分ではない。
pub fn read_view_body(view: &Value) -> Result<ViewBody, ViewBodyReadError> {
    let Some(v) = view.as_object().and_then(|m| m.get("body")) else {
        return Ok(ViewBody::default());
    };
    let s = v.as_str().ok_or(ViewBodyReadError::NotAString)?;
    ViewBody::from_vocab(s).ok_or(ViewBodyReadError::Unknown)
}

/// ビュー `view_id` が行の要素の練習データ `fixture` の本文として出す木を引く。
///
/// ビューが `Shown` と読め、`fixture` が `Some` で、`ir.fixture_docs` にその ID があるときだけ木を返す。
/// それ以外（未定義のビュー・読めない宣言・`Hidden`・練習データなし・木なし）はすべて `None` で、今の表示
/// （本文なし）を保つ側に倒す。`fixture` は core の要素の型に依らないよう `&str` で受ける。
pub fn body_doc<'a>(ir: &'a Ir, view_id: &str, fixture: Option<&str>) -> Option<&'a Doc> {
    let view = ir.views.get(view_id)?;
    if read_view_body(view) != Ok(ViewBody::Shown) {
        return None;
    }
    ir.fixture_docs.get(fixture?)
}
