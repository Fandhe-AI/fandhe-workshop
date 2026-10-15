//! 学習目標（LO）の確認の記録の型と、履歴から記録を導く関数（TASK-291、版 1）。
//!
//! 受講者が自力で正答したかの数え方（GUIDE-11・GUIDE-12・GUIDE-18）、復習画面（REVIEW-13）、講師資料（INSTR）、
//! 最初の教材が同じ記録を読めるよう、記録の形と導出をこの 1 か所に置く。各担当が履歴から別々に記録を作ると、
//! 統合のときに食い違うためである。
//!
//! 記録が持つ情報は次の 3 つである。
//! - Step ごとの最初の回答の正誤
//! - 選び直しの正答。前後の比較を見た後の選び直しを含み、最初の回答とは区別して持つ
//! - 開いた最大のヒントの段
//!
//! 呼び出し元は次のとおり。
//! - core: TASK-304・TASK-305 が、`Event` から [`CheckObservation`] への写像と数え方の規則を持つ
//! - web・REVIEW・INSTR・最初の教材: [`LoCheckRecord`] を読んで表示・集計する
//!
//! # 境界（本契約が定義しないもの）
//! - 数え方の規則（どれを自力に数えるか）。`self_reliant` のような欄は持たない（TASK-304・TASK-305）
//! - core の `Event` から観測への写像。観測と対応するイベントは次のとおり。
//!   `HintOpened` は `Event::HintOpened`、`Answered { correct: true }` は確認の `Confirmed`、
//!   `Answered { correct: false }` は確認への拒否の記録。`Event::Judged` の追加は TASK-267・TASK-204 の版上げで行う
//! - IR の `confirm` から [`CheckTarget`] を作る処理（TASK-47）
//! - 選んだ選択肢の ID と表示名。必要になれば版を上げて足す
//! - 進行役の手助けの有無（体験会 SESS が記録する。エンジンには分からない）
//! - AI の代理の結果を除くこと（運用の方針で、欄にはしない）
//!
//! `step`・`lo` は教材と履歴に由来する未信頼の値で、型は保持するだけで解釈しない。HTML へ出すときのエスケープは
//! web の責務である。エラーの文面は入力値を埋め込まない。
//!
//! 値・規則・型を変える場合は、先に spec を直し、[`LO_CHECK_CONTRACT_VERSION`] を上げる。

use core::fmt;

use serde::{Deserialize, Serialize};

/// この契約の版。値・規則・型を変えたら上げる。
pub const LO_CHECK_CONTRACT_VERSION: u32 = 1;

/// 確認の方式。語彙の `confirm` の語から `none` を除いたもの（`none` は確認が無いので記録を作らない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckMethod {
    /// 画面の要素を指して確認する。
    PointAt,
    /// 選んで答えて確認する。
    Answer,
    /// 承認して確認する。
    Approve,
}

impl CheckMethod {
    /// 全方式。
    pub const ALL: [CheckMethod; 3] = [
        CheckMethod::PointAt,
        CheckMethod::Answer,
        CheckMethod::Approve,
    ];

    /// 直列化・語彙と同じ綴り。
    pub fn as_str(self) -> &'static str {
        match self {
            CheckMethod::PointAt => "point_at",
            CheckMethod::Answer => "answer",
            CheckMethod::Approve => "approve",
        }
    }
}

/// 最初の回答の正誤。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirstAnswer {
    /// 最初の回答が正答。
    Correct,
    /// 最初の回答が誤答。
    Wrong,
    /// まだ回答していない。
    Unanswered,
}

/// 誤答の後の選び直し。最初の回答が誤答のときだけ `None` 以外になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reselection {
    /// 選び直しの対象ではない（最初の回答が誤答ではない）。
    None,
    /// 選び直して正答した。
    Correct,
    /// 誤答の後にまだ正答していない。
    NotCorrect,
}

/// 記録の不変条件の違反。文面は固定で入力値を含まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoCheckError {
    /// `step` が空。
    EmptyStep,
    /// `lo` が空。
    EmptyLo,
    /// 最初の回答が誤答でないのに、選び直しまたは比較後の欄がある。
    ReselectionWithoutWrong,
    /// 最初の回答が誤答なのに、選び直しの欄が `none`。
    MissingReselection,
}

impl fmt::Display for LoCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            LoCheckError::EmptyStep => "lo check record has an empty step",
            LoCheckError::EmptyLo => "lo check record has no learning objective",
            LoCheckError::ReselectionWithoutWrong => {
                "reselection fields require a wrong first answer"
            }
            LoCheckError::MissingReselection => "a wrong first answer requires a reselection state",
        })
    }
}

impl std::error::Error for LoCheckError {}

/// 1 つの Step の LO の確認の記録。履歴から [`derive_records`] で導く。
///
/// 逆直列化でも [`LoCheckRecord::validate`] と同じ検査をし、違反は拒否する（fail-closed）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawRecord")]
pub struct LoCheckRecord {
    /// Step の実体 ID（IR の `Node.uid`）。
    pub step: String,
    /// Step の `lo` の写し。1 つの Step が複数の LO を受け持てる。
    pub lo: Vec<String>,
    /// 確認の方式。
    pub method: CheckMethod,
    /// 最初の回答の正誤。
    pub first_answer: FirstAnswer,
    /// 誤答の後の選び直し。最初の回答とは区別して持つ（GUIDE-18）。
    pub reselection: Reselection,
    /// 選び直しの前に前後の比較を見せたか。
    pub after_comparison: bool,
    /// 開いた最大のヒントの段。0 は開いていない。
    pub max_hint: u32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRecord {
    step: String,
    lo: Vec<String>,
    method: CheckMethod,
    first_answer: FirstAnswer,
    reselection: Reselection,
    after_comparison: bool,
    max_hint: u32,
}

impl TryFrom<RawRecord> for LoCheckRecord {
    type Error = LoCheckError;

