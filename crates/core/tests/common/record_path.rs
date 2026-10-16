//! 「記録の経路」の試験用の部品（`history_replay`・`history_verify`・`history_open` の試験が共有する）。
//!
//! 下書きと送信を記録しながら `apply` で状態を進める。実行時の入力欄が通る道に当たり、適用し直し
//! （[`fandhe_workshop_core::history_replay::replay`]）の結果と比べる側の基準になる。

use super::op_gen::{Harness, OpSpec};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::history_envelope::{record_draft, record_submit};
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::ops_record::{apply_recorded, reload_recorded, OpsRecorder};
use fandhe_workshop_core::session::{apply, SessionState};

/// 下書きの種類。`None` は下書きを挟まない。
pub fn draft_text(kind: u8) -> Option<&'static str> {
    match kind % 5 {
        0 => None,
        1 => Some(""),
        2 => Some(" \u{3000}"),
        3 => Some("ノートを作りたい"),
        _ => Some("/make-note first-note"),
    }
}

/// 記録の経路。下書きと送信を記録しながら `apply` で状態を進める（実行時の入力欄が通る道に当たる）。
/// 操作の解決（進行の宛先など）は、同じ列を流す `Harness` に任せる。
pub struct RecordPath<'a> {
    ir: &'a Ir,
    pub harness: Harness<'a>,
    pub state: SessionState,
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
        if *spec == OpSpec::Reload {
            self.reload();
            return;
        }
        let applied = self.harness.step(spec);
        self.op(applied.op.expect("a non-reload spec resolves to an op"));
    }

    /// 再読み込みを記録しながら進める。ハーネスの状態も同じ再読み込みでそろえる。
    pub fn reload(&mut self) {
        let (state, _) = reload_recorded(self.state.clone(), &mut self.rec);
        self.state = state;
        self.harness.reload();
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
