//! 教材実行の核（状態遷移・完了判定）を置く crate の入口。
//!
//! web・表示基盤に依存しない。web と pack から呼ばれる。
//! 現時点は IR の読み込みの入口（`ir_load`）と、状態変化の提供元の境界（[`provider`]、TASK-187）、
//! 履歴ファイルの大きさの検査（[`history_size`]、TASK-155）、解釈の前の入れ子の深さの検査
//! （[`json_depth`]、TASK-157）、それを含む判定の順序の関門（[`load_gate`]、TASK-157）を持つ。
//! ほかは後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod history_size;
pub mod ir_load;
pub mod json_depth;
pub mod load_gate;
pub mod provider;

pub use ir_load::{check_ir_limits, load_ir, IrLoadError};
