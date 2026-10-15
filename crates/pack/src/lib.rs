//! 教材データの検証・変換を行う crate のライブラリ側の入口。
//!
//! bin の `fandhe-pack` から呼ばれる。CLI の枠（`cli`）・診断の型（`diag`）・パックの読み込み層（`source`）・
//! IR を作る `compile`・モジュールの `use` を展開する `expand`（非公開）・Markdown を木にする `markdown`・IR をバイト単位で比べる `compare`・語彙の版の表 `vocab` を持つ。他のコマンドの中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod cli;
pub mod compare;
pub mod compile;
pub mod diag;
mod expand;
pub mod markdown;
pub mod source;
pub mod vocab;
mod yaml_strict;

pub use diag::{DiagCode, Diagnostic, DiagnosticError};
