//! 教材データの検証・変換を行う crate のライブラリ側の入口。
//!
//! bin の `fandhe-pack` から呼ばれる。CLI の枠（`cli`）と診断の型（`diag`）の骨格を持つ。
//! 各コマンドの中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod cli;
pub mod diag;

pub use diag::{DiagCode, Diagnostic, DiagnosticError};
