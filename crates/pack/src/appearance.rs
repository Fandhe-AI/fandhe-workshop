//! `pack.appearance`（外観）の検査と診断への変換（TASK-53 1/2）。
//!
//! `compile` が `check_profile` の直後に、`validate` が静的検査の一部として呼ぶ。閉じた集合（4 値）と読み取りの
//! 規則の実体は `fandhe_workshop_contract::appearance` にあり、画面（web）が同じ関数で IR の値を読めるように
//! してある。本モジュールはその失敗をすべて `E-PARSE`（item は `pack:appearance`）へ写す。
//!
//! 診断の文面は固定の英語で、`theme` の値を写さない。未知の欄の名前は `safe_ident` を通るときだけ写す
//! （`: ` や改行で 1 行の形を壊さないため）。教材パックにはスタイル（CSS・色・フォント）を指定する欄を置かず、
//! `theme` 以外の欄は、ここで IR を出す前に拒否する（fail-closed）。

use fandhe_workshop_contract::appearance::{read_appearance, AppearanceReadError};

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, WorkshopSrc};

/// `pack.appearance` を読んで検査する。読めないときは `E-PARSE` を 1 件返す。
pub fn check_appearance(workshop_file: &str, w: &WorkshopSrc) -> Vec<Diagnostic> {
    let Err(e) = read_appearance(w.pack.appearance.as_ref()) else {
        return Vec::new();
    };
    let message = match e {
        AppearanceReadError::NotAMapping => "pack.appearance must be a mapping".to_string(),
        AppearanceReadError::UnknownKey(k) => {
            if safe_ident(&k) {
                format!("unknown key `{k}` in pack.appearance")
            } else {
                "unknown key in pack.appearance".to_string()
            }
        }
        AppearanceReadError::ThemeNotString => "pack.appearance.theme must be a string".to_string(),
        AppearanceReadError::ThemeUnknown => {
            "pack.appearance.theme must be one of standard, mono, high_contrast, reading"
                .to_string()
        }
    };
    let file = if workshop_file.trim().is_empty() {
        "."
    } else {
        workshop_file
    };
    Diagnostic::new(file, "pack:appearance", DiagCode::Parse, message)
        .ok()
        .into_iter()
        .collect()
}
