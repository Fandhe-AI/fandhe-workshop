//! 操作を履歴の `ops` に記録しながら進める実行環境（TASK-319 1/3、SAVE-2・SAVE-3・SAVE-28）。
//!
//! 呼び出し元は web のホスト（受講中の操作の記録と、離れる直前の保存用の包みの組み立て）と、起動の分岐
//! （[`crate::history_boot`]）である。web の表示層は状態を直に触れない（`crates/web/tests/no_progress.rs`）ため、
//! `ops` への記録と、写しの下書き `drafts` の同期（[`record_draft`]・[`record_submit`]）はここに集める。
//! 進行の規則は [`Session`] が持ち、この型は記録を足すだけで、世代・Step・処理中の判定は変えない。
//!
//! 不変条件。
//! 1. エンジンの操作・送信・下書き・再読み込みは、結果（受付・拒否・無視）にかかわらず記録する（[`OpsRecorder`] の規則）
//! 2. `ops` が 0 件のときは保存用の包みを作らない（[`RecordedSession::envelope_for_save`]）。
//!    最初の変更まで保存しないことが、未操作の再読み込みで世代が上がらないことの根拠である（SAVE-28）
//! 3. 包みを組み立てられないとき（開始点が IR に無い等）は保存しない側に倒す（fail-closed）
//!
//! 文字列（下書き・送信の本文）は利用者由来で信頼できない。加工せず保持し、表示時のエスケープは web 層が行う。

use fandhe_workshop_contract::ir::Ir;

use crate::addr::{Root, StepUid};
use crate::event::Event;
use crate::history_envelope::{build_envelope, record_draft, record_submit, SessionEnvelope};
use crate::history_replay::submit_input;
use crate::input::Op;
use crate::ops_record::{OpsRecorder, SessionOp};
use crate::session::Session;

/// 記録つきの実行環境。実行環境・記録器・最初の開始点の ID を束ねる。
#[derive(Debug, Clone)]
pub struct RecordedSession {
    session: Session,
    recorder: OpsRecorder,
    start: String,
}

impl RecordedSession {
    /// 記録が空の状態で作る。`start` は最初の開始点の ID で、保存用の包みに入る。
    pub fn new(session: Session, start: impl Into<String>) -> Self {
        Self::with_recorder(session, OpsRecorder::new(), start)
    }

    /// 既存の記録の続きとして作る（保存済みの履歴を開いた直後）。
    pub fn with_recorder(
        session: Session,
        recorder: OpsRecorder,
        start: impl Into<String>,
    ) -> Self {
        Self {
            session,
            recorder,
            start: start.into(),
        }
    }

    /// 実行環境（読み取りのみ）。
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// 教材の最初の開始点の ID（空の `starts` では空文字列。このとき包みは作れない）。
    pub fn start(&self) -> &str {
        &self.start
    }

    /// 記録済みの `ops`。
    pub fn ops(&self) -> &[SessionOp] {
        self.recorder.ops()
    }

    /// 教材の最初の開始点の ID を IR から求める。無ければ空文字列。
    pub fn first_start(ir: &Ir) -> String {
        ir.starts.first().map(|s| s.id.clone()).unwrap_or_default()
    }

    /// エンジンの操作を記録してから適用し、今回足されたイベントを返す。
    pub fn apply(&mut self, op: Op) -> &[Event] {
        self.recorder.engine(op.clone());
        self.session.apply(op)
    }

    /// 送信を記録してから適用し、今回足されたイベントを返す。空白だけの送信は記録も適用もしない。
    ///
    /// 下書きを除くのは `apply` の前（送った時点の根）で、着地した根ではない。順序を変えない（S-7）。
    ///
    /// `prefix` は操作プロファイルのコマンド接頭辞。分類は適用し直し（`replay`）と同じ [`submit_input`] を使う。
    pub fn submit(&mut self, prefix: &str, text: String) -> &[Event] {
        if !record_submit(self.session.state_mut(), &mut self.recorder, text.clone()) {
            return &[];
        }
        self.session.apply(Op::Input(submit_input(prefix, text)))
    }

    /// 下書きの置き換えを記録する（写しの現在の根の下書きも同じ値にする）。
    pub fn draft(&mut self, text: String) {
        record_draft(self.session.state_mut(), &mut self.recorder, text);
    }

    /// 現在の根の下書き（MOD-4）。[`Session::current_draft`] と同じ。
    pub fn current_draft(&self) -> Option<&str> {
        self.session.current_draft()
    }

    /// その Step の実体で開いたヒントの段数（GUIDE-7）。[`Session::hints_opened_at`] と同じ。
    pub fn hints_opened_at(&self, uid: &StepUid) -> u32 {
        self.session.hints_opened_at(uid)
    }

    /// 根（実体）ごとのヒントの段数の合計（GUIDE-7）。[`Session::hints_opened_in`] と同じ。
    pub fn hints_opened_in(&self, root: &Root) -> u32 {
        self.session.hints_opened_in(root)
    }

    /// 再読み込みを記録してから行い、今回足されたイベントを返す（SAVE-4）。
    pub fn reload(&mut self) -> &[Event] {
        self.recorder.reload();
        self.session.reload()
    }

    /// 保存用の包み。`ops` が 0 件のとき、組み立てられないときは `None`（保存しない）。
    pub fn envelope_for_save(&self) -> Option<SessionEnvelope> {
        if self.recorder.is_empty() {
            return None;
        }
        build_envelope(
            self.session.ir(),
            &self.start,
            self.recorder.ops().to_vec(),
            self.session.state().clone(),
        )
        .ok()
    }
}
