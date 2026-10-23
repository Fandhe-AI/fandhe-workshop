//! 結合試験で共有する部品の入口。
//!
//! 各試験ファイルが `mod common;` で取り込む。ファイルごとに使う部品が異なるため、
//! 未使用の警告だけをここで抑える。

#![allow(dead_code, reason = "試験ファイルごとに使う共通部品が異なるため")]

pub mod contract_sample;
pub mod history_sample;
pub mod json_input;
pub mod mem_store;
pub mod op_gen;
pub mod op_props;
pub mod property;
pub mod record_path;
pub mod text_table;
pub mod tick;
pub mod trace;

/// 持ち主の規則（`owner_of`）が `root_of` に落ちるだけの最小の IR（呼び出しの Step を持たない）。
pub fn min_ir() -> fandhe_workshop_contract::ir::Ir {
    let path =
        contract_sample::testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    fandhe_workshop_core::load_ir(&std::fs::read(path).expect("read min-3step")).expect("load_ir")
}
