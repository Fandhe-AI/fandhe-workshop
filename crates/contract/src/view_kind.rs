//! ビューの種類（`views.<ID>.kind`）の閉じた集合と読み取り（TASK-796、D-97）。
//!
//! 出所は spec の ART-1・AUTHOR-2・EXT-8 と決定 D-97 である。呼び出し元は、pack の `view_kind`
//! （綴りの検査と `E-VIEW-KIND-UNKNOWN`／`E-PARSE` への写し）と、web の `ViewKind::as_str`（綴りを `VIEW_KINDS` の添字で引く）である。
//!
//! contract に置く理由: 画面の `ViewKind` と教材の検査は同じ 7 種の綴りを使うが、web から pack への normal 依存は
//! 禁じられている（`docs/architecture.md` 第 4 節。前例は `view_body`・`left_pane`・`appearance`）。
//! 画面側の fail-closed（一覧にない綴りは描かない）は変えない。本モジュールは検査と画面の両方の綴りの正である。
//!
//! I/O も状態も持たない純関数だけで構成し、wasm32 でも動く。誤りの型は入力の値を持たない。

use serde_json::Value;

/// ビューの種類の綴り 7 種。順の持ち主は本定数で、web の `ViewKind` の宣言順・`ALL` の順はこれに従う（const の表明が守る。#2023）。
pub const VIEW_KINDS: [&str; 7] = [
    "file_tree",
    "document",
    "diff",
    "task_tree",
    "card_list",
    "check_list",
    "app_demo",
];

/// `views.<ID>.kind` が読めない理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewKindReadError {
    /// `kind` の鍵が無い、またはビューが表でない。
    Missing,
    /// 文字列ではない（`null` を含む）。
    NotAString,
    /// 7 種のどれでもない（大文字小文字・前後の空白の違いを含む）。
    Unknown,
}

/// 1 つのビューの宣言から `kind` を読む。大文字小文字を区別した完全一致だけを通す。
pub fn read_view_kind(view: &Value) -> Result<&'static str, ViewKindReadError> {
    let v = view
        .as_object()
        .and_then(|m| m.get("kind"))
        .ok_or(ViewKindReadError::Missing)?;
    let s = v.as_str().ok_or(ViewKindReadError::NotAString)?;
    VIEW_KINDS
        .iter()
        .copied()
        .find(|k| *k == s)
        .ok_or(ViewKindReadError::Unknown)
}
