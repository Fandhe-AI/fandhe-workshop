//! `ops`・写し・4 軸の版を履歴の包みに組み立てる（TASK-315 2/2、SAVE-2・SAVE-18・SAVE-24、MOD-4）。
//!
//! 保存・書き出しの入口（web の TASK-317・TASK-330）が、記録器 [`OpsRecorder`] の列と
//! 実行環境の写し [`SessionState`] から契約どおりの包みを作るために呼ぶ。I/O・時刻・乱数を持たない純関数だけを置く。
//! 契約の型は `fandhe_workshop_contract::history::HistoryEnvelope`（版 1）で、ここはそれに core の
//! 型を束縛し（[`SessionEnvelope`]）、版の欄を埋めるだけである。契約の版（`CONTRACT_VERSION`）の対象外。
//!
//! 不変条件。
//! 1. `ops` と写しは受け取った値のまま包む。並び替え・補正・切り詰めをしない。ログの並び、`log_len`、
//!    チェックポイントの持ち主の根（`owner`）は組み立てで変わらない（SAVE-2）
//! 2. 版の欄は IR から求め、求められなければ既定値に倒さず [`BuildError`] で返す（fail-closed）
//! 3. 最初の開始点 `start` は `ops` からも写しからも導けないため引数で受け、IR に無い ID は拒否する
//!
//! 下書きの同期（SAVE-18・MOD-4）。[`record_draft`]・[`record_submit`] は `ops` への記録と、
//! 写しの現在の根の下書き `drafts` の更新を一緒に行う。`ops` の末尾の `{"draft": …}` と
//! 写しの下書きが食い違わないようにするためである。
//!
//! 適用し直す側（TASK-316）への約束。
//! - 根は現在の Step の `StepUid::root` で決める。入れ子の実体では `Instance.id`（最後の `/` より前）になる（#1413）
//! - 現在の Step が無い（開始前）間の下書き・送信は `ops` にだけ残り、`drafts` は変えない
//! - 空文字列の下書きは `""` のまま `drafts` に入る。記録した送信だけが、その時点の根の欄を除く
//!   （空白だけの送信は記録されず、`drafts` も変わらない）
//!
//! 境界（含まないもの）。
//! - `ops` の適用し直しによる復元（[`crate::history_replay`]、TASK-316 1/3）、写しとの照合・`replay_mismatch`（[`crate::history_replay::verify_envelope`]、TASK-316 2/3）
//! - 読み込み・拒否理由コード・判定の順序（TASK-310・TASK-174）
//! - ファイルへの書き出しのバイト列（[`crate::history_export`]、TASK-330 1/2）、ダウンロード（web）、保存先（TASK-317）
//! - 再読み込みの記録（SAVE-4）、件数・長さの上限の検査（読み込み口の責務。保存の前の判定は [`crate::history_limit`]）
//!
//! 文字列（`pack_id`・`start`・下書き・送信）は利用者・教材由来で信頼できない。保持するだけで加工せず、
//! 表示時のエスケープは web 層が行う。下書きは書き出すと相手に渡る（SAVE-18）。書き出す前の案内は画面の責務である。

use std::fmt;

use fandhe_workshop_contract::content_hash::{content_hash, ContentHashError};
use fandhe_workshop_contract::history::HistoryEnvelope;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;

use crate::addr::StepUid;
use crate::input::Op;
use crate::ops_record::{OpsRecorder, SessionOp};
use crate::session::SessionState;
use crate::version_record::{record_versions, RecordError};

/// core の [`Op`]・[`SessionState`] を束縛した履歴の包み。
pub type SessionEnvelope = HistoryEnvelope<Op, SessionState>;

/// 包みを組み立てられなかった理由。値（開始点の文字列など）は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum BuildError {
    /// `content_hash` を求められない。
    ContentHash(ContentHashError),
    /// 4 軸の版を書けない。
    Versions(RecordError),
    /// `start` が IR の開始点に無い。
    UnknownStart,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::ContentHash(e) => write!(f, "content hash: {e}"),
            BuildError::Versions(e) => write!(f, "versions: {e}"),
            BuildError::UnknownStart => f.write_str("start is not in the IR starts"),
        }
    }
}

impl std::error::Error for BuildError {}

/// IR・最初の開始点・`ops`・写しから包みを組み立てる（現在のエンジンの版で書く）。
///
/// # Errors
///
/// [`BuildError`]。`content_hash` や版を求められないとき、`start` が IR に無いとき。
pub fn build_envelope(
    ir: &Ir,
    start: &str,
    ops: Vec<SessionOp>,
    snapshot: SessionState,
) -> Result<SessionEnvelope, BuildError> {
    let hash = content_hash(ir).map_err(BuildError::ContentHash)?;
    let versions = record_versions(ir, &hash).map_err(BuildError::Versions)?;
    build_envelope_with(ir, versions, start, ops, snapshot)
}

/// 版の欄を呼び出し側が渡して組み立てる（試験や、別の版を模す用途の入口）。
///
/// # Errors
///
/// `start` が IR の開始点に無いとき [`BuildError::UnknownStart`]。
pub fn build_envelope_with(
    ir: &Ir,
    versions: HistoryVersions,
    start: &str,
    ops: Vec<SessionOp>,
    snapshot: SessionState,
) -> Result<SessionEnvelope, BuildError> {
    if !ir.starts.iter().any(|s| s.id == start) {
        return Err(BuildError::UnknownStart);
    }
    Ok(HistoryEnvelope::new(
        ir.pack.id.clone(),
        versions,
        start.to_owned(),
        ops,
        snapshot,
    ))
}

/// 下書きを `ops` に記録し、写しの現在の根の下書きも同じ値にする（MOD-4・SAVE-18）。
/// 現在の Step が無いときは `ops` にだけ記録する。
pub fn record_draft(state: &mut SessionState, rec: &mut OpsRecorder, text: String) {
    if let Some(root) = state.run.current.as_ref().map(StepUid::root) {
        state.drafts.insert(root, text.clone());
    }
    rec.draft(text);
}

/// 送信を `ops` に記録し、記録した場合は現在の根の下書きだけを写しから除く。
/// 戻り値は [`OpsRecorder::submit`] と同じ（空白だけなら `false` で、何も変えない）。
pub fn record_submit(state: &mut SessionState, rec: &mut OpsRecorder, text: String) -> bool {
    let recorded = rec.submit(text);
    if recorded {
        if let Some(root) = state.run.current.as_ref().map(StepUid::root) {
            state.drafts.remove(&root);
        }
    }
    recorded
}
