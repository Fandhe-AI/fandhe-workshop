//! 選び直しの手順を持つ選択肢の確認の「自力」の数え方（TASK-304 2/3、GUIDE-11）。
//!
//! 根拠は TASK-3 の決定（#484、2026-10-31）である。contract の [`LoCheckRecord`] に数え方の規則を当て、
//! 数えた結果（件数）だけを返す。割合の閾値（合否の線）は持たない。教材の LO の ID・Step の ID はここに持たず、
//! どの LO に当てるかは呼び出し側が `lo` で指定する（エンジンは教材を知らない）。
//!
//! # 呼び出し元
//!
//! 教材テスト・講師資料・体験会の集計・復習が、[`crate::lo_observe::records_from_history`]（TASK-304 1/3）の
//! 結果を渡す。
//!
//! # 規則
//!
//! - 最初の回答で正答した確認は自力に数える
//! - 誤答の後に理由を読んで選び直した正答も自力に数える（GUIDE-18 の前後の比較を見せる確認とは違う）
//! - 答えを含むヒントの段以上を開いた後の正答は自力に数えない。答えの段は教材の約束なので、
//!   呼び出し側が [`SelfRelianceRule`] で渡す（D-55。エンジンは固定しない）。開いた最大の段は別に残す
//! - 前後の比較を見た後の選び直しの正答は、この確認には無い形である。自力を多く数えない側に倒し、
//!   自力に数えない
//! - 参加者単位の束ねは「その LO の確認をすべて自力で正答した参加者」を数える（案 A）。
//!   未回答（到達しなかった確認）が 1 件でもあれば数えず、対象の確認が 0 件の参加者も数えない
//! - 分類の前に [`LoCheckRecord::validate`] を通し、不変条件を破る記録は [`LoCheckError`] にする（fail-closed）
//! - 純粋で全域。I/O・時刻・乱数・panic を持たない。`step`・`lo` は未信頼の文字列で、等しさの比較にだけ使う
//!
//! # 扱わないこと
//!
//! - 進行役の手助けの有無（SESS の担当）。AI の代理の結果を人の評価に数えないことは運用の方針であり、欄にしない
//! - 割合の閾値（8 割・半数などの合否の線）
//! - 最初の回答の正答だけを自力に数える確認（[`crate::lo_first_answer`]、GUIDE-18）
//! - 選び直しの手順を持たない確認（GUIDE-12、TASK-304 3/3 が [`CheckKind`] に足す）

use std::fmt;

use fandhe_workshop_contract::lo_check::{FirstAnswer, LoCheckError, LoCheckRecord, Reselection};

/// 確認の種類ごとの規則。後続（GUIDE-12）が、選び直しを自力に数えない種類をここに足す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckKind {
    /// 選び直しの手順を持つ選択肢の確認（GUIDE-11）。
    Reselectable,
}

/// 規則。答えを含むヒントの段は呼び出し側が渡す（D-55）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SelfRelianceRule {
    kind: CheckKind,
    answer_stage: u32,
}

/// 規則の作成の失敗。文面は固定で入力値を含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfRelianceRuleError {
    /// 答えを含むヒントの段が 0。
    ZeroAnswerStage,
}

impl fmt::Display for SelfRelianceRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            SelfRelianceRuleError::ZeroAnswerStage => "answer hint stage must be at least 1",
        })
    }
}

impl std::error::Error for SelfRelianceRuleError {}

impl SelfRelianceRule {
    /// 選び直しの手順を持つ確認の規則を作る。`answer_stage` は答えを含むヒントの段（1 始まり）で、
    /// 0 は拒否する（常に「答えの段を開いた」ことになり、自力が 0 件になるため）。
    pub fn reselectable(answer_stage: u32) -> Result<Self, SelfRelianceRuleError> {
        if answer_stage == 0 {
            return Err(SelfRelianceRuleError::ZeroAnswerStage);
        }
        Ok(Self {
            kind: CheckKind::Reselectable,
            answer_stage,
        })
    }

    /// 答えを含むヒントの段。
    pub fn answer_stage(&self) -> u32 {
        self.answer_stage
    }

    /// 規則の種類。
    pub fn kind(&self) -> CheckKind {
        self.kind
    }
}

/// 記録 1 件の分類。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelfRelianceOutcome {
    /// 自力に数える。
    SelfReliant {
        /// 誤答の後の選び直しの正答か。
        reselected: bool,
    },
    /// 答えを含む段以上のヒントを開いた後の正答。自力に数えない。
    AfterAnswerHint {
        /// 誤答の後の選び直しの正答か。
        reselected: bool,
    },
    /// 前後の比較を見た後の選び直しの正答。自力に数えない。
    ReselectedAfterComparison,
    /// 誤答の後、まだ正答していない。
    NotCorrect,
    /// まだ回答していない。
    Unanswered,
}

impl SelfRelianceOutcome {
    /// 自力に数えるか。[`SelfRelianceOutcome::SelfReliant`] のときだけ真。
    pub fn is_self_reliant(self) -> bool {
        matches!(self, SelfRelianceOutcome::SelfReliant { .. })
    }
}