    fn try_from(r: RawRecord) -> Result<Self, LoCheckError> {
        let rec = LoCheckRecord {
            step: r.step,
            lo: r.lo,
            method: r.method,
            first_answer: r.first_answer,
            reselection: r.reselection,
            after_comparison: r.after_comparison,
            max_hint: r.max_hint,
        };
        rec.validate()?;
        Ok(rec)
    }
}

impl LoCheckRecord {
    /// 不変条件を確かめる。
    pub fn validate(&self) -> Result<(), LoCheckError> {
        if self.step.is_empty() {
            return Err(LoCheckError::EmptyStep);
        }
        if self.lo.is_empty() {
            return Err(LoCheckError::EmptyLo);
        }
        if self.first_answer == FirstAnswer::Wrong {
            if self.reselection == Reselection::None {
                return Err(LoCheckError::MissingReselection);
            }
        } else if self.reselection != Reselection::None || self.after_comparison {
            return Err(LoCheckError::ReselectionWithoutWrong);
        }
        Ok(())
    }
}

/// 導出の対象。IR の Step から作るのは呼び出し側（`confirm` の解釈は TASK-47）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CheckTarget {
    /// Step の実体 ID。
    pub step: String,
    /// Step の `lo`。
    pub lo: Vec<String>,
    /// 確認の方式。
    pub method: CheckMethod,
}

/// 導出の入力の 1 件。履歴の順に並べる。core の `Event` からの写像は TASK-304 が持つ。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckObservation {
    /// ヒントを開いた（`Event::HintOpened` に対応）。
    HintOpened {
        /// Step の実体 ID。
        step: String,
        /// 開いた段（1 始まり）。
        opened: u32,
    },
    /// 確認に回答した。
    Answered {
        /// Step の実体 ID。
        step: String,
        /// 正答か。
        correct: bool,
    },
    /// 前後の比較を見せた。
    ComparisonShown {
        /// Step の実体 ID。
        step: String,
    },
}

impl CheckObservation {
    fn step(&self) -> &str {
        match self {
            CheckObservation::HintOpened { step, .. }
            | CheckObservation::Answered { step, .. }
            | CheckObservation::ComparisonShown { step } => step,
        }
    }
}

/// 履歴の観測から記録を導く。純粋で全域（panic しない）。
///
/// - 出力は `targets` 1 件につき 1 件で、`targets` の順に並ぶ
/// - 対象の外の Step の観測は捨てる
/// - 最初の `Answered` で最初の回答が決まる。正答ならそこで確定する
/// - 誤答なら選び直しは `NotCorrect` で、後の正答の `Answered` で `Correct` になり確定する。
///   確定の前に `ComparisonShown` があれば `after_comparison` を立てる
/// - `max_hint` は確定までの `HintOpened` の最大の段。確定しなければ全観測から取る
/// - 確定の後の観測は無視し、最初の結論を書き換えない
/// - 回答が無ければ `Unanswered`
/// - 最初の回答の前の `ComparisonShown` は記録に反映しない（不変条件を保つため）
///
/// チェックポイントへの復帰では記録をやり直さない。履歴は追記だけで、最初の回答は最初のまま残る。
/// 自力を多く数えない側に倒す判断である。
pub fn derive_records(
    targets: &[CheckTarget],
    observations: &[CheckObservation],
) -> Vec<LoCheckRecord> {
    targets
        .iter()
        .map(|t| derive_one(t, observations))
        .collect()
}

fn derive_one(t: &CheckTarget, observations: &[CheckObservation]) -> LoCheckRecord {
    let mut rec = LoCheckRecord {
        step: t.step.clone(),
        lo: t.lo.clone(),
        method: t.method,
        first_answer: FirstAnswer::Unanswered,
        reselection: Reselection::None,
        after_comparison: false,
        max_hint: 0,
    };
    for obs in observations.iter().filter(|o| o.step() == t.step) {
        let settled =
            rec.first_answer == FirstAnswer::Correct || rec.reselection == Reselection::Correct;
        if settled {
            break;
        }
        match obs {
            CheckObservation::HintOpened { opened, .. } => {
                rec.max_hint = rec.max_hint.max(*opened);
            }
            CheckObservation::ComparisonShown { .. } => {
                if rec.first_answer == FirstAnswer::Wrong {
                    rec.after_comparison = true;
                }
            }
            CheckObservation::Answered { correct, .. } => match rec.first_answer {
                FirstAnswer::Unanswered => {
                    if *correct {
                        rec.first_answer = FirstAnswer::Correct;
                    } else {
                        rec.first_answer = FirstAnswer::Wrong;
                        rec.reselection = Reselection::NotCorrect;
                    }
                }
                _ => {
                    if *correct {
                        rec.reselection = Reselection::Correct;
                    }
                }
            },
        }
    }
    rec
}
