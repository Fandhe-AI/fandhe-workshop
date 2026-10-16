//! 結合試験で共有する部品の入口。
//!
//! 各試験ファイルが `mod common;` で取り込む。ファイルごとに使う部品が異なるため、
//! 未使用の警告だけをここで抑える。

#![allow(dead_code, reason = "試験ファイルごとに使う共通部品が異なるため")]

pub mod contract_sample;
pub mod history_sample;
pub mod json_input;
pub mod op_gen;
pub mod op_props;
pub mod property;
pub mod tick;
