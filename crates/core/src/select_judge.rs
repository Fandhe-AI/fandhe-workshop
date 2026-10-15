//! 選択肢・画面操作・承認の判定（JUDGE-1、TASK-263）。
//!
//! 教材が宣言した入力方式のうち `choice`・`ui_action`・`approval` は、LLM を使わずに Step の宣言
//! （`input`・`rules`・`confirm`）との照合だけで判定できる。本モジュールはその照合の純関数を置く。
//! 呼び出し元は、受付・達成の遷移（TASK-207）、`Judged` の発行（[`crate::judge`]、TASK-267）、対象つきの「直す」（TASK-264）、
//! 誤答の文面の差し込み（TASK-274）、画面（web、TASK-445・TASK-447）である。
//! 承認の「直す」の対象つき入力（JUDGE-10、TASK-264）の判定もここに置く。
//! 結果の型の語彙は `fandhe_workshop_contract::judgment`、入力・確認の宣言の型は `input_spec`・`confirm_spec`
//! が正で、ここでは再定義しない。
//!
//! # 2 つの段
//!
//! - [`judge_select`]: Step が入力を待つ段。`Step.input` で宣言に無い値と方式違いを拒否し、`Step.rules` を
//!   上から見て最初に一致した規則の `outcome` を結果にする
//! - [`judge_confirm`]: Step が受付済みで確認の操作を待つ段。`Step.confirm`（`answer`・`point_at`・`approve`）に照らす
//!
//! # 規則
//!
//! - 受付は、受付の規則（`outcome.kind == accept`）に一致したときにしか返さない。宣言に無い値は、
//!   それを指す規則があっても受付にしない
//! - 規則の `on` のキーは、その方式の照合キーとちょうど一致するものだけを一致とする。余分なキーのある規則は一致しない
//! - 承認の `target` は完全一致で、規則に無ければ入力も `None` でなければならない（対象つきの規則に
//!   対象なしの入力を一致させない）。宣言に無い対象を受付にしないため、対象なしの規則を任意の対象に
//!   当てる照合（試作の挙動）は採らない
//! - 対象つきの「直す」の規則が 1 つでもある Step では、対象なしの「直す」を受付・問い返しにしない
//!   （[`revise_targets`]）。対象なしの規則のうち拒否だけを採り、無ければ `not_in_this_step` の分類による拒否にする。
//!   この判定のため、対象なしの「直す」では規則の全件を先に走査し、一致より後ろの不正な規則も
//!   `InvalidDeclaration` にする（安全側）
//! - 文字列の比較は大文字小文字を区別する完全一致である。trim・Unicode の正規化はしない
//! - 宣言が読めない場合は [`SelectJudgeError::InvalidDeclaration`] にし、受付へは倒さない
//!
//! 外部への呼び出し（ネットワーク・ファイル・プロセス・時刻・乱数・LLM）は持たない。全域（panic しない）で決定的で、
//! 計算量は宣言の件数に対して線形である。値は信頼できない文字列として比べるだけで、エラーの `Display` に
//! 入力値は入れない。
//!
//! # 扱わないこと
//!
//! - 状態の変更・`Session::apply` への組み込み（TASK-207）。結果の統合と `Judged` の材料は [`crate::judge`]（TASK-267）
//! - 文面の解決・差し込み・`NextAction` の生成（本モジュールは文面の ID だけを返す。解決は
//!   [`crate::rejection_text`]、差し込みは TASK-274）
//! - 規則の `on` の組み合わせの静的な検査（TASK-45）
//! - `rejections[id].category` の解決（規則による拒否の分類は呼び出し側）
//! - コマンド・自然文の判定（[`crate::command_lex`]・`TextSpec::judge`）

use std::fmt;

use fandhe_workshop_contract::confirm_spec::{
    answer_spec_of, ApproveSpec, ConfirmKind, NoneSpec, PointAtSpec,
};
use fandhe_workshop_contract::input_spec::{ApprovalSpec, ChoiceSpec, InputKind, UiActionSpec};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{JudgmentKind, RejectCategory};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::input::{Decision, Input};

