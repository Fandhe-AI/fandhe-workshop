//! 履歴の `ops` を初期状態に順に適用し直して状態を復元する（TASK-316 1/3、SAVE-1）。
//!
//! 履歴の復元の主は、初期状態に入力の列 `ops` を適用し直すことである。包みの写し（`snapshot`）は照合と
//! 表示だけの元で、写しだけを信じて開かない。右のログ（結果のイベント）だけでは、停止・移動・下書きを
//! 一意に戻せないためである。[`verify_envelope`] が適用し直した結果を写しと照合し、一致したときだけ
//! 適用し直した状態（写しではない）を返す。食い違いは書き換え・壊れとして [`VerifyError::load_reason`] が
//! `replay_mismatch` に対応づける。「表示だけ」で開く場合の元は [`view_only_source`] が写しを読み取り専用で渡す。
//! 呼び出し元は、版の判定（TASK-174）、読み込み（TASK-310）、入力欄の結線（TASK-275・TASK-347。送信の分類に
//! [`submit_input`] を使う）である。
//! I/O・時刻・乱数を持たない純関数だけを置く。契約の版（`CONTRACT_VERSION`）の対象外。
//!
//! 適用し直しの規則（`ops` の 1 件ごと）。
//!
//! | `ops` の要素 | 適用し直し |
//! |---|---|
//! | `Engine(op)` | [`apply`]（記録側の [`crate::ops_record::apply_recorded`] と同じ遷移） |
//! | `Draft(text)` | [`record_draft`]。`apply` は呼ばず、現在の根の `drafts` だけを更新する |
//! | `Submit(text)` | [`record_submit`] で現在の根の下書きを除いてから、[`submit_input`] で分類した入力を `apply` する |
//! | `Reload` | [`reload`]（世代 +1・`Reloaded`。記録側の [`crate::ops_record::reload_recorded`] と同じ遷移。TASK-320） |
//!
//! 途中を観測したい呼び出し元は [`replay_with`] を使う（同じ規則に前後の hook を足したもの）。
//!
//! 下書きの同期は記録側と同じ関数を通す。作業用の [`OpsRecorder`] を 1 つ持つのは、そのためだけである
//! （規則を二重に定義しない）。
//!
//! 不変条件。
//! 1. 全域で panic しない。決定的で、I/O を持たない
//! 2. `ops` の文字列は、送信の分類（接頭辞で始まるか）以外で解釈しない。`Input` には受け取った値のまま入れる
//!    （試作は trim してから入れていたが、現行の core は受講者の文を原文のまま保つ。SAVE-24）
//! 3. 適用し直せない `ops`（読めない操作プロファイル・記録側が残さない空白だけの送信）は、
//!    黙って飛ばさず型つきのエラーで拒否する（fail-closed）。エラーは位置だけを持ち、利用者由来の文字列を持たない
//! 4. 連続する `Draft` は記録側が畳むため通常は現れない。あっても状態は最後の値と同じになるので拒否しない
//!
//! 境界（含まないもの）。
//! - 版の軸が違う場合の `replay_diverged`（表示だけ）への読み替えと、どの理由が表示だけになるかの線引き（TASK-174）
//! - 読み込みの全段の順序（TASK-310 3/3・TASK-331）。`pack_id`・`versions`・`start`・`format` は見ない
//! - 開始点 `start` からの初期状態の構築（`Control::Begin`。TASK-217・TASK-294）。初期状態は引数で受ける
//! - 件数・長さ・入れ子の上限の検査（`history_size`・`json_depth`・`load_gate`）。呼び出し側が検査済みの `ops` を渡す

use std::fmt;

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_contract::profile::{
    classify_input, read_profile, InputClass, ProfileReadError,
};

use crate::event::Event;
use crate::history_envelope::{record_draft, record_submit, SessionEnvelope};
use crate::input::{Input, Op};
use crate::ops_record::{OpsRecorder, SessionOp};
use crate::session::{apply, reload, SessionState};

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
}

