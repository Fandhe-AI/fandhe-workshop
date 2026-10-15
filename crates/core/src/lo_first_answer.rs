//! 学習目標（LO）の確認で、最初の回答の正答だけを自力に数える規則（TASK-305、GUIDE-18、D-25）。
//!
//! 前後の比較を見せて選び直させる確認（試作の LO-2）に当てる規則である。教材の LO の ID はここに持たない。
//! どの LO に当てるかは、呼び出し側が [`tally`] の `lo` で指定する（エンジンは教材を知らない）。
//!
//! # 呼び出し元
//!
//! 教材テスト・講師資料・体験会の集計・通しの試験（TASK-306）が、contract の
//! [`LoCheckRecord`]（TASK-291）を渡す。履歴から記録への導出は contract の `derive_records` が持ち、
//! `Event` から観測への写像は TASK-304 が持つ。
//!
//! # 規則
//!
//! - 最初の回答で正答した確認は「自力」に数える
//! - 誤答の後の選び直しで正答した確認は、前後の比較を見た後のものも含めて自力に数えない。
//!   選び直しの正答として別に分類する（比較の有無も別に保つ）
//! - 分類は `first_answer`・`reselection`・`after_comparison` だけを見る
//! - 不変条件を破る記録は [`LoCheckError`] にする。不正な記録を自力に数える経路は作らない（fail-closed）
//! - 純粋で全域。I/O・時刻・乱数・panic を持たない
//!
//! # 扱わないこと
//!
//! - ヒントの段（`max_hint`）との組み合わせ。GUIDE-18 は決めていないため読まない
//! - 他の LO の数え方（GUIDE-11・GUIDE-12 は TASK-304）への一般化
//! - 参加者単位の束ね・人数の数え方（SESS・INSTR 側）
//! - 進行役の手助け・AI の代理の結果の除外（運用側）

use fandhe_workshop_contract::lo_check::{FirstAnswer, LoCheckError, LoCheckRecord, Reselection};

/// 1 件の記録の、最初の回答だけを自力に数える規則での分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstAnswerOutcome {
    /// 最初の回答で正答した。自力に数える。
    SelfReliant,
    /// 誤答の後の選び直しで正答した。自力に数えず、別に記録する。
    Reselected {
        /// 選び直しの前に前後の比較を見せたか。
        after_comparison: bool,
    },
    /// 誤答の後、まだ正答していない。
    NotCorrect,
    /// まだ回答していない。
    Unanswered,
}

impl FirstAnswerOutcome {
    /// 自力に数えるか。[`FirstAnswerOutcome::SelfReliant`] のときだけ真。
    pub fn is_self_reliant(self) -> bool {
        matches!(self, FirstAnswerOutcome::SelfReliant)
    }
}

/// 記録 1 件を分類する。先に [`LoCheckRecord::validate`] を通し、違反は `Err` にする。
pub fn classify(record: &LoCheckRecord) -> Result<FirstAnswerOutcome, LoCheckError> {
    record.validate()?;
    Ok(match record.first_answer {
        FirstAnswer::Correct => FirstAnswerOutcome::SelfReliant,
        FirstAnswer::Unanswered => FirstAnswerOutcome::Unanswered,
        FirstAnswer::Wrong => match record.reselection {
            Reselection::Correct => FirstAnswerOutcome::Reselected {
                after_comparison: record.after_comparison,
            },
            // `validate` が `None` を拒否済みのため、残りは未正答である。
            Reselection::NotCorrect | Reselection::None => FirstAnswerOutcome::NotCorrect,
        },
    })
}

/// 指定した LO を受け持つ記録だけを数えた集計。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FirstAnswerTally {
    /// 最初の回答で正答した件数（自力）。
    pub self_reliant: u32,
    /// 前後の比較を見た後の選び直しで正答した件数。自力に数えない。
    pub reselected_after_comparison: u32,
    /// 比較を見ずに選び直して正答した件数。自力に数えない。
    pub reselected_without_comparison: u32,
    /// 誤答の後、まだ正答していない件数。
    pub not_correct: u32,
    /// 未回答の件数。
    pub unanswered: u32,
}

/// `records` のうち `lo` を受け持つものだけを数える。1 件でも不正な記録があれば `Err`（読み飛ばさない）。
///
/// `lo` は呼び出し側が、前後の比較を見せる確認の LO の ID として渡す。1 つの Step が複数の LO を
/// 受け持てるため、`record.lo` に含まれるかで判定する。加算は飽和し、panic しない。
pub fn tally(records: &[LoCheckRecord], lo: &str) -> Result<FirstAnswerTally, LoCheckError> {
    let mut t = FirstAnswerTally::default();
    for rec in records {
        let outcome = classify(rec)?;
        if !rec.lo.iter().any(|l| l == lo) {
            continue;
        }
        let slot = match outcome {
            FirstAnswerOutcome::SelfReliant => &mut t.self_reliant,
            FirstAnswerOutcome::Reselected {
                after_comparison: true,
            } => &mut t.reselected_after_comparison,
            FirstAnswerOutcome::Reselected {
                after_comparison: false,
            } => &mut t.reselected_without_comparison,
            FirstAnswerOutcome::NotCorrect => &mut t.not_correct,
            FirstAnswerOutcome::Unanswered => &mut t.unanswered,
        };
        *slot = slot.saturating_add(1);
    }
    Ok(t)
}
