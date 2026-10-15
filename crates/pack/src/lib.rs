//! 教材データの検証・変換を行う crate のライブラリ側の入口。
//!
//! bin の `fandhe-pack` から呼ばれる。CLI の枠（`cli`）・診断の型（`diag`）・パックの読み込み層（`source`）・
//! IR を作る `compile`・モジュールの `use` を展開する `expand`（非公開）・Markdown を木にする `markdown`・IR をバイト単位で比べる `compare`・語彙の版の表 `vocab`・`pack.profile` の検査と教材テスト用の入力の分け `profile`・静的検査 `validate`（参照・ID の重複・形式の版・構造・モジュール呼び出しの巡回・確認の欠落の検査）・入力方式の宣言の検査 `input`（TASK-44）、確認の宣言と未指定の検査 `confirm`（TASK-47）、状態変化の語彙（要素・属性・フラグ・進行・条件つき）の検査 `effect_check`（TASK-43）、拒否の分類の語の検査 `rejection`（TASK-45）、到達性と行き止まりの検査 `reach`（TASK-55）を持つ。他のコマンドの中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod cli;
pub mod compare;
pub mod compile;
mod confirm;
pub mod diag;
mod effect_check;
mod expand;
mod input;
pub mod markdown;
pub mod profile;
mod reach;
mod rejection;
pub mod source;
pub mod validate;
pub mod vocab;
mod yaml_strict;

pub use diag::{DiagCode, Diagnostic, DiagnosticError};
