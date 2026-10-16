//! 拒否の文面の解決と、空でない次の行動（JUDGE-4・JUDGE-10、TASK-264）。
//!
//! 判定（[`crate::select_judge`]）は、拒否を文面の ID（規則による拒否）か分類（分類による拒否）で返す。
//! 本モジュールはそれを、教材の `rejections` の表から [`Rejection`]（分類・文面の ID・空でない
//! [`NextAction`]）に解決する。「分類はエンジン、文面は教材」（JUDGE-4）に従い、文面は教材の値を読むだけで作らない。
//! 呼び出し元は、`Judged` の発行（TASK-267）、`Session::apply`（TASK-207）、差し込みの充填（[`crate::text_fill`]、TASK-274）、
//! 画面の表示（TASK-447）である。
//!
//! # 文面の ID の決め方
//!
//! - 規則による拒否: 規則が持つ ID
//! - 分類による拒否: `Step.on_reject[分類]`、無ければ `Ir.defaults[分類]`（試作と同じ順）。分類は引数の値を使い、
//!   文面の側の `category` との一致は見ない（pack の検査の範囲、TASK-46）
//!
//! # 扱わないこと
//!
//! - 差し込みの枠（`{{…}}`）の充填。返す次の行動は「差し込み前」の値で、枠を含んでいても空ではない。
//!   実行時に利用者へ出す値は [`crate::text_fill`] が充填した後の値である（TASK-274）
//! - 状態の変更・`Judged` の発行（TASK-207・TASK-267）
//! - 教材の文面の静的な検査（TASK-46）
//!
//! 外部への呼び出しは持たない。全域（panic しない）で決定的な純関数である。教材の値は未信頼の文字列として
//! 保持するだけで、HTML への埋め込みとエスケープは web の責務である。失敗は [`RejectionTextError`] にし、
//! 受付にも空の次の行動にも倒さない。`Display` は固定文で、入力値・教材の値・ID を埋め込まない。

use std::fmt;

use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::{Category, NextAction, RejectCategory, Rejection};
use serde_json::Value;

use crate::select_judge::SelectVerdict;

/// 解決する拒否の出どころ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionSource<'a> {
    /// 規則による拒否。`rejections` の ID。
    Rule(&'a str),
    /// 分類による拒否。文面は `on_reject`、`defaults` の順で引く。
    Category(RejectCategory),
}

/// 文面を解決できない理由。`Display` は入力値・教材の値・ID を埋め込まない固定文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionTextError {
    /// `on_reject`・`defaults` のどちらにも無い、または ID が `rejections` に無い。
    MissingText,
    /// 文面の形が読めない（object でない・`category` の欠け・未知・`free_text_unjudged`・`next_action` の欠け）。
    InvalidText,
    /// `next_action` が空、または空白だけ。
    EmptyNextAction,
}

impl fmt::Display for RejectionTextError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingText => f.write_str("rejection text is not declared"),
            Self::InvalidText => f.write_str("rejection text cannot be read"),
            Self::EmptyNextAction => f.write_str("rejection text has an empty next action"),
        }
    }
}

impl std::error::Error for RejectionTextError {}

/// 拒否を、教材の表から [`Rejection`] に解決する。次の行動は空でない（差し込み前）。
pub fn resolve_rejection(
    ir: &Ir,
    step: &Step,
    source: RejectionSource<'_>,
) -> Result<Rejection, RejectionTextError> {
    let (id, by_category) = match source {
        RejectionSource::Rule(id) => (id, None),
        RejectionSource::Category(c) => {
            let key = c.as_str();
            let on_reject = step.on_reject.get(key);
            let id = on_reject
                .or_else(|| ir.defaults.get(key))
                .ok_or(RejectionTextError::MissingText)?;
            (id.as_str(), Some(c))
        }
    };
    let text = ir
        .rejections
        .get(id)
        .ok_or(RejectionTextError::MissingText)?
        .as_object()
        .ok_or(RejectionTextError::InvalidText)?;
    let category = match by_category {
        Some(c) => c,
        None => text
            .get("category")
            .and_then(Value::as_str)
            .and_then(Category::from_str_opt)
            .and_then(|c| RejectCategory::try_from(c).ok())
            .ok_or(RejectionTextError::InvalidText)?,
    };
    let next_action = text
        .get("next_action")
        .and_then(Value::as_str)
        .ok_or(RejectionTextError::InvalidText)?;
    let next_action =
        NextAction::new(next_action).map_err(|_| RejectionTextError::EmptyNextAction)?;
    Ok(Rejection {
        category,
        text: id.to_string(),
        next_action,
    })
}

/// 判定結果が拒否なら [`Rejection`] に解決する。受付・問い返しは `Ok(None)`。
pub fn rejection_of(
    ir: &Ir,
    step: &Step,
    verdict: &SelectVerdict,
) -> Result<Option<Rejection>, RejectionTextError> {
    let source = match verdict {
        SelectVerdict::RejectedByRule { rejection, .. } => RejectionSource::Rule(rejection),
        SelectVerdict::Rejected(c) => RejectionSource::Category(*c),
        SelectVerdict::Accepted { .. } | SelectVerdict::Clarified { .. } => return Ok(None),
    };
    resolve_rejection(ir, step, source).map(Some)
}
