//! `views.<ID>.body`（文書のビューが練習データの本文を出す宣言）の検査と診断への変換（TASK-66 1/2、PACK-38）。
//!
//! `compile` が外観の検査の近くで、`validate` が静的検査の一部として呼ぶ。閉じた集合（`shown`・`hidden`）と
//! 読み取りの規則の実体は `fandhe_workshop_contract::view_body` にあり、画面（web）が同じ関数で IR の値を読める。
//! 本モジュールは読み取りの失敗をすべて `E-PARSE`（item は `view:<ID>`）へ写す。新しい診断コードは足さない。
//!
//! `kind` は見ない（pack は今もビューの種類を検査しない。種類は画面の契約が持つ）。診断の文面は固定の英語で、
//! 入力の値を写さない。ビュー ID は `safe_ident` を通るときだけ写す。読めない宣言は `compile` が IR を出す前に
//! 止める（fail-closed）。

use fandhe_workshop_contract::view_body::{read_view_body, ViewBodyReadError};

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, WorkshopSrc};
use crate::validate::diag;

/// `workshop.views` の `body` を ID の順に読んで検査する。読めないビューごとに `E-PARSE` を 1 件返す。
pub fn check_view_bodies(workshop_file: &str, w: &WorkshopSrc) -> Vec<Diagnostic> {
    let file = if workshop_file.trim().is_empty() {
        "."
    } else {
        workshop_file
    };
    let mut out = Vec::new();
    for (id, view) in &w.views {
        let Err(e) = read_view_body(view) else {
            continue;
        };
        let (item, name) = if safe_ident(id) {
            (format!("view:{id}"), id.as_str())
        } else {
            ("view".to_string(), "<id>")
        };
        let message = match e {
            ViewBodyReadError::NotAString => format!("views.{name}.body must be a string"),
            ViewBodyReadError::Unknown => {
                format!("views.{name}.body must be one of shown, hidden")
            }
        };
        out.push(diag(file, &item, DiagCode::Parse, message));
    }
    out
}
