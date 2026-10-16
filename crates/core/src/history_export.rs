//! 履歴の包みを書き出すバイト列にする（TASK-330 1/2、SAVE-16・SAVE-17）。
//!
//! 呼び出し元は web のダウンロード（`crates/web` の `history_download`）と、保存先へ積む文字列を作る層である。
//! ブラウザの保存領域はオリジンごとに分かれるため、受講者が履歴を持ち運ぶ手段はファイルだけになる（SAVE-16）。
//! ここは、その書き出しの中身（バイト列）を作る純関数だけを持つ。I/O・時刻・乱数を持たない。
//!
//! 不変条件。
//! 1. 形は契約の [`EXPORT_FORM`]（`ops` と写しの両方。D-52・SAVE-17）。`ops` だけの形は作れない。
//!    値が増えたときは、下の網羅 match がコンパイルで知らせる
//! 2. 直列化は compact な 1 行の JSON である（PoC-6 の測定の基点と同じ形）。契約の試料（整形つき）は試験用の正の形で、
//!    別のものとして扱う
//! 3. 同じ包みからは同じバイト列が出る（`SessionState` の表は `BTreeMap`、欄は宣言順）。
//!    日時を入れないのも、この決定性のためである
//! 4. 失敗したら途中のバイト列を返さず [`ExportError`] で返す（fail-closed）
//!
//! 大きさの上限（推論）。spec に書き出し側の上限の記述は無い。書き出しは受講者の唯一の写しを持ち出す手段のため、
//! 読み込みの上限（[`MAX_HISTORY_BYTES`]・[`MAX_HISTORY_OPS`]）を超えても止めず、[`ExportReport`] の印で知らせる。
//! 案内は画面の責務（TASK-531）である。
//!
//! 含まないもの: ファイル名・ダウンロード（web）、読み込み（[`crate::history_load`]、TASK-330 2/2）、
//! 未送信の下書きの案内（SHELL-22）。履歴は下書きを含む受講者のデータであり、値を記録・出力しない。

use std::fmt;

use fandhe_workshop_contract::history::{ExportForm, EXPORT_FORM};
use fandhe_workshop_contract::limits::{MAX_HISTORY_BYTES, MAX_HISTORY_OPS};

use crate::history_envelope::SessionEnvelope;

/// 書き出しの形を契約と照合する。値が増えたら網羅 match が壊れ、コンパイルで気づく。
const _: () = match EXPORT_FORM {
    ExportForm::OpsAndSnapshot => (),
};

/// 書き出せなかった理由。値は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportError {
    /// JSON に直列化できない。
    Encode,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Encode => f.write_str("history could not be encoded"),
        }
    }
}

impl std::error::Error for ExportError {}

/// 書き出しの大きさの報告。読み込みの上限を超えるかどうかの印を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportReport {
    /// 書き出すバイト数。
    pub bytes: usize,
    /// `ops` の件数。
    pub ops: usize,
    /// バイト数が読み込みの上限（[`MAX_HISTORY_BYTES`]）を超える。
    pub exceeds_size_limit: bool,
    /// 件数が読み込みの上限（[`MAX_HISTORY_OPS`]）を超える。
    pub exceeds_ops_limit: bool,
}

/// 包みを compact な JSON のバイト列にする。
///
/// # Errors
///
/// 直列化に失敗したとき [`ExportError::Encode`]。途中までのバイト列は返さない。
pub fn export_bytes(env: &SessionEnvelope) -> Result<Vec<u8>, ExportError> {
    serde_json::to_vec(env).map_err(|_| ExportError::Encode)
}

/// 書き出すバイト列と包みから、大きさの報告を作る。上限を超えても書き出し自体は拒否しない。
#[must_use]
pub fn export_report(bytes: &[u8], env: &SessionEnvelope) -> ExportReport {
    ExportReport {
        bytes: bytes.len(),
        ops: env.ops.len(),
        exceeds_size_limit: u64::try_from(bytes.len()).map_or(true, |n| n > MAX_HISTORY_BYTES),
        exceeds_ops_limit: env.ops.len() > MAX_HISTORY_OPS,
    }
}
