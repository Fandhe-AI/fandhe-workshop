//! 受講者の操作を履歴の入力の列 `ops` に追記する記録（TASK-315 1/2、SAVE-2・SAVE-24）。
//!
//! 保存・適用し直しによる復元・書き出しは、`ops` の並びを正とする。その並びを作る側の規則をここに置く。
//! 状態は進めない（進めるのは [`crate::session::apply`]）。呼び出し元は、入力欄の結線（TASK-275）、
//! 包みの組み立て（[`crate::history_envelope`]、TASK-315 2/2）、試験である。
//!
//! 記録の規則（出所は `fandhe_workshop_contract::history` の記録側の規則）。
//! 1. 連続する下書きは最後の 1 件に畳む。`Submit` や `Engine` を挟むと畳まない（[`OpsRecorder::draft`]）
//! 2. 空白だけの送信は記録しない。記録する本文は受け取った値のまま、切り詰め・trim・分類をしない
//!    （[`OpsRecorder::submit`]、SAVE-24。20,000 文字の送信は 20,000 文字のまま残る）
//! 3. エンジンの操作は結果（受付・拒否・無視）にかかわらず追記する（状態を変えうるため。[`OpsRecorder::engine`]）
//! 4. 並びは操作の順のまま変えない
//! 5. 再読み込み（SAVE-4）は結果にかかわらず `Reload` として 1 件追記する。下書きは畳まない（[`OpsRecorder::reload`]、[`reload_recorded`]）
//!
//! Enter 専用の記録は作らない。送信の入口は [`OpsRecorder::submit`] の 1 つで、送信の経路（キー・ボタン）を
//! 引数に取らない。記録の種類は contract の `HistoryOp` が決め、Enter に当たる種類は無い。
//!
//! 境界（含まないもの）。
//! - 包みの組み立てと、写しの `drafts` との同期（[`crate::history_envelope`]、TASK-315 2/2）
//! - 送信の分類（コマンドか自然文か）と適用（[`crate::history_replay::submit_input`]、TASK-275・TASK-316。操作プロファイルで行う）
//! - 件数・長さの上限の検査（読み込み口の責務。記録で切り詰めると SAVE-24 に反する。上限を超えても記録は止めない。
//!   保存の前の判定は [`crate::history_limit`]）
//!
//! 文字列は利用者由来で信頼できない。記録器は加工も解釈もせず保持するだけで、表示時のエスケープは web 層が行う。
//! 契約の版（`CONTRACT_VERSION`）の対象外である。

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;

use crate::event::Event;
use crate::input::Op;
use crate::session::{apply, reload, SessionState};

/// core の [`Op`] を束縛した履歴の入力の列の 1 件（仕様の `SessionOp`）。
pub type SessionOp = HistoryOp<Op>;

/// 受講者の操作を `ops` に追記する記録器。記録の規則だけを持ち、状態は進めない。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OpsRecorder {
    ops: Vec<SessionOp>,
}

impl OpsRecorder {
    /// 空の記録器を作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 読み込んだ `ops` から作る（保存済みの履歴の続きを記録するため。起動時の復元 [`crate::history_boot`]）。
    pub fn from_ops(ops: Vec<SessionOp>) -> Self {
        Self { ops }
    }

    /// 下書きの置き換えを記録する。直前が `Draft` なら置き換え、そうでなければ追記する。
    /// 空文字列も記録する（入力欄を空にした操作も状態の一部）。
    pub fn draft(&mut self, text: String) {
        if let Some(HistoryOp::Draft(prev)) = self.ops.last_mut() {
            *prev = text;
        } else {
            self.ops.push(HistoryOp::Draft(text));
        }
    }

    /// 下書きの送信（Enter・送信ボタンの共通の入口）を記録する。
    /// 空白だけなら記録せず `false` を返す。記録する本文は受け取った値のまま。
    pub fn submit(&mut self, text: String) -> bool {
        if text.trim().is_empty() {
            return false;
        }
        self.ops.push(HistoryOp::Submit(text));
        true
    }

    /// エンジンの操作を記録する。結果（受付・拒否・無視）を見ない。
    pub fn engine(&mut self, op: Op) {
        self.ops.push(HistoryOp::Engine(op));
    }

    /// 再読み込みを記録する（SAVE-4）。直前が `Draft` でも畳まず、結果を見ない。
    pub fn reload(&mut self) {
        self.ops.push(HistoryOp::Reload);
    }

    /// 記録済みの列。
    pub fn ops(&self) -> &[SessionOp] {
        &self.ops
    }

    /// 列を取り出す（包みの組み立てのため）。
    pub fn into_ops(self) -> Vec<SessionOp> {
        self.ops
    }

    /// 記録の件数。
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    /// 記録が空か。
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }
}

/// 操作を記録してから [`apply`] する。記録は適用の結果に左右されない（拒否・無視も残る）。
///
/// 記録を先に行うのは、適用が拒否・無視でも `ops` に残すため。`apply` は全域で、記録だけが残る経路は無い。
pub fn apply_recorded(
    ir: &Ir,
    state: SessionState,
    recorder: &mut OpsRecorder,
    op: Op,
) -> (SessionState, Vec<Event>) {
    recorder.engine(op.clone());
    apply(ir, state, op)
}

/// 再読み込みを記録してから [`reload`] する。記録は結果（無変化・`Warning`）に左右されない。
///
/// 適用し直し（[`crate::history_replay::replay`]）が同じ [`reload`] を通るので、結果は決定的に一致する。
pub fn reload_recorded(
    state: SessionState,
    recorder: &mut OpsRecorder,
) -> (SessionState, Vec<Event>) {
    recorder.reload();
    reload(state)
}