impl fmt::Display for ReplayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReplayError::Profile(_) => f.write_str("pack profile is not readable"),
            ReplayError::BlankSubmit { index } => write!(f, "blank submit at ops[{index}]"),
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
/// [`ReplayError`]。`pack.profile` を読めないとき、空白だけの送信を含むとき。
pub fn replay(
    ir: &Ir,
    initial: SessionState,
    ops: &[SessionOp],
) -> Result<SessionState, ReplayError> {
    replay_with(ir, initial, ops, |_, _| (), |(), _| ())
}

/// [`replay`] と同じ規則で適用し直しながら、`apply` を通す操作の前後に hook を呼ぶ。
///
/// 規則を二重に定義せず、適用し直しの途中を観測したい呼び出し元（[`crate::lo_observe`]）のための口である。
/// `pre` は `apply` の直前に操作の前の状態と操作を受け取り、小さな値 `P` を返す。`post` は `apply` の直後に
/// その `P` と、その操作が出したイベントを受け取る。操作の前の状態を丸ごと複製せずに済ませるため、
/// 状態の読み取りは `pre` の中で済ませる（全体で `ops` の件数に線形）。
///
/// hook を呼ぶのは `apply` を通す操作（`Engine`・`Submit`）だけである。`Draft` は下書きだけを変え、
/// `Reload` は `Reloaded` だけを出し、どちらも確認の観測を生まないため hook を呼ばない。
///
/// # Errors
///
/// [`replay`] と同じ。
pub fn replay_with<P>(
    ir: &Ir,
    initial: SessionState,
    ops: &[SessionOp],
    mut pre: impl FnMut(&SessionState, &Op) -> P,
    mut post: impl FnMut(P, &[Event]),
) -> Result<SessionState, ReplayError> {
    let profile = read_profile(ir.pack.profile.as_ref()).map_err(ReplayError::Profile)?;
    let prefix = profile.command_prefix;
    let mut rec = OpsRecorder::new();
    let mut state = initial;
    for (index, op) in ops.iter().enumerate() {
        match op {
            HistoryOp::Engine(op) => {
                let p = pre(&state, op);
                let (next, events) = apply(ir, state, op.clone());
                state = next;
                post(p, &events);
            }
            HistoryOp::Draft(text) => record_draft(&mut state, &mut rec, text.clone()),
            HistoryOp::Submit(text) => {
                if !record_submit(&mut state, &mut rec, text.clone()) {
                    return Err(ReplayError::BlankSubmit { index });
                }
                let op = Op::Input(submit_input(&prefix, text.clone()));
                let p = pre(&state, &op);
                let (next, events) = apply(ir, state, op);
                state = next;
                post(p, &events);
            }
            HistoryOp::Reload => state = reload(state).0,
        }
    }
    Ok(state)
}

/// 写しと食い違った最初の欄。欄の名前だけを持ち、値（ログ・下書きの文字列）は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SnapshotField {
    /// 実行の位置（`run`）。
    Run,
    /// ログ（`log`）。
    Log,
    /// 入力欄の状態（`entry`）。
    Entry,
    /// チェックポイント（`checkpoints`）。
    Checkpoints,
    /// 次のチェックポイントの番号（`next_checkpoint`）。
    NextCheckpoint,
    /// 下書き（`drafts`）。
    Drafts,
    /// 開いたヒント（`hints_opened`）。
    HintsOpened,
}

