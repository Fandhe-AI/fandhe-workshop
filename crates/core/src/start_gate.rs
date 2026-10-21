//! 途中開始の開始条件（`requires`・`unmet`。EP-9、PACK-18、TASK-51）を状態に触れる前に評価する関門。
//!
//! 呼び出し元は、開始の操作（`Control::Begin`。TASK-217）と、開始条件による拒否の記録（TASK-240、MOD-7）である。
//! 評価を core に置くのは、それらが core にあり、core から pack へ依存できないためである。
//! 本モジュールは評価だけを行い、`Refused` の記録・世代・状態の変更は行わない。`&RunState` しか受け取らない
//! ので、状態を変える前の評価であることを型が保証する。
//!
//! # 規則
//!
//! - 開始 ID が IR に無い、または開始先の Step の実体が無いときは [`StartRefusal::UnknownStart`]
//! - `requires` が無ければ開始してよい
//! - `requires` は開始先の Step が属する実体の根で評価する。モジュールの開始点も同じ規則で、その実体の根になる
//! - 真のときだけ開始してよい。偽と、評価できない（`None`）は [`StartRefusal::Unmet`] にする（安全側）
//! - `Unmet` の文面は `unmet` の ID を [`crate::rejection_text`] で解決する。文面を解決できなくても拒否は覆らない
//!
//! 全域（panic しない）で決定的な純関数で、I/O・時刻・乱数を使わない。文面は教材の値を未信頼の
//! 文字列のまま返す（HTML への埋め込みとエスケープは web の責務）。

use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_contract::judgment::Rejection;

use crate::addr::Root;
use crate::condition::VocabConditions;
use crate::effect::ConditionEval;
use crate::event::RefuseReason;
use crate::rejection_text::{resolve_rejection, RejectionSource, RejectionTextError};
use crate::session::RunState;

/// 開始してよくない理由。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StartRefusal {
    /// 開始 ID が IR に無い、または開始先の Step の実体が無い。
    UnknownStart,
    /// 開始条件を満たさない、または評価できない。
    Unmet {
        /// 開始の ID。
        start: String,
        /// 示す文面。解決できなくても拒否は覆らない。
        text: Result<Rejection, RejectionTextError>,
    },
}

impl StartRefusal {
    /// `Refused` に記録する理由（TASK-240）。
    pub fn reason(&self) -> RefuseReason {
        match self {
            Self::UnknownStart => RefuseReason::UnknownStart,
            Self::Unmet { .. } => RefuseReason::StartUnmet,
        }
    }
}

/// 開始点 `start_id` を開始してよいかを判定する。状態は読むだけで変えない。
pub fn check_start<'ir>(
    ir: &'ir Ir,
    run: &RunState,
    start_id: &str,
) -> Result<&'ir Start, StartRefusal> {
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .ok_or(StartRefusal::UnknownStart)?;
    let node = ir
        .nodes
        .iter()
        .find(|n| n.uid == start.node)
        .ok_or(StartRefusal::UnknownStart)?;
    let Some(requires) = start.requires.as_ref() else {
        return Ok(start);
    };
    let root = Root(node.scope.clone());
    let met = VocabConditions::new(&ir.instances).eval(requires, run, &root);
    if met == Some(true) {
        return Ok(start);
    }
    let text = match start.unmet.as_ref().and_then(|v| v.as_str()) {
        Some(id) => resolve_rejection(ir, &node.step, RejectionSource::Rule(id)),
        None => Err(RejectionTextError::MissingText),
    };
    Err(StartRefusal::Unmet {
        start: start.id.clone(),
        text,
    })
}
