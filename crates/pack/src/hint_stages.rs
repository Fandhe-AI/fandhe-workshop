//! ヒントの段の構造の確認（TASK-298、GUIDE-8・D-55）。
//!
//! 「3 段」は教材側の約束であり、語彙・エンジンでは固定しない（D-55。ヒントの上限は現在の案内レベルで出せる数まで。
//! GUIDE-17）。そのため `validate` には結線せず、教材テストが約束を宣言として渡し、IR の Step が宣言どおりかを
//! 一覧で受け取る形にする。呼び出し元は結合試験と、将来の `fandhe-pack test`（TASK-404）である。
//!
//! - 段は Step の `hints` の並び順で数える。ヒントの定義の `level`・`text` などの欄は語彙の語ではないので読まない
//!   （読むには拡張点の手続きが要る）。答えの照合に使う欄の名前も宣言から受け取り、エンジンは固定しない
//! - 定義に解決できない ID は段に数えない（`visible_hints` と同じ fail-closed）。未定義の ID の報告は `E-REF-HINT` の担当
//! - ヒントを持たない Step は対象外。ただし宣言が名指しした Step は段数 0 として確かめる
//! - 報告は `uid`・数・種類だけを持ち、ヒントの文面・答え・欄の値を写さない
//!
//! 確かめないこと: 段の中身（場所・道具の名前・完全なコマンド）の意味と教育上の適切さ。答えの照合は部分文字列の包含の近似である。

use std::collections::BTreeMap;
use std::fmt;

use fandhe_workshop_contract::ir::Ir;
use serde_json::Value;

/// 教材テストが渡す、段の約束の宣言（語彙ではない）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HintStageRule {
    /// 既定の段数（教材の約束。例: 3）。
    pub stages: usize,
    /// 答えの段（1 始まり）。`None` なら答えの確認をしない。
    pub answer_stage: Option<usize>,
    /// `uid` ごとの例外の段数。
    pub per_step: BTreeMap<String, usize>,
    /// 任意の答えの照合。
    pub answer_text: Option<AnswerText>,
}

/// 答えの段の文面が答えを含むかの照合の宣言。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnswerText {
    /// 文面の欄の名前（教材が決める。エンジンは固定しない）。
    pub field: String,
    /// `uid` ごとの答えの文字列。
    pub answers: BTreeMap<String, String>,
}

/// 宣言そのものの誤り。報告 0 件と区別するため `Err` で返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintStageRuleError {
    /// 段数が 0。
    ZeroStages,
    /// 答えの段が 0 または段数を超える。
    AnswerStageOutOfRange,
    /// 文面の欄の名前が空。
    EmptyField,
}

impl fmt::Display for HintStageRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ZeroStages => "hint stage rule: stages must be at least 1",
            Self::AnswerStageOutOfRange => "hint stage rule: answer stage is out of range",
            Self::EmptyField => "hint stage rule: answer text field name is empty",
        })
    }
}

impl std::error::Error for HintStageRuleError {}

/// 構造の確認の報告 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HintStageFinding {
    /// 段数が宣言と違う。
    StageCount {
        /// 実体の Step の `uid`。
        uid: String,
        /// 定義に解決できたヒントの数。
        found: usize,
        /// 宣言の段数。
        declared: usize,
    },
    /// 答えの段に当たるヒントが無い（段数不足）。
    AnswerStageUnresolved {
        /// 実体の Step の `uid`。
        uid: String,
        /// 答えの段（1 始まり）。
        stage: usize,
    },
    /// 照合を宣言したが、その Step の答えが無い・空。
    AnswerNotGiven {
        /// 実体の Step の `uid`。
        uid: String,
    },
    /// 宣言した欄が無い・文字列でない（定義がオブジェクトでない場合を含む）。
    AnswerTextUnreadable {
        /// 実体の Step の `uid`。
        uid: String,
    },
    /// 答えの段の文面が答えを含まない。
    AnswerMissing {
        /// 実体の Step の `uid`。
        uid: String,
    },
    /// 宣言が IR に無い `uid` を指す（宣言の打ち間違いで確認が空振りするのを防ぐ）。
    UnknownStep {
        /// 宣言が指した `uid`。
        uid: String,
    },
}

/// `uid`（`<実体 ID>/<Step ID>`）を報告へ写してよいか。区切りごとに短い識別子のときだけ写す。
fn printable(uid: &str) -> &str {
    if !uid.is_empty() && uid.split('/').all(crate::source::safe_ident) {
        uid
    } else {
        "(unprintable)"
    }
}