/// 記録 1 件を分類する。先に [`LoCheckRecord::validate`] を通し、違反は `Err` にする。
///
/// `max_hint` は contract の導出が「確定までの最大の段」として持つため、誤答の後・選び直しの前に
/// 答えの段を開いた場合も「答えの段を開いた後の正答」に当たる。
pub fn classify(
    record: &LoCheckRecord,
    rule: &SelfRelianceRule,
) -> Result<SelfRelianceOutcome, LoCheckError> {
    record.validate()?;
    let reselected = match (record.first_answer, record.reselection) {
        (FirstAnswer::Unanswered, _) => return Ok(SelfRelianceOutcome::Unanswered),
        (FirstAnswer::Correct, _) => false,
        (FirstAnswer::Wrong, Reselection::Correct) => true,
        // `validate` が `None` を拒否済みのため、残りは未正答である。
        (FirstAnswer::Wrong, _) => return Ok(SelfRelianceOutcome::NotCorrect),
    };
    if record.max_hint >= rule.answer_stage {
        return Ok(SelfRelianceOutcome::AfterAnswerHint { reselected });
    }
    if reselected && record.after_comparison {
        return Ok(SelfRelianceOutcome::ReselectedAfterComparison);
    }
    Ok(SelfRelianceOutcome::SelfReliant { reselected })
}

/// 指定した LO を受け持つ記録だけを数えた集計（1 人の参加者分）。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SelfRelianceTally {
    /// 最初の回答で正答した件数（自力）。
    pub self_reliant_first: u32,
    /// 誤答の後の選び直しで正答した件数（自力）。
    pub self_reliant_reselected: u32,
    /// 答えを含む段以上のヒントを開いた後の正答の件数。自力に数えない。
    pub after_answer_hint: u32,
    /// 前後の比較を見た後の選び直しの正答の件数。自力に数えない。
    pub reselected_after_comparison: u32,
    /// 誤答の後、まだ正答していない件数。
    pub not_correct: u32,
    /// 未回答の件数。
    pub unanswered: u32,
    /// 対象の記録で開いた最大のヒントの段。自力の件数とは別に残す。
    pub max_hint: u32,
}

impl SelfRelianceTally {
    /// 対象の記録の件数。
    pub fn checks(&self) -> u32 {
        [
            self.self_reliant(),
            self.after_answer_hint,
            self.reselected_after_comparison,
            self.not_correct,
            self.unanswered,
        ]
        .iter()
        .fold(0u32, |a, n| a.saturating_add(*n))
    }

    /// 自力に数える件数。
    pub fn self_reliant(&self) -> u32 {
        self.self_reliant_first
            .saturating_add(self.self_reliant_reselected)
    }

    /// 対象の記録が 1 件以上あり、すべて自力のときだけ真。0 件は真にしない。
    pub fn all_self_reliant(&self) -> bool {
        self.checks() > 0 && self.self_reliant() == self.checks()
    }
}

/// `records` のうち `lo` を受け持つものを数える。1 件でも不正な記録があれば `Err`
/// （指定の LO 以外の記録も読み飛ばさない）。加算は飽和し、panic しない。
///
/// 1 つの Step が複数の LO を受け持てるため、`record.lo` に含まれるかで判定する。
pub fn tally(
    records: &[LoCheckRecord],
    lo: &str,
    rule: &SelfRelianceRule,
) -> Result<SelfRelianceTally, LoCheckError> {
    let mut t = SelfRelianceTally::default();
    for rec in records {
        let outcome = classify(rec, rule)?;
        if !rec.lo.iter().any(|l| l == lo) {
            continue;
        }
        let slot = match outcome {
            SelfRelianceOutcome::SelfReliant { reselected: false } => &mut t.self_reliant_first,
            SelfRelianceOutcome::SelfReliant { reselected: true } => &mut t.self_reliant_reselected,
            SelfRelianceOutcome::AfterAnswerHint { .. } => &mut t.after_answer_hint,
            SelfRelianceOutcome::ReselectedAfterComparison => &mut t.reselected_after_comparison,
            SelfRelianceOutcome::NotCorrect => &mut t.not_correct,
            SelfRelianceOutcome::Unanswered => &mut t.unanswered,
        };
        *slot = slot.saturating_add(1);
        t.max_hint = t.max_hint.max(rec.max_hint);
    }
    Ok(t)
}

/// 参加者単位の束ね（案 A）の結果。割合は返さない。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ParticipantCount {
    /// 渡された参加者の数。
    pub participants: u32,
    /// その LO の確認をすべて自力で正答した参加者の数。
    pub all_self_reliant: u32,
}

/// 参加者ごとの記録から、その LO の確認をすべて自力で正答した参加者を数える。
/// 1 人でも不正な記録があれば `Err`。
pub fn count_participants<R: AsRef<[LoCheckRecord]>>(
    participants: &[R],
    lo: &str,
    rule: &SelfRelianceRule,
) -> Result<ParticipantCount, LoCheckError> {
    let mut c = ParticipantCount::default();
    for p in participants {
        let t = tally(p.as_ref(), lo, rule)?;
        c.participants = c.participants.saturating_add(1);
        if t.all_self_reliant() {
            c.all_self_reliant = c.all_self_reliant.saturating_add(1);
        }
    }
    Ok(c)
}
