//! 教材データの検証・変換を行う crate のライブラリ側の入口。
//!
//! bin の `fandhe-pack` から呼ばれる。CLI の枠（`cli`）・診断の型（`diag`）・パックの読み込み層（`source`）・
//! IR を作る `compile`・Markdown を木にする `markdown`・IR をバイト単位で比べる `compare`・語彙の版の表 `vocab`・状態変化の語彙（要素・属性・フラグ）の・進行と条件つきの検査 `effect_check`（TASK-43）を持つ。他のコマンドの中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod cli;
pub mod compare;
pub mod compile;
pub mod diag;
mod effect_check;
pub mod markdown;
pub mod source;
pub mod vocab;
mod yaml_strict;

pub use diag::{DiagCode, Diagnostic, DiagnosticError};
