//! 入力方式 5 種の宣言の型と、`text` の判定の純関数（TASK-44、版 1）。
//!
//! Step の `input`（PACK-10）は、`command`・`text`・`choice`・`ui_action`・`approval` の 5 方式で宣言する。
//! 呼び出し元は、宣言を検査する pack（TASK-44）と、実行時に `text` を判定する core（TASK-262・TASK-267）である。
//! core は pack に依存できないため、両方が使う型と判定は contract に置く（`docs/architecture.md` 第 6 節）。
//!
//! IR の `Step.input` は未型付けの値のまま保たれ（golden のバイト一致のため）、この型は検査と実行時の
//! 判定で読むときだけ使う。どの型も未知の欄を拒否する（失敗を拒否に倒す）。`wait`（EP-1）は別タスクで足す。
//!
//! `text` が判定するのは、空でないこと・文字数の下限と上限だけで、内容の良し悪しは判定しない。
//! 受け付けた入力の分類は `free_text_unjudged` で、範囲外は `bad_arg_shape` で拒否する。
//! 判定結果と分類の型は [`crate::judgment`] のものを使い、ここでは定義しない。
//! エラーの `Display` は入力値を埋め込まない固定文にする。

use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::judgment::{Category, JudgmentKind};

/// この契約の版。欄・判定の規則を変えたら上げる。
pub const INPUT_SPEC_CONTRACT_VERSION: u32 = 1;

/// 入力方式の種類。`wait`（EP-1）は別タスクで足す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InputKind {
    /// コマンドと引数。
    Command,
    /// 複数行の自然文。
    Text,
    /// 選択肢。
    Choice,
    /// 画面の操作。
    UiAction,
    /// 承認・差し戻し。
    Approval,
}

impl InputKind {
    /// 全 5 方式。
    pub const ALL: [InputKind; 5] = [
        Self::Command,
        Self::Text,
        Self::Choice,
        Self::UiAction,
        Self::Approval,
    ];

    /// 教材に書く `kind` の語。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::Text => "text",
            Self::Choice => "choice",
            Self::UiAction => "ui_action",
            Self::Approval => "approval",
        }
    }

    /// `kind` の語から読む。未知の語は `None`。
    pub fn from_str_opt(s: &str) -> Option<InputKind> {
        Self::ALL.into_iter().find(|k| k.as_str() == s)
    }
}

/// 宣言の意味の制約の違反。`Display` は入力値を含まない固定文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecError {
    /// `text` の下限が上限を超えている。
    MinAboveMax,
    /// 名前・ID・表示名・ビュー・経路のどれかが空。
    EmptyName,
    /// 引数名または選択肢の ID が重複している。
    Duplicate,
    /// 選択肢または対象が 1 件も無い。
    EmptyList,
}

impl fmt::Display for SpecError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::MinAboveMax => "text min exceeds max",
            Self::EmptyName => "empty name",
            Self::Duplicate => "duplicate name",
            Self::EmptyList => "empty list",
        })
    }
}

impl std::error::Error for SpecError {}

fn non_empty(s: &str) -> Result<(), SpecError> {
    if s.trim().is_empty() {
        Err(SpecError::EmptyName)
    } else {
        Ok(())
    }
}

/// 引数の形。閉じた集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArgShape {
    /// 小文字・数字・ハイフン。
    KebabCase,
    /// 数字のみ。
    Digits,
    /// 形を問わない。
    #[default]
    Any,
}

/// コマンドの引数 1 つの宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArgSpec {
    /// 引数名。
    pub name: String,
    /// 位置引数か。
    #[serde(default)]
    pub positional: bool,
    /// 必須か。
    #[serde(default)]
    pub required: bool,
    /// 形。
    #[serde(default)]
    pub shape: ArgShape,
}

/// `command` の宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommandSpec {
    /// コマンド名。
    pub name: String,
    /// 引数の宣言。
    #[serde(default)]
    pub args: Vec<ArgSpec>,
}

impl CommandSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), SpecError> {
        non_empty(&self.name)?;
        let mut seen = BTreeSet::new();
        for a in &self.args {
            non_empty(&a.name)?;
            if !seen.insert(a.name.as_str()) {
                return Err(SpecError::Duplicate);
            }
        }
        Ok(())
    }
}

