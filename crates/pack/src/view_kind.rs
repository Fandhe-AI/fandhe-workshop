//! `views.<ID>.kind`（ビューの種類）の綴りの検査と診断への変換（TASK-796、D-97。ART-1・AUTHOR-2・EXT-8）。
//!
//! `compile` が `view_body` の検査の直後に、`validate` が静的検査の一部として呼ぶ。閉じた集合と読み取りの規則の
//! 実体は `fandhe_workshop_contract::view_kind` にある。画面（web）は一覧にない綴りを今も描かない（fail-closed）が、
//! 作成者が誤りに気づけるよう、教材の検査でも止める。
//!
//! 7 種のどれでもない文字列は `E-VIEW-KIND-UNKNOWN`、`kind` の欠け・文字列でない値は `E-PARSE` とする
//! （item は `view:<ID>`）。診断の文面は固定の英語で入力の値を写さない。ビュー ID は `safe_ident` を通るときだけ写す。
//! 検出した教材を `compile` は IR を出す前に止める。

use fandhe_workshop_contract::view_kind::{read_view_kind, ViewKindReadError, VIEW_KINDS};

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, WorkshopSrc};
use crate::validate::diag;

/// `workshop.views` の `kind` を ID の順に読んで検査する。読めないビューごとに診断を 1 件返す。
pub fn check_view_kinds(workshop_file: &str, w: &WorkshopSrc) -> Vec<Diagnostic> {
    let file = if workshop_file.trim().is_empty() {
        "."
    } else {
        workshop_file
    };
    let mut out = Vec::new();
    for (id, view) in &w.views {
        let Err(e) = read_view_kind(view) else {
            continue;
        };
        let (item, name) = if safe_ident(id) {
            (format!("view:{id}"), id.as_str())
        } else {
            ("view".to_string(), "<id>")
        };
        let (code, message) = match e {
            ViewKindReadError::Missing => {
                (DiagCode::Parse, format!("views.{name}.kind is required"))
            }
            ViewKindReadError::NotAString => (
                DiagCode::Parse,
                format!("views.{name}.kind must be a string"),
            ),
            ViewKindReadError::Unknown => (
                DiagCode::ViewKindUnknown,
                format!("views.{name}.kind must be one of {}", VIEW_KINDS.join(", ")),
            ),
        };
        out.push(diag(file, &item, code, message));
    }
    out
}
