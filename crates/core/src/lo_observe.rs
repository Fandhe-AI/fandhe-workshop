//! 履歴から、確認ごとの最初の回答の正誤と開いたヒントの段を観測の列として取り出す（TASK-304 1/3、GUIDE-11）。
//!
//! contract の `derive_records` は観測の列から `LoCheckRecord` を作るが、core の [`Event`] から観測
//! （`CheckObservation`）を作る写像は無かった。各試験が同じ規則を書き写していたため、ここに 1 か所で置く。
//! 呼び出し元は、教材テスト（pack）・講師資料・復習画面・体験会の集計（`records_from_history` が入口）と、
//! 操作を 1 件ずつ流す試験（[`answer_phase`] と [`observe_op`]）である。呼び出し先は
//! [`crate::history_replay::replay_with`]（適用し直しの規則は二重に持たない）と [`crate::instance::status_of`] である。
//!
//! # 回答に数える操作
//!
//! 確認の段階はログから推定できない（`Restored` は `Entered` を出さずに状態を戻し、上限で止めた操作は
//! 操作の前へ戻る）。そこで、操作の前の状態から段階を読み、操作の後のイベントから正誤を決める。
//! 次を全部満たす操作だけが `Answered` を 1 件作る。
//!
//! 1. 操作が `Op::Input` である（入力欄の送信の適用を含む）
//! 2. 操作の前の現在の Step が `Accepted` で、処理中でない
//! 3. 入力の種類が Step の確認の方式に合う（`answer` は選択、`point_at` は画面操作、`approve`・`confirm` 無しは
//!    「承認」。「直す」は誤答に数えない）。`none`・未知の `kind`・読めない宣言は回答を作らない
//! 4. 戻りに `TransitionLimitReached` が無い（止めた操作は確認が成立していない）
//!
//! 正誤は戻りの `Judged` で決める。`Confirmed` は正答、`Rejected` は誤答、それ以外（`Judged` が無い等）は
//! 回答を作らない。宣言の誤りを回答に数えない。未知の選択肢の ID は誤答に数え、自力を多く数えない側に倒す。
//!
//! ヒントは戻りの `Event::HintOpened` を、操作の種類によらず写す。拒否（`Refused`）は写さない。
//! `step` は根を含む完全な実体 ID で、周回・実体ごとに別の観測になる（GUIDE-7）。
//!
//! # 扱わないこと
//!
//! - 前後の比較を見せたこと（`ComparisonShown`）。対応するイベントが無いため作らず、導いた記録の
//!   `after_comparison` は常に `false` になる（`docs/known-gaps.md` GUIDE-18）
//! - IR からの `CheckTarget` の作成（TASK-47）、自力の数え方（`lo_first_answer`、GUIDE-11・GUIDE-12 の後続）
//! - 確定の後の観測を捨てる規則（`derive_records` が持つ）
//!
//! 不変条件。全域で panic しない。決定的で I/O を持たない。`step` は教材と履歴に由来する未信頼の文字列で、
//! 写すだけで解釈しない。契約の版（`CONTRACT_VERSION`）の対象外。

use fandhe_workshop_contract::confirm_spec::ConfirmKind;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckObservation, CheckTarget, LoCheckRecord,
};
use serde_json::Value;

use crate::event::Event;
use crate::history_replay::{replay_with, ReplayError};
use crate::input::{Decision, Input, Op};
use crate::instance::status_of;
use crate::ops_record::SessionOp;
use crate::session::SessionState;
use crate::step_status::StepStatus;

/// 操作の前の状態から読んだ、回答の段階の印。外からは [`answer_phase`] でしか作れない（偽の段階を渡せない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerPhase {
    step: String,
}

/// 確認の方式に合う入力か。`kind` の読みは確認の判定と同じ（`confirm` が無ければ `approve`）。
fn fits_method(step: &Step, input: &Input) -> bool {
    let kind = match step.confirm.as_ref() {
        None => ConfirmKind::Approve,
        Some(decl) => match decl
            .get("kind")
            .and_then(Value::as_str)
            .and_then(ConfirmKind::from_str_opt)
        {
            Some(k) => k,
            None => return false,
        },
    };
    matches!(
        (kind, input),
        (ConfirmKind::Answer, Input::Choice(_))
            | (ConfirmKind::PointAt, Input::Ui { .. })
            | (
                ConfirmKind::Approve,
                Input::Approval {
                    decision: Decision::Approve,
                    ..
                }
            )
    )
}

/// 操作の前に呼ぶ。回答に数える条件のうち、操作の前の状態で決まるもの（上の 1〜3）を満たすときだけ `Some`。
pub fn answer_phase(ir: &Ir, before: &SessionState, op: &Op) -> Option<AnswerPhase> {
    let Op::Input(input) = op else {
        return None;
    };
    if before.run.busy.is_some() {
        return None;
    }
    let uid = before.run.current.as_ref()?;
    if status_of(ir, &before.run, uid) != StepStatus::Accepted {
        return None;
    }
    let node = ir.nodes.iter().find(|n| n.uid == uid.0)?;
    if !fits_method(&node.step, input) {
        return None;
    }
    Some(AnswerPhase {
        step: uid.0.clone(),
    })
}

/// 操作の後に呼ぶ。戻りのイベントから、ヒントと回答の観測を作る。
pub fn observe_op(phase: Option<AnswerPhase>, events: &[Event]) -> Vec<CheckObservation> {
    let mut out: Vec<CheckObservation> = events
        .iter()
        .filter_map(|e| match e {
            Event::HintOpened { at, opened, .. } => Some(CheckObservation::HintOpened {
                step: at.step.0.clone(),
                opened: *opened,
            }),
            _ => None,
        })
        .collect();
    let stopped = events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. }));
    if let (Some(phase), false) = (phase, stopped) {
        let correct = events.iter().find_map(|e| match e {
            Event::Judged { judgment, .. } => match judgment {
                Judgment::Confirmed => Some(true),
                Judgment::Rejected(_) => Some(false),
                _ => None,
            },
            _ => None,
        });
        if let Some(correct) = correct {
            out.push(CheckObservation::Answered {
                step: phase.step,
                correct,
            });
        }
    }
    out
}

/// 初期状態に `ops` を適用し直しながら、観測を履歴の順に集める。
///
/// # Errors
///
/// [`ReplayError`]（適用し直せない `ops`）。
pub fn observe_history(
    ir: &Ir,
    initial: SessionState,
    ops: &[SessionOp],
) -> Result<Vec<CheckObservation>, ReplayError> {
    let mut out = Vec::new();
    replay_with(
        ir,
        initial,
        ops,
        |state, op| answer_phase(ir, state, op),
        |phase, events| out.extend(observe_op(phase, events)),
    )?;
    Ok(out)
}

/// [`observe_history`] の結果を `derive_records` に渡して記録にする。
///
/// # Errors
///
/// [`ReplayError`]。
pub fn records_from_history(
    ir: &Ir,
    initial: SessionState,
    ops: &[SessionOp],
    targets: &[CheckTarget],
) -> Result<Vec<LoCheckRecord>, ReplayError> {
    Ok(derive_records(targets, &observe_history(ir, initial, ops)?))
}
