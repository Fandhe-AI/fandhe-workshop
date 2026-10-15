//! IR・履歴を解釈する前の関門。大きさ、入れ子の深さ、JSON の解釈、型・内容の順に判定する（TASK-157、SAFE-20）。
//!
//! 読み込み口（TASK-154・155・331・545）が、IR・履歴の読み込みでここを呼ぶ。順序の正は SAFE-20 と
//! contract の `LOAD_STAGE_ORDER` である。入力は未信頼として扱い、`serde_json` の解釈の前に大きさと深さで断つ。
//! `serde_json` の再帰の上限（128 段）は、深さの検査の後ろの保険として残る。
//!
//! 意図して固定しないもの: UTF-8 の検査の位置（不正な UTF-8 は現状は解釈の段の失敗になるが、契約ではない）、
//! 履歴の件数の判定（`TooManyOps`。型・内容の段で後続が行う）、受講者向けの文面の選択
//! （呼び出し元が contract の `rejection_message` から選ぶ）、`not_json`・`malformed` の理由コードの文字列。

use fandhe_workshop_contract::limits::{
    LimitReason, LoadEntry, LoadStage, MAX_HISTORY_BYTES, MAX_IR_BYTES,
};
use serde::de::DeserializeOwned;

use crate::json_depth::check_json_depth;

/// 関門が断った理由。入力の内容・`serde_json` のエラー文・実測の段数やバイト数は含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum LoadError {
    /// 大きさ（`TooLarge`）または入れ子の深さ（`TooDeep`）の上限を超えた。
    Limit(LimitReason),
    /// JSON として解釈できない。
    NotJson,
    /// JSON だが、型・内容が合わない。
    Malformed,
}

impl LoadError {
    /// この理由を判定した段。
    #[must_use]
    pub const fn stage(&self) -> LoadStage {
        match self {
            LoadError::Limit(reason) => reason.stage(),
            LoadError::NotJson => LoadStage::JsonParse,
            LoadError::Malformed => LoadStage::TypeAndContent,
        }
    }
}

/// 入口ごとの大きさの上限。
///
/// 履歴の大きさの純関数（TASK-155）との一本化は後続（TASK-154・155）が行う。
const fn max_bytes(entry: LoadEntry) -> u64 {
    match entry {
        LoadEntry::Ir => MAX_IR_BYTES,
        LoadEntry::History => MAX_HISTORY_BYTES,
    }
}

/// 大きさ → 入れ子の深さ → JSON の解釈の 3 段を順に通し、`serde_json::Value` を返す。
///
/// 各段は 1 行で書き、順序を試験（`tests/load_order.rs`）と変異注入で固定する。
///
/// # Errors
///
/// 最初に違反した段の `LoadError`。大きさ・深さは `Limit`、解釈の失敗は `NotJson`。
pub fn parse_json_value(entry: LoadEntry, bytes: &[u8]) -> Result<serde_json::Value, LoadError> {
    if bytes.len() as u64 > max_bytes(entry) {
        return Err(LoadError::Limit(LimitReason::TooLarge));
    }
    check_json_depth(bytes).map_err(LoadError::Limit)?;
    serde_json::from_slice::<serde_json::Value>(bytes).map_err(|_| LoadError::NotJson)
}

/// 3 段に加えて、型・内容（`T` への変換）を最後の段として通す。
///
/// 解釈と型を 2 段に分けるのは、`from_slice::<T>` の 1 回だと構文の誤りより先に型の誤りが返ることがあり、
/// 「解釈 → 型」の順序が崩れるためである。
///
/// # Errors
///
/// 最初に違反した段の `LoadError`。型・内容の不一致は `Malformed`。
pub fn load_json<T: DeserializeOwned>(entry: LoadEntry, bytes: &[u8]) -> Result<T, LoadError> {
    let value = parse_json_value(entry, bytes)?;
    serde_json::from_value::<T>(value).map_err(|_| LoadError::Malformed)
}
