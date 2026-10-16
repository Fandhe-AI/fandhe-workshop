//! 履歴の `ops` を記録しながら `apply` で状態を進める「記録の経路」（試験の共通部品）。
//!
//! 呼び出し元は `history_replay.rs`（適用し直しの試験）と `history_open.rs`（版の判定の試験）で、
//! どちらも写しと `ops` を同じ手順で作るために使う。

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::history_envelope::{record_draft, record_submit};
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::ops_record::{apply_recorded, OpsRecorder};
use fandhe_workshop_core::session::{apply, SessionState};

use super::op_gen::{Harness, OpSpec};

/// 記録の経路。下書きと送信を記録しながら `apply` で状態を進める（実行時の入力欄が通る道に当たる）。
/// 操作の解決（進行の宛先など）は、同じ列を流す `Harness` に任せる。
pub struct RecordPath<'a> {
    ir: &'a Ir,
    pub harness: Harness<'a>,
    /// 記録しながら進めた現在の状態（写しの元）。
    pub state: SessionState,
    /// 記録した `ops`。
    pub rec: OpsRecorder,
}

impl<'a> RecordPath<'a> {
    pub fn new(ir: &'a Ir, initial: &SessionState) -> Self {
        RecordPath {
            ir,
            harness: Harness::pure(ir, initial.clone()),
            state: initial.clone(),
            rec: OpsRecorder::new(),
        }
    }

    pub fn draft(&mut self, text: &str) {
        record_draft(&mut self.state, &mut self.rec, text.to_owned());
    }

    /// 抽象の操作を解決して記録しながら進める。入力は下書き→送信→適用の順にする。
    pub fn spec(&mut self, spec: &OpSpec) {
        let applied = self.harness.step(spec);
        self.op(applied.op);
    }

    /// 解決済みの操作を記録しながら進める。解決済みの `Op` をそのまま使い、分類し直さない。
    pub fn op(&mut self, op: Op) {
        match &op {
            Op::Input(Input::Command(s) | Input::Text(s)) => {
                record_draft(&mut self.state, &mut self.rec, s.clone());
                assert!(record_submit(&mut self.state, &mut self.rec, s.clone()));
                self.state = apply(self.ir, self.state.clone(), op).0;
            }
            _ => {
                let (state, _) = apply_recorded(self.ir, self.state.clone(), &mut self.rec, op);
                self.state = state;
            }
        }
    }
}
