//! 教材実行の核（状態遷移・完了判定）を置く crate の入口。
//!
//! web・表示基盤に依存しない。web と pack から呼ばれる。
//! 現時点は状態変化の提供元の境界（[`provider`]、TASK-187）を持つ。ほかは後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod provider;