/// 入力の段の判定結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectVerdict {
    /// 規則が受付を返した。効果の適用は呼び出し側（TASK-207）。
    Accepted {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
    },
    /// 規則が拒否を返した。`rejection` は IR の `rejections` の ID。
    RejectedByRule {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
        /// `rejections` の ID（分類の解決は呼び出し側）。
        rejection: String,
    },
    /// 規則が問い返しを返した。`clarification` は IR の `clarifications` の ID。
    Clarified {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
        /// `clarifications` の ID。
        clarification: String,
    },
    /// 分類による拒否（宣言に無い値・方式の違い・一致する規則が無い）。
    Rejected(RejectCategory),
}

impl SelectVerdict {
    /// 判定結果の種類。受付・拒否・問い返しのどれかに写る。
    pub fn kind(&self) -> JudgmentKind {
        match self {
            Self::Accepted { .. } => JudgmentKind::Accepted,
            Self::RejectedByRule { .. } | Self::Rejected(_) => JudgmentKind::Rejected,
            Self::Clarified { .. } => JudgmentKind::Clarified,
        }
    }
}

/// 確認の段の判定結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmVerdict {
    /// 確認の操作が成立した。達成への遷移は呼び出し側（TASK-207）。
    Confirmed,
    /// `answer` の誤答。`text` は選択肢の `on_wrong`、無ければ Step の `on_wrong`（EP-6）の文面の ID。
    WrongAnswer {
        /// 選ばれた選択肢の ID。
        option: String,
        /// 誤答の文面の ID。
        text: String,
    },
    /// `point_at` の外れで、`on_miss` がある。
    Missed {
        /// 外したときの文面の ID。
        text: String,
    },
    /// 分類による拒否。
    Rejected(RejectCategory),
}

impl ConfirmVerdict {
    /// 判定結果の種類。成立は確認、それ以外は拒否に写る。
    pub fn kind(&self) -> JudgmentKind {
        match self {
            Self::Confirmed => JudgmentKind::Confirmed,
            Self::WrongAnswer { .. } | Self::Missed { .. } | Self::Rejected(_) => {
                JudgmentKind::Rejected
            }
        }
    }
}

/// 判定できない理由。`Display` は入力値・宣言の値を埋め込まない固定文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectJudgeError {
    /// `Command`・`Text`・`Cancel` など、3 方式でない入力。振り分けは呼び出し側。
    NotSelectInput,
    /// `Step.input`・`rules`・`confirm` が読めない（未知の欄・未知の `outcome.kind`・`rejection` の欠けなど）。
    InvalidDeclaration,
}

impl fmt::Display for SelectJudgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotSelectInput => f.write_str("input is not a choice, ui or approval input"),
            Self::InvalidDeclaration => f.write_str("step declaration cannot be read"),
        }
    }
}

impl std::error::Error for SelectJudgeError {}

fn is_select_input(input: &Input) -> bool {
    matches!(
        input,
        Input::Choice(_) | Input::Ui { .. } | Input::Approval { .. }
    )
}

/// `kind` を除いた宣言を型で読む。読めなければ `None`。
fn read_spec<T: DeserializeOwned>(decl: &Map<String, Value>) -> Option<T> {
    let mut map = decl.clone();
    map.remove("kind");
    serde_json::from_value(Value::Object(map)).ok()
}

fn decision_word(d: Decision) -> &'static str {
    match d {
        Decision::Approve => "approve",
        Decision::Reject => "reject",
        Decision::Revise => "revise",
    }
}

/// 承認の対象の照合。完全一致で、規則に無ければ入力も `None` でなければならない。
///
/// 試作のように対象なしの規則を任意の対象に当てると、宣言に無い対象つきの入力が受付になるため、
/// 完全一致のままにする（TASK-264 の判断）。誤った対象の候補の判定は TASK-265 が決める。
fn approval_target_matches(rule_target: Option<&str>, input_target: Option<&str>) -> bool {
    rule_target == input_target
}

