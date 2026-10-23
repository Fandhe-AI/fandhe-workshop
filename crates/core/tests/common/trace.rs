//! 1 回の実行の記録と、2 回の実行の比較器（TASK-218 の 3 件が共有する部品）。
//!
//! 呼び出し元は `determinism.rs`（1/3。同じ入力列の 2 回適用）で、2/3（表示文字列の差し替え）・3/3（ID の変更）も
//! 同じ比べ方を使えるよう共通部品に置く。2/3・3/3 が表示文字列の扱いを足す場合も、この記録は変えずに足す。
//! 比較は `==` に加えて `serde_json` の直列化バイト列でも行う。ハッシュ系の集合は `PartialEq` が順序を無視するため、
//! `==` だけでは反復順の非決定性を見逃す。
//! 試験の部品なので、直列化の失敗は理由を添えた `expect` で止める（本番コードの規則の対象外）。

use super::op_gen::{Harness, OpSpec};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::Projection;

/// 1 操作の記録。導出は操作後のログ全体から作り直した値である。
#[derive(Debug, Clone, PartialEq)]
pub struct StepTrace {
    pub op: Option<Op>,
    pub events: Vec<Event>,
    pub after: SessionState,
    pub log_rows: Vec<fandhe_workshop_core::projection::LogRow>,
    pub marks: Vec<fandhe_workshop_core::projection::Mark>,
    pub review: Vec<fandhe_workshop_core::projection::ReviewItem>,
}

/// 列を最後まで流して記録する。
pub fn trace(mut harness: Harness<'_>, specs: &[OpSpec]) -> Vec<StepTrace> {
    specs
        .iter()
        .map(|spec| {
            let applied = harness.step(spec);
            let projection = Projection::from_events(&applied.after.log);
            StepTrace {
                op: applied.op,
                events: applied.events,
                log_rows: projection.log().to_vec(),
                marks: projection.marks().to_vec(),
                review: projection.review().to_vec(),
                after: applied.after,
            }
        })
        .collect()
}

fn bytes<T: serde::Serialize>(value: &T) -> Vec<u8> {
    serde_json::to_vec(value).expect("serialize trace value")
}

/// 2 つの記録を各操作で比べる。食い違いは何番目の操作の何の欄かを返す。
pub fn compare_traces(a: &[StepTrace], b: &[StepTrace]) -> Result<(), String> {
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        if x.op != y.op {
            return Err(format!("op #{i}: op differs: {:?} vs {:?}", x.op, y.op));
        }
        if x.events != y.events {
            return Err(format!("op #{i} {:?}: events differ", x.op));
        }
        if x.after != y.after {
            let field = if x.after.run != y.after.run {
                "run"
            } else if x.after.log != y.after.log {
                "log"
            } else if x.after.entry != y.after.entry {
                "entry"
            } else if x.after.checkpoints != y.after.checkpoints {
                "checkpoints"
            } else if x.after.drafts != y.after.drafts {
                "drafts"
            } else if x.after.hints_opened != y.after.hints_opened {
                "hints_opened"
            } else {
                "other"
            };
            return Err(format!("op #{i} {:?}: state differs in {field}", x.op));
        }
        if x.log_rows != y.log_rows {
            return Err(format!("op #{i} {:?}: projection log rows differ", x.op));
        }
        if x.marks != y.marks {
            return Err(format!("op #{i} {:?}: projection marks differ", x.op));
        }
        if x.review != y.review {
            return Err(format!("op #{i} {:?}: projection review differs", x.op));
        }
        if bytes(&x.events) != bytes(&y.events)
            || bytes(&x.after) != bytes(&y.after)
            || bytes(&x.log_rows) != bytes(&y.log_rows)
            || bytes(&x.marks) != bytes(&y.marks)
            || bytes(&x.review) != bytes(&y.review)
        {
            return Err(format!("op #{i} {:?}: serialized bytes differ", x.op));
        }
    }
    if a.len() != b.len() {
        return Err(format!("run length differs: {} vs {}", a.len(), b.len()));
    }
    Ok(())
}
