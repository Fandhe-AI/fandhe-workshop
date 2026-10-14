//! 教材データ・イベント・成果物の契約型を置く crate の入口。
//!
//! core と web が共有する型の置き場で、表示基盤にも core にも依存しない。
//! 現時点は IR の仮の型（`ir`。固定は TASK-41）と、未信頼入力の上限の契約（[`limits`]、TASK-199）を持つ。
//! ほかの型は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod ir;
pub mod limits;
