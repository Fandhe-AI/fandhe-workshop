//! 履歴の `ops` を初期状態に順に適用し直して状態を復元する（TASK-316 1/3、SAVE-1）。
//!
//! 履歴の復元の主は、初期状態に入力の列 `ops` を適用し直すことである。包みの写し（`snapshot`）は照合と
//! 表示だけの元で、写しだけを信じて開かない。右のログ（結果のイベント）だけでは、停止・移動・下書きを
//! 一意に戻せないためである。呼び出し元は、写しとの照合（TASK-316 2/3）、版の判定（TASK-174）、読み込み
//! （TASK-310）、入力欄の結線（TASK-275・TASK-347。送信の分類に [`submit_input`] を使う）である。
//! I/O・時刻・乱数を持たない純関数だけを置く。契約の版（`CONTRACT_VERSION`）の対象外。
//!
//! 適用し直しの規則（`ops` の 1 件ごと）。
//!
//! | `ops` の要素 | 適用し直し |
//! |---|---|
//! | `Engine(op)` | [`apply`]（記録側の [`crate::ops_record::apply_recorded`] と同じ遷移） |
//! | `Draft(text)` | [`record_draft`]。`apply` は呼ばず、現在の根の `drafts` だけを更新する |
//! | `Submit(text)` | [`record_submit`] で現在の根の下書きを除いてから、[`submit_input`] で分類した入力を `apply` する |
//! | `Reload` | [`ReplayError::UnsupportedReload`] で拒否する（意味の実装は TASK-320） |
//!
//! 下書きの同期は記録側と同じ関数を通す。作業用の [`OpsRecorder`] を 1 つ持つのは、そのためだけである
//! （規則を二重に定義しない）。
//!
//! 不変条件。
//! 1. 全域で panic しない。決定的で、I/O を持たない
//! 2. `ops` の文字列は、送信の分類（接頭辞で始まるか）以外で解釈しない。`Input` には受け取った値のまま入れる
//!    （試作は trim してから入れていたが、現行の core は受講者の文を原文のまま保つ。SAVE-24）
//! 3. 適用し直せない `ops`（読めない操作プロファイル・記録側が残さない空白だけの送信・未実装の再読み込み）は、
//!    黙って飛ばさず型つきのエラーで拒否する（fail-closed）。エラーは位置だけを持ち、利用者由来の文字列を持たない
//! 4. 連続する `Draft` は記録側が畳むため通常は現れない。あっても状態は最後の値と同じになるので拒否しない
//!
//! 境界（含まないもの）。
//! - 写しとの照合、`replay_mismatch`、書き出し→読み込みの一致（TASK-316 2/3）
//! - 再読み込みの意味（世代 +1・`Reloaded`。TASK-320）
//! - 開始点 `start` からの初期状態の構築（`Control::Begin`。TASK-217・TASK-294）。初期状態は引数で受ける
//! - 件数・長さ・入れ子の上限の検査（`history_size`・`json_depth`・`load_gate`）。呼び出し側が検査済みの `ops` を渡す

use std::fmt;

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::profile::{
    classify_input, read_profile, InputClass, ProfileReadError,
};

use crate::history_envelope::{record_draft, record_submit};
use crate::input::{Input, Op};
use crate::ops_record::{OpsRecorder, SessionOp};
use crate::session::{apply, SessionState};

/// 適用し直せなかった理由。位置（`ops` の添字）だけを持ち、利用者由来の文字列は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReplayError {
    /// IR の `pack.profile` を読めない（送信を分類できない）。
    Profile(ProfileReadError),
    /// 空白だけの送信。記録側は残さないため、書き換えか壊れとして拒否する。
    BlankSubmit {
        /// `ops` の添字。
        index: usize,
    },
    /// 再読み込み。意味の実装は TASK-320 で、それまでは誤った状態を返さないよう拒否する。
    UnsupportedReload {
        /// `ops` の添字。
        index: usize,
    },
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::Profile(_) => f.write_str("pack profile is not readable"),
            ReplayError::BlankSubmit { index } => write!(f, "blank submit at ops[{index}]"),
            ReplayError::UnsupportedReload { index } => {
                write!(f, "reload at ops[{index}] is not supported yet")
            }
        }
    }
}

impl std::error::Error for ReplayError {}

/// 送信の本文を、操作プロファイルの接頭辞でコマンドか自然文に分ける。
///
/// 本文は受け取った値のまま入れる（trim しない）。分けの判定だけは [`classify_input`] が前後の空白を除いて行う。
/// 入力欄の結線と適用し直しが同じ分け方を使うための、core の 1 か所の入口である。
pub fn submit_input(prefix: &str, text: String) -> Input {
    match classify_input(prefix, &text) {
        InputClass::Command => Input::Command(text),
        InputClass::Text => Input::Text(text),
    }
}

/// 初期状態に `ops` を順に適用し直した状態を返す。イベントは戻りの状態の `log` に入っている。
///
/// # Errors
///
/// [`ReplayError`]。`pack.profile` を読めないとき、空白だけの送信・再読み込みを含むとき。
pub fn replay(
    ir: &Ir,
    initial: SessionState,
    ops: &[SessionOp],
) -> Result<SessionState, ReplayError> {
    let profile = read_profile(ir.pack.profile.as_ref()).map_err(ReplayError::Profile)?;
    let prefix = profile.command_prefix;
    let mut rec = OpsRecorder::new();
    let mut state = initial;
    for (index, op) in ops.iter().enumerate() {
        match op {
            HistoryOp::Engine(op) => state = apply(ir, state, op.clone()).0,
            HistoryOp::Draft(text) => record_draft(&mut state, &mut rec, text.clone()),
            HistoryOp::Submit(text) => {
                if !record_submit(&mut state, &mut rec, text.clone()) {
                    return Err(ReplayError::BlankSubmit { index });
                }
                let input = submit_input(&prefix, text.clone());
                state = apply(ir, state, Op::Input(input)).0;
            }
            HistoryOp::Reload => return Err(ReplayError::UnsupportedReload { index }),
        }
    }
    Ok(state)
}