/// `text` の判定の結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextVerdict {
    /// 受け付ける。
    Accepted,
    /// 空（空白だけを含む）。
    Empty,
    /// 下限に満たない。
    TooShort,
    /// 上限を超える。
    TooLong,
}

impl TextVerdict {
    /// 判定結果の種類。受付か拒否。
    pub fn kind(self) -> JudgmentKind {
        match self {
            Self::Accepted => JudgmentKind::Accepted,
            _ => JudgmentKind::Rejected,
        }
    }

    /// 付ける分類。受付は `free_text_unjudged`、範囲外は `bad_arg_shape`。
    pub fn category(self) -> Category {
        match self {
            Self::Accepted => Category::FreeTextUnjudged,
            _ => Category::BadArgShape,
        }
    }
}

/// `text` の宣言。欄は文字数の下限と上限だけで、内容を判定する欄は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextSpec {
    /// 文字数の下限（両端を含む）。
    pub min: u32,
    /// 文字数の上限（両端を含む）。
    pub max: u32,
}

impl TextSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), SpecError> {
        if self.min > self.max {
            Err(SpecError::MinAboveMax)
        } else {
            Ok(())
        }
    }

    /// 入力を判定する。前後の空白を除いた後が空なら `Empty`、文字数（コードポイント）が範囲外なら
    /// `TooShort`・`TooLong`、それ以外は `Accepted`。内容は見ない。数えるのは `max + 1` までで、
    /// 巨大な入力でも計算量は宣言の上限に比例する。
    pub fn judge(&self, text: &str) -> TextVerdict {
        let t = text.trim();
        if t.is_empty() {
            return TextVerdict::Empty;
        }
        let n = t
            .chars()
            .take((self.max as usize).saturating_add(1))
            .count();
        if n < self.min as usize {
            TextVerdict::TooShort
        } else if n > self.max as usize {
            TextVerdict::TooLong
        } else {
            TextVerdict::Accepted
        }
    }
}

/// 選択肢 1 つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceOption {
    /// 選択肢の ID。
    pub id: String,
    /// 表示名。
    pub label: String,
}

/// `choice` の宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoiceSpec {
    /// 選択肢。
    pub options: Vec<ChoiceOption>,
}

impl ChoiceSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), SpecError> {
        if self.options.is_empty() {
            return Err(SpecError::EmptyList);
        }
        let mut seen = BTreeSet::new();
        for o in &self.options {
            non_empty(&o.id)?;
            non_empty(&o.label)?;
            if !seen.insert(o.id.as_str()) {
                return Err(SpecError::Duplicate);
            }
        }
        Ok(())
    }
}

/// 画面の操作の対象。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiTarget {
    /// ビューの ID。
    pub view: String,
    /// ビューの中の経路。
    pub path: String,
}

/// `ui_action` の宣言。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UiActionSpec {
    /// 操作の対象。
    pub targets: Vec<UiTarget>,
}

impl UiActionSpec {
    /// 意味の制約を検査する。
    pub fn check(&self) -> Result<(), SpecError> {
        if self.targets.is_empty() {
            return Err(SpecError::EmptyList);
        }
        for t in &self.targets {
            non_empty(&t.view)?;
            non_empty(&t.path)?;
        }
        Ok(())
    }
}

/// `approval` の宣言。欄を持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalSpec {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_words_round_trip() {
        for k in InputKind::ALL {
            assert_eq!(InputKind::from_str_opt(k.as_str()), Some(k));
        }
        assert_eq!(InputKind::from_str_opt("wait"), None);
    }

    #[test]
    fn judge_boundaries() {
        let t = TextSpec { min: 2, max: 3 };
        assert_eq!(t.judge(""), TextVerdict::Empty);
        assert_eq!(t.judge("a"), TextVerdict::TooShort);
        assert_eq!(t.judge("ab"), TextVerdict::Accepted);
        assert_eq!(t.judge("abc"), TextVerdict::Accepted);
        assert_eq!(t.judge("abcd"), TextVerdict::TooLong);
    }

    /// `max` が `u32::MAX` でも、32 ビットの `usize`（wasm32）で計数上限が溢れず正しく判定する。
    #[test]
    fn judge_max_u32_max() {
        let t = TextSpec {
            min: 1,
            max: u32::MAX,
        };
        assert_eq!(t.judge("a"), TextVerdict::Accepted);
        assert_eq!(t.judge("   "), TextVerdict::Empty);
    }
}
