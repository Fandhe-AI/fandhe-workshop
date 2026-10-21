//! 教材データの検証・変換を行う crate のライブラリ側の入口。
//!
//! bin の `fandhe-pack` から呼ばれる。CLI の枠（`cli`）・CLI の外部仕様の契約 `cli_contract`（TASK-399 1/2・2/2）・診断の型（`diag`）・パックの読み込み層（`source`）・
//! IR を作る `compile`・モジュールの `use` を展開する `expand`（非公開）・Markdown を木にする `markdown`・IR をバイト単位で比べる `compare`・語彙の版の表 `vocab`・文書ビューの本文の宣言の検査 `view_body`（TASK-66 1/2）・`pack.profile` の検査と教材テスト用の入力の分け `profile`・`pack.appearance` の検査 `appearance`（TASK-53）・`pack.left_pane` の検査と `E-LEFT-*` の 3 種 `left_pane`（TASK-53 2/2）・静的検査 `validate`（参照・ID の重複・形式の版・構造・モジュール呼び出しの巡回・確認の欠落の検査）・入力方式の宣言の検査 `input`（TASK-44）、確認の宣言と未指定の検査 `confirm`（TASK-47）、状態変化の語彙（要素・属性・フラグ・進行・条件つき）の検査 `effect_check`（TASK-43）、条件の語彙（状態・世界状態・論理）の検査 `condition_check`（TASK-42）、拒否の分類の語の検査 `rejection`（TASK-45 1/2）、到達性と行き止まりの検査 `reach`（TASK-55）、教材テストの誤入力の期待の読み取り `play_expect`（TASK-45 2/2）、拒否・問い返しの文面の差し込みの枠と次の行動の検査 `slot_check`（TASK-46 2/2）、`pack.vocab` の宣言と使用の一致の検査 `vocab_check`（TASK-59）・拡張点を使った Step の数え上げ `vocab_report`（TASK-408・TASK-409）、`metrics` の数え方と表 `metrics`（TASK-402）・案内レベルの語の読み取りと本文の選択 `guidance`（TASK-49 1/2）を持つ。他のコマンドの中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod appearance;
pub mod cli;
pub mod cli_contract;
pub mod compare;
pub mod compile;
mod condition_check;
mod confirm;
pub mod diag;
mod effect_check;
mod element_check;
mod expand;
pub mod guidance;
mod input;
pub mod left_pane;
pub mod markdown;
pub mod metrics;
mod placed;
pub mod play_expect;
pub mod profile;
mod reach;
mod rejection;
pub mod slot_check;
pub mod source;
mod start_check;
pub mod validate;
pub mod view_body;
pub mod vocab;
mod vocab_check;
pub mod vocab_report;
mod yaml_strict;

pub use diag::{DiagCode, Diagnostic, DiagnosticError};