/// 規則の `on` が、入力に一致するか。キーの集合が方式の照合キーとちょうど一致しなければ偽。
fn on_matches(on: &Map<String, Value>, input: &Input) -> bool {
    match input {
        Input::Choice(id) => {
            on.len() == 1 && on.get("option").and_then(Value::as_str) == Some(id.as_str())
        }
        Input::Ui { view, path } => {
            if on.len() != 1 {
                return false;
            }
            let Some(ui) = on.get("ui").and_then(Value::as_object) else {
                return false;
            };
            ui.len() == 2
                && ui.get("view").and_then(Value::as_str) == Some(view.as_str())
                && ui.get("path").and_then(Value::as_str) == Some(path.as_str())
        }
        Input::Approval { decision, target } => {
            let rule_target = match on.get("target") {
                None => None,
                Some(v) => match v.as_str() {
                    Some(s) => Some(s),
                    None => return false,
                },
            };
            let expected_len = if on.contains_key("target") { 2 } else { 1 };
            on.len() == expected_len
                && on.get("approval").and_then(Value::as_str) == Some(decision_word(*decision))
                && approval_target_matches(rule_target, target.as_deref())
        }
        Input::Command(_) | Input::Text(_) | Input::Cancel => false,
    }
}

/// Step の規則が「直す」に対して宣言した対象の一覧（`on: { approval: revise, target: … }` の `target`）。
///
/// 規則の並び順で、重複は除く。`when` は見ず、静的な宣言だけで決める（決定的）。画面（TASK-447）が
/// 「直す（対象: …）」の操作を出すときの正で、web に規則の JSON を読ませないために core が返す。
/// 規則・`on` が object でない場合と、`target` が文字列でない場合は [`SelectJudgeError::InvalidDeclaration`]。
pub fn revise_targets(step: &Step) -> Result<Vec<String>, SelectJudgeError> {
    let mut targets: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for rule in &step.rules {
        let rule = rule
            .as_object()
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        let on = rule
            .get("on")
            .and_then(Value::as_object)
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        if on.get("approval").and_then(Value::as_str) != Some("revise") {
            continue;
        }
        let Some(target) = on.get("target") else {
            continue;
        };
        let target = target
            .as_str()
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        if seen.insert(target) {
            targets.push(target.to_string());
        }
    }
    Ok(targets)
}