/// 2 つの状態で最初に食い違う欄を固定の順で返す。一致すれば `None`。
///
/// 全欄を網羅の分割代入で受けるため、[`SessionState`] に欄が増えると、ここでコンパイルエラーになり
/// 照合の抜けに気付ける。
pub fn first_difference(a: &SessionState, b: &SessionState) -> Option<SnapshotField> {
    let SessionState {
        run,
        log,
        entry,
        checkpoints,
        next_checkpoint,
        drafts,
        hints_opened,
    } = a;
    if *run != b.run {
        Some(SnapshotField::Run)
    } else if *log != b.log {
        Some(SnapshotField::Log)
    } else if *entry != b.entry {
        Some(SnapshotField::Entry)
    } else if *checkpoints != b.checkpoints {
        Some(SnapshotField::Checkpoints)
    } else if *next_checkpoint != b.next_checkpoint {
        Some(SnapshotField::NextCheckpoint)
    } else if *drafts != b.drafts {
        Some(SnapshotField::Drafts)
    } else if *hints_opened != b.hints_opened {
        Some(SnapshotField::HintsOpened)
    } else {
        None
    }
}

/// 照合で開けなかった理由。位置・欄名だけを持ち、利用者由来の文字列は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VerifyError {
    /// 適用し直せなかった。
    Replay(ReplayError),
    /// 適用し直した結果が写しと食い違った。
    Mismatch {
        /// 最初に食い違った欄。
        field: SnapshotField,
    },
}

impl VerifyError {
    /// 版の軸がすべて同じ場合の読み込みの理由。
    ///
    /// 食い違いと、記録側が残さない空白だけの送信は、書き換え・壊れとして `replay_mismatch` にする。
    /// 教材側の不備（読めない操作プロファイル）は `None` で、
    /// 呼び出し元は開かない（fail-closed）。版の軸が違う場合の `replay_diverged` への読み替えは TASK-174 の責務。
    pub fn load_reason(&self) -> Option<LoadReason> {
        match self {
            VerifyError::Mismatch { .. } | VerifyError::Replay(ReplayError::BlankSubmit { .. }) => {
                Some(LoadReason::ReplayMismatch)
            }
            VerifyError::Replay(_) => None,
        }
    }
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::Replay(e) => write!(f, "replay failed: {e}"),
            VerifyError::Mismatch { field } => {
                write!(f, "replayed state differs from the snapshot in {field:?}")
            }
        }
    }
}

impl std::error::Error for VerifyError {}

/// 包みの `ops` を初期状態に適用し直し、写しと照合する。一致したときだけ、適用し直した状態を返す。
///
/// 返すのは写しではなく適用し直した状態である（写しだけを信じて開かない）。`envelope` は読み込み口
/// （`load_gate`・`history_size`）で件数・長さ・深さを検査済みとする。
///
/// # Errors
///
/// [`VerifyError`]。適用し直せないとき、結果が写しと食い違うとき。どちらも開いてはならない。
pub fn verify_envelope(
    ir: &Ir,
    initial: SessionState,
    envelope: &SessionEnvelope,
) -> Result<SessionState, VerifyError> {
    let replayed = replay(ir, initial, &envelope.ops).map_err(VerifyError::Replay)?;
    match first_difference(&replayed, &envelope.snapshot) {
        None => Ok(replayed),
        Some(field) => Err(VerifyError::Mismatch { field }),
    }
}

/// 「表示だけ」で開く場合の元。写しへの読み取り専用の参照だけを持ち、実行の状態へ変換する関数は持たない。
///
/// 適用し直さず、実行の状態としても開かない。ただし `snapshot()` の戻りの `SessionState` は `Clone` なので、
/// 複製を `Session::from_state` へ渡せば実行を再開できてしまい、これは型では防げない。表示だけで開く約束は、
/// 呼び出し側が守る運用である（表示用の型への分離は別の課題）。どの理由が表示だけになるかの線引きは TASK-174 の責務である。
#[derive(Debug, Clone, Copy)]
pub struct ViewOnlySource<'a>(&'a SessionState);

impl<'a> ViewOnlySource<'a> {
    /// 表示の元にする写し。
    pub fn snapshot(&self) -> &'a SessionState {
        self.0
    }
}

/// 包みの写しを、表示だけの元として渡す。
pub fn view_only_source(envelope: &SessionEnvelope) -> ViewOnlySource<'_> {
    ViewOnlySource(&envelope.snapshot)
}
