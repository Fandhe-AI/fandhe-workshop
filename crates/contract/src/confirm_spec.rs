//! Step の確認（`confirm`）4 種の宣言の型と、受付と達成の分け方の純関数（TASK-47、版 1）。
//!
//! 入力を受け付けただけでは Step を達成にせず、受講者の確認の操作（左の変化を指す・問いに答える・承認する）で
//! 初めて達成にする（R-22、PACK-13）。確認を省けるのは、理由を添えて `none` と明示した Step だけである。
//! 呼び出し元は、宣言を検査する pack（TASK-47）、実行時に確認の判定をする core（後続）、選択肢を表示する web
//! （TASK-47 の 2/2）である。3 者が同じ型を読むため、`docs/architecture.md` 第 6 節に従い contract に置く。
//!
//! IR の `Step.confirm` は未型付けの値のまま保たれ（golden のバイト一致のため）、この型は検査と実行時の
//! 判定で読むときだけ使う。`kind` は呼び出し側が取り除いてから、種類ごとの型で読む（`input_spec` と同じ方式）。
//! どの型も未知の欄を拒否する（失敗を拒否に倒す）。エラーの `Display` は入力値を埋め込まない固定文にする。
//!
//! `answer` の選択肢の `on_wrong`（EP-6）と `label`（v1.3）は語彙で固定済みの欄なので形として受け付けるが、
//! 意味の検査と表示は後続のタスクの担当である。

use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub use crate::input_spec::UiTarget;

/// この契約の版。欄・規則を変えたら上げる。
pub const CONFIRM_SPEC_CONTRACT_VERSION: u32 = 1;

/// 確認の種類。語彙の文書 7 節の `confirm` の 4 語。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfirmKind {
    /// 左の変化を指す。
    PointAt,
    /// 問いに答える。
    Answer,
    /// 承認する。
    Approve,
    /// 確認を省く（理由が要る）。
    None,
}

impl ConfirmKind {
    /// 全種類。
    pub const ALL: [ConfirmKind; 4] = [Self::PointAt, Self::Answer, Self::Approve, Self::None];

    /// 教材に書く `kind` の語。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PointAt => "point_at",
            Self::Answer => "answer",
            Self::Approve => "approve",
            Self::None => "none",
        }
    }

    /// `kind` の語から読む。未知の語は `None`。
    pub fn from_str_opt(s: &str) -> Option<ConfirmKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// 宣言の意味の制約の違反。`Display` は入力値を含まない固定文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmSpecError {
    /// ID・文面・ビュー・経路のどれかが空。
    EmptyName,
    /// 選択肢の ID が重複している。
    Duplicate,
    /// 対象または選択肢が 1 件も無い。
    EmptyList,
    /// 正答の選択肢が 1 件も無い。
    NoCorrectOption,
}

impl fmt::Display for ConfirmSpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::EmptyName => "empty name",
            Self::Duplicate => "duplicate name",
            Self::EmptyList => "empty list",
            Self::NoCorrectOption => "no correct option",
        })
    }
}

impl std::error::Error for ConfirmSpecError {}

fn non_empty(s: &str) -> Result<(), ConfirmSpecError> {
    if s.trim().is_empty() {
        Err(ConfirmSpecError::EmptyName)
    } else {
        Ok(())
    }
}

/// `point_at` の宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointAtSpec {
    /// 指す対象。
    pub targets: Vec<UiTarget>,
    /// 外したときの文面の ID。
    #[serde(default)]
    pub on_miss: Option<String>,
}

impl PointAtSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), ConfirmSpecError> {
        if self.targets.is_empty() {
            return Err(ConfirmSpecError::EmptyList);
        }
        for t in &self.targets {
            non_empty(&t.view)?;
            non_empty(&t.path)?;
        }
        Ok(())
    }
}

/// `answer` の選択肢 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerOption {
    /// 選択肢の ID。
    pub id: String,
    /// 正答か。
    pub correct: bool,
    /// この選択肢の誤答の文面（EP-6）。意味の検査は TASK-46 の担当。
    #[serde(default)]
    pub on_wrong: Option<String>,
    /// 表示名（v1.3）。表示は TASK-47 の 2/2 の担当。
    #[serde(default)]
    pub label: Option<String>,
}

/// `answer` の宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AnswerSpec {
    /// 選択肢。
    pub options: Vec<AnswerOption>,
    /// Step の既定の誤答の文面の ID。
    pub on_wrong: String,
}

impl AnswerSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), ConfirmSpecError> {
        if self.options.is_empty() {
            return Err(ConfirmSpecError::EmptyList);
        }
        non_empty(&self.on_wrong)?;
        let mut seen = BTreeSet::new();
        for o in &self.options {
            non_empty(&o.id)?;
            if !seen.insert(o.id.as_str()) {
                return Err(ConfirmSpecError::Duplicate);
            }
        }
        if !self.options.iter().any(|o| o.correct) {
            return Err(ConfirmSpecError::NoCorrectOption);
        }
        Ok(())
    }
}

/// `approve` の宣言。欄を持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApproveSpec {}

/// `none`（確認の省略）の宣言。理由の有無は `E-NO-CONFIRM` の判定に使うため、欠けた理由は空として読む。
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoneSpec {
    /// 省略の理由。
    #[serde(default)]
    pub reason: String,
}

impl NoneSpec {
    /// 前後の空白を除いて空でない理由を持つか。
    pub fn has_reason(&self) -> bool {
        !self.reason.trim().is_empty()
    }
}

/// 受付と達成の関係（R-22）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Achievement {
    /// 受付の後、この種類の確認の操作で達成になる。
    AfterConfirm(ConfirmKind),
    /// 受付と同時に達成になる（理由つきの省略を明示した Step だけ）。
    OnAccept,
}

/// 確認の種類から、受付と達成の関係を決める。省略の明示（`none`）だけが受付で達成になる。確認が無い場合は
/// 安全側に倒し、受付と同時に達成にしない（`approve` として確認を要する）。
pub fn achievement(kind: Option<ConfirmKind>) -> Achievement {
    match kind {
        Some(ConfirmKind::None) => Achievement::OnAccept,
        Some(k) => Achievement::AfterConfirm(k),
        Option::None => Achievement::AfterConfirm(ConfirmKind::Approve),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_words_round_trip() {
        for k in ConfirmKind::ALL {
            assert_eq!(ConfirmKind::from_str_opt(k.as_str()), Some(k));
        }
        assert_eq!(ConfirmKind::from_str_opt("skip"), None);
    }

    #[test]
    fn only_none_achieves_on_accept() {
        for k in ConfirmKind::ALL {
            let expect = if k == ConfirmKind::None {
                Achievement::OnAccept
            } else {
                Achievement::AfterConfirm(k)
            };
            assert_eq!(achievement(Some(k)), expect);
        }
        assert!(matches!(achievement(None), Achievement::AfterConfirm(_)));
    }
}
