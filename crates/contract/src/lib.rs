//! 教材データ・イベント・成果物の契約型を置く crate の入口。
//!
//! core と web が共有する型の置き場で、表示基盤にも core にも依存しない。
//! 現時点は IR の型と `ir_format` 1・2 の契約（`ir`、TASK-41・TASK-60）、本文と練習データの木の型（`doc`、TASK-60）、未信頼入力の上限の契約（[`limits`]、TASK-199）、履歴の版と判定結果の契約（[`history_version`]、TASK-173）、`content_hash` の求め方（[`content_hash`]、TASK-180）、履歴の入力の列・書き出しの形・履歴の包み（[`history`]、TASK-309）、入力の判定結果・拒否の分類・次の行動・字句化の型（[`judgment`]、TASK-256）を持つ。
//! ほかの型は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod content_hash;
pub mod doc;
pub mod history;
pub mod history_version;
pub mod ir;
pub mod judgment;
pub mod limits;
mod sha256;
