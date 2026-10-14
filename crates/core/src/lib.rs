//! 教材実行の核（状態遷移・完了判定）を置く crate の入口。
//!
//! web・表示基盤に依存しない。web と pack から呼ばれる。
//! 現時点は IR の読み込みの入口（`ir_load`）だけを持ち、実行の中身は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod ir_load;

pub use ir_load::{load_ir, IrLoadError};
