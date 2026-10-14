//! 履歴ファイルの大きさの上限（SAFE-18・TASK-155）の検査。
//!
//! 履歴ファイルは未信頼入力であり、巨大なものを JSON として解釈するとブラウザのメモリと時間を使い切る。
//! そこで判定の順序（`LOAD_STAGE_ORDER`）の最初の段 `LoadStage::Size` として、中身を読まずに
//! 長さだけで断る。上限の値と理由は contract（TASK-199）のものを使い、ここでは再定義しない。
//!
//! 引数を `u64` の長さにするのは、契約の整数の規約（バイト数は `u64`。wasm32 の `usize` は 32 bit）に
//! 従うためと、呼び出し側が `File.size` や保存値の長さを、中身を wasm のメモリへ読み込む前に渡せるように
//! するためである。呼び出し元は履歴の読み込み口（TASK-331・TASK-330）で、中身を読む前に呼ぶ。
//!
//! 判定は副作用の無い純関数で、保存済みの履歴を書き換えない。文面は持たず、表示側が
//! `rejection_message(LoadEntry::History, reason)` で引く。

use fandhe_workshop_contract::limits::{LimitReason, MAX_HISTORY_BYTES};

/// 履歴ファイルの長さ（バイト）だけで判定する。ちょうど上限は通し、超えたら [`LimitReason::TooLarge`]。
///
/// # Errors
///
/// `len` が [`MAX_HISTORY_BYTES`] を超えるとき、[`LimitReason::TooLarge`] を返す。
pub const fn check_history_size(len: u64) -> Result<(), LimitReason> {
    if len > MAX_HISTORY_BYTES {
        Err(LimitReason::TooLarge)
    } else {
        Ok(())
    }
}

/// バイト列の長さで判定する便宜の関数。中身は読まない。
///
/// # Errors
///
/// 長さが上限を超えるとき、または長さを `u64` に変換できないとき（安全側に倒す）、
/// [`LimitReason::TooLarge`] を返す。
pub fn check_history_bytes(bytes: &[u8]) -> Result<(), LimitReason> {
    match u64::try_from(bytes.len()) {
        Ok(len) => check_history_size(len),
        Err(_) => Err(LimitReason::TooLarge),
    }
}