/// 宣言の方式と入力の種類が合い、宣言にある値なら `Ok(true)`。方式違い・宣言に無い値は `Ok(false)`。
fn declared(step: &Step, input: &Input) -> Result<bool, SelectJudgeError> {
    let Some(decl) = &step.input else {
        return Ok(false);
    };
    let decl = decl
        .as_object()
        .ok_or(SelectJudgeError::InvalidDeclaration)?;
    let kind = decl
        .get("kind")
        .and_then(Value::as_str)
        .and_then(InputKind::from_str_opt)
        .ok_or(SelectJudgeError::InvalidDeclaration)?;
    match (kind, input) {
        (InputKind::Choice, Input::Choice(id)) => {
            let spec: ChoiceSpec = read_spec(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            Ok(spec.options.iter().any(|o| o.id == *id))
        }
        (InputKind::UiAction, Input::Ui { view, path }) => {
            let spec: UiActionSpec = read_spec(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            Ok(spec
                .targets
                .iter()
                .any(|t| t.view == *view && t.path == *path))
        }
        (InputKind::Approval, Input::Approval { .. }) => {
            read_spec::<ApprovalSpec>(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// 入力の段の判定。Step の `input` で宣言に無い値・方式違いを拒否し、`rules` を上から照合する。
///
/// `when` は規則の `when` の評価器。`Some(true)` のときだけその規則を採り、`Some(false)` と `None`
/// （評価できない）は規則を飛ばす（`effect::apply_effects` と同じく安全側）。純関数であること。
/// 評価器は `ConditionEval` の実行状態を持たない形にして、状態に触れない本関数から呼びやすくしている。
pub fn judge_select(
    step: &Step,
    input: &Input,
    when: &dyn Fn(&Value) -> Option<bool>,
) -> Result<SelectVerdict, SelectJudgeError> {
    if !is_select_input(input) {
        return Err(SelectJudgeError::NotSelectInput);
    }
    if !declared(step, input)? {
        return Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep));
    }
    // 対象つきの「直す」の規則がある Step では、対象なしの「直す」に対して拒否の規則だけを採る（JUDGE-10）。
    let revise_without_target = matches!(
        input,
        Input::Approval {
            decision: Decision::Revise,
            target: None
        }
    ) && !revise_targets(step)?.is_empty();
    for (rule_index, rule) in step.rules.iter().enumerate() {
        let rule = rule
            .as_object()
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        let rule_id = rule
            .get("id")
            .and_then(Value::as_str)
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        let on = rule
            .get("on")
            .and_then(Value::as_object)
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        if !on_matches(on, input) {
            continue;
        }
        if let Some(cond) = rule.get("when") {
            if when(cond) != Some(true) {
                continue;
            }
        }
        let outcome = rule
            .get("outcome")
            .and_then(Value::as_object)
            .ok_or(SelectJudgeError::InvalidDeclaration)?;
        let text_of = |key: &str| {
            outcome
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or(SelectJudgeError::InvalidDeclaration)
        };
        let rule_id = rule_id.to_string();
        let kind = outcome.get("kind").and_then(Value::as_str);
        if revise_without_target && matches!(kind, Some("accept" | "clarify")) {
            continue;
        }
        return match kind {
            Some("accept") => Ok(SelectVerdict::Accepted {
                rule_index,
                rule_id,
            }),
            Some("reject") => Ok(SelectVerdict::RejectedByRule {
                rule_index,
                rule_id,
                rejection: text_of("rejection")?,
            }),
            Some("clarify") => Ok(SelectVerdict::Clarified {
                rule_index,
                rule_id,
                clarification: text_of("clarification")?,
            }),
            _ => Err(SelectJudgeError::InvalidDeclaration),
        };
    }
    Ok(SelectVerdict::Rejected(RejectCategory::NotInThisStep))
}

/// 確認の段の判定。`Step.confirm` に照らす。`confirm` が無い Step は `approve` として扱う
/// （`confirm_spec::achievement(None)` と同じ安全側）。
pub fn judge_confirm(step: &Step, input: &Input) -> Result<ConfirmVerdict, SelectJudgeError> {
    if !is_select_input(input) {
        return Err(SelectJudgeError::NotSelectInput);
    }
    let rejected = ConfirmVerdict::Rejected(RejectCategory::NotInThisStep);
    let Some(confirm) = &step.confirm else {
        return Ok(approve_verdict(input));
    };
    let decl = confirm
        .as_object()
        .ok_or(SelectJudgeError::InvalidDeclaration)?;
    let kind = decl
        .get("kind")
        .and_then(Value::as_str)
        .and_then(ConfirmKind::from_str_opt)
        .ok_or(SelectJudgeError::InvalidDeclaration)?;
    match kind {
        ConfirmKind::Answer => {
            let spec = answer_spec_of(confirm).ok_or(SelectJudgeError::InvalidDeclaration)?;
            let Input::Choice(id) = input else {
                return Ok(rejected);
            };
            Ok(match spec.option(id) {
                None => rejected,
                Some(o) if o.correct => ConfirmVerdict::Confirmed,
                Some(o) => ConfirmVerdict::WrongAnswer {
                    option: o.id.clone(),
                    text: o.on_wrong.clone().unwrap_or_else(|| spec.on_wrong.clone()),
                },
            })
        }
        ConfirmKind::PointAt => {
            let spec: PointAtSpec = read_spec(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            let Input::Ui { view, path } = input else {
                return Ok(rejected);
            };
            if spec
                .targets
                .iter()
                .any(|t| t.view == *view && t.path == *path)
            {
                return Ok(ConfirmVerdict::Confirmed);
            }
            Ok(match spec.on_miss {
                Some(text) => ConfirmVerdict::Missed { text },
                None => rejected,
            })
        }
        ConfirmKind::Approve => {
            read_spec::<ApproveSpec>(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            Ok(approve_verdict(input))
        }
        ConfirmKind::None => {
            read_spec::<NoneSpec>(decl).ok_or(SelectJudgeError::InvalidDeclaration)?;
            Ok(rejected)
        }
    }
}

/// 対象なしの「承認」だけが確認の操作になる。
fn approve_verdict(input: &Input) -> ConfirmVerdict {
    match input {
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        } => ConfirmVerdict::Confirmed,
        _ => ConfirmVerdict::Rejected(RejectCategory::NotInThisStep),
    }
}