impl fmt::Display for HintStageFinding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StageCount {
                uid,
                found,
                declared,
            } => write!(
                f,
                "step {}: {found} hint stages, declared {declared}",
                printable(uid)
            ),
            Self::AnswerStageUnresolved { uid, stage } => write!(
                f,
                "step {}: answer stage {stage} has no hint",
                printable(uid)
            ),
            Self::AnswerNotGiven { uid } => {
                write!(f, "step {}: no expected answer is given", printable(uid))
            }
            Self::AnswerTextUnreadable { uid } => write!(
                f,
                "step {}: answer stage text is unreadable",
                printable(uid)
            ),
            Self::AnswerMissing { uid } => write!(
                f,
                "step {}: answer stage does not contain the answer",
                printable(uid)
            ),
            Self::UnknownStep { uid } => {
                write!(f, "declaration names unknown step {}", printable(uid))
            }
        }
    }
}

fn validate_rule(rule: &HintStageRule) -> Result<(), HintStageRuleError> {
    if rule.stages == 0 || rule.per_step.values().any(|n| *n == 0) {
        return Err(HintStageRuleError::ZeroStages);
    }
    if let Some(k) = rule.answer_stage {
        let min = rule
            .per_step
            .values()
            .copied()
            .fold(rule.stages, usize::min);
        if k == 0 || k > min {
            return Err(HintStageRuleError::AnswerStageOutOfRange);
        }
    }
    if let Some(a) = &rule.answer_text {
        if a.field.is_empty() {
            return Err(HintStageRuleError::EmptyField);
        }
    }
    Ok(())
}

/// IR の各 Step の段の構造を宣言と突き合わせ、食い違いを返す（宣言が指す未知の `uid` が先、続いて IR の順）。
///
/// 宣言の誤りは `Err`。どんな `Value` でも panic しない。
pub fn check_hint_stages(
    ir: &Ir,
    rule: &HintStageRule,
) -> Result<Vec<HintStageFinding>, HintStageRuleError> {
    validate_rule(rule)?;
    let mut out = Vec::new();
    for uid in rule
        .per_step
        .keys()
        .chain(rule.answer_text.iter().flat_map(|a| a.answers.keys()))
    {
        if !ir.nodes.iter().any(|n| &n.uid == uid) {
            out.push(HintStageFinding::UnknownStep { uid: uid.clone() });
        }
    }
    for node in &ir.nodes {
        let defs: Vec<&Value> = node
            .step
            .hints
            .iter()
            .filter_map(|h| ir.hints.get(h))
            .collect();
        let named = rule.per_step.contains_key(&node.uid)
            || rule
                .answer_text
                .as_ref()
                .is_some_and(|a| a.answers.contains_key(&node.uid));
        if defs.is_empty() && !named {
            continue;
        }
        let uid = node.uid.clone();
        let declared = rule.per_step.get(&uid).copied().unwrap_or(rule.stages);
        if defs.len() != declared {
            out.push(HintStageFinding::StageCount {
                uid: uid.clone(),
                found: defs.len(),
                declared,
            });
        }
        let Some(k) = rule.answer_stage else { continue };
        let Some(def) = defs.get(k.saturating_sub(1)) else {
            out.push(HintStageFinding::AnswerStageUnresolved { uid, stage: k });
            continue;
        };
        let Some(at) = &rule.answer_text else {
            continue;
        };
        let Some(answer) = at.answers.get(&uid).filter(|s| !s.is_empty()) else {
            out.push(HintStageFinding::AnswerNotGiven { uid });
            continue;
        };
        match def.get(&at.field).and_then(Value::as_str) {
            None => out.push(HintStageFinding::AnswerTextUnreadable { uid }),
            Some(text) if !text.contains(answer.as_str()) => {
                out.push(HintStageFinding::AnswerMissing { uid });
            }
            Some(_) => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule() -> HintStageRule {
        HintStageRule {
            stages: 3,
            answer_stage: Some(3),
            per_step: BTreeMap::new(),
            answer_text: None,
        }
    }

    #[test]
    fn declaration_errors_are_err() {
        let mut r = rule();
        r.stages = 0;
        assert_eq!(validate_rule(&r), Err(HintStageRuleError::ZeroStages));
        let mut r = rule();
        r.answer_stage = Some(4);
        assert_eq!(
            validate_rule(&r),
            Err(HintStageRuleError::AnswerStageOutOfRange)
        );
        r.answer_stage = Some(0);
        assert_eq!(
            validate_rule(&r),
            Err(HintStageRuleError::AnswerStageOutOfRange)
        );
        let mut r = rule();
        r.per_step.insert("a/b".into(), 2);
        assert_eq!(
            validate_rule(&r),
            Err(HintStageRuleError::AnswerStageOutOfRange)
        );
        let mut r = rule();
        r.answer_text = Some(AnswerText {
            field: String::new(),
            answers: BTreeMap::new(),
        });
        assert_eq!(validate_rule(&r), Err(HintStageRuleError::EmptyField));
        assert_eq!(validate_rule(&rule()), Ok(()));
    }

    #[test]
    fn unprintable_uid_is_masked() {
        assert_eq!(printable("main/st-1"), "main/st-1");
        assert_eq!(printable("a b"), "(unprintable)");
        assert_eq!(printable(""), "(unprintable)");
    }
}
