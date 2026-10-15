//! 複数行の自然文の受付と、受付による成果物の変化（JUDGE-2、TASK-266）。
//!
//! 入力方式が `text` の Step では、内容の良し悪しを判定しない（W-27・N-4・EXT-3。意味理解・正規表現・LLM を使わない）。
//! 確かめるのは「前後の空白を除いて空でないこと」と「文字数（コードポイント）が宣言の範囲内であること」だけで、
//! 範囲内の文は分類 `free_text_unjudged` の受付になる。この分類は受付に付くもので、拒否ではない（PACK-11）。
//! 範囲の判定は contract の `TextSpec::judge` が正で、ここでは再実装しない。
//!
//! 呼び出し元は、`session::apply` の入力の分岐（TASK-207）と教材テスト（TASK-404、pack）である。
//! 宣言の読み出し（[`text_spec`]）・判定と規則の選択（[`judge_text`]）・受付の適用（[`accept_text`]）を
//! I/O・時刻・乱数・panic を持たない決定的な関数として置く。
//!
//! # 規則
//!
//! 1. 宣言が `TextSpec` として読めて `check` を通らなければ [`TextDeclError`] にする（受付にしない）
//! 2. 範囲外（空・下限未満・上限超過）は [`TextJudgment::Rejected`]（分類 `bad_arg_shape`）にし、規則は読まない
//! 3. 受付なら、`rules` を上から読み、`on.text` が真偽値の `true` で結果が `accept` の最初の規則を選ぶ。
//!    `when` があれば評価器で `Some(true)` のときだけ選ぶ（`Some(false)`・`None` は次へ進む）。
//!    当たる規則が無くても受付で、効果は空
//! 4. 選んだ規則の `effects` が 1 項目でも語彙として読めなければ [`TextDeclError::InvalidEffect`] にする
//!    （一部だけ適用する状態を作らない）
//! 5. 受付の結果は受講者の文を原文のまま（`trim`・正規化をせず、改行を含めて）保つ
//!
//! # 「成果物への反映」の解釈（PACK-39、D-50）
//!
//! 受講者の値を状態変化の経路・属性・文面へ差し込む語は語彙に無い（未信頼入力として守る範囲を広げないため）。
//! 反映は、当たった規則の効果（例: `put` でメモのカードを置く）を世界状態に適用することと、
//! 受講者の文を原文のまま保って呼び出し側が `Event::InputReceived` に載せられるようにすることの 2 点である。
//! 文の表示は web の非解釈の描画（SAFE-11）が担う。
//!
//! # 扱わないこと
//!
//! - `session::apply` への結線、`processing` を持つ Step の処理中への移行、`InputReceived` の発行（TASK-207・TASK-210）
//! - `Judged` の発行と、`on.text` の規則の `reject`・`clarify` の扱い（TASK-267）
//! - `not_in_this_step` などの分類（TASK-268・TASK-269）。`NotText` の分類は呼び出し側が決める
//! - 達成。受付だけでは Step は達成にならず、確認の操作の後の遷移は TASK-207・TASK-306 が受け持つ
//! - 自然文の内容で規則を分ける語（PACK-41、D-43）
//! - 入力欄の 20,000 コードポイントの上限（SAFE-21）は読み込み口（web）の責務。本モジュールは宣言の `max` までしか
//!   数えない（`TextSpec::judge` が `max + 1` で止める）ため、計算量は宣言の上限に比例する

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::input_spec::{TextSpec, TextVerdict};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{Category, JudgmentKind, RejectCategory};
use serde_json::Value;

use crate::addr::{Root, Stamp, StepUid};
use crate::effect::{apply_effects, ConditionEval};
use crate::event::{Cause, Event};
use crate::session::RunState;
use crate::step_status::StepStatus;

/// 受け付けた自然文。
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedText {
    /// 受講者の文。原文のまま（`trim`・正規化をしない）。未信頼の値で、HTML への埋め込みは web 層の責務である。
    pub text: String,
    /// 当たった規則の `id`。規則が無い、または `id` が文字列でなければ `None`。
    pub rule_id: Option<String>,
    /// 適用する効果の列。当たった規則の `outcome.effects` で、規則が無ければ空。
    pub effects: Vec<EffectItem>,
}

/// 自然文の判定の結果。
#[derive(Debug, Clone, PartialEq)]
pub enum TextJudgment {
    /// 受付（分類は `free_text_unjudged`）。
    Accepted(AcceptedText),
    /// 範囲外（空・下限未満・上限超過）。分類は `bad_arg_shape`。
    Rejected(TextVerdict),
}

impl TextJudgment {
    /// 判定結果の種類。
    pub fn kind(&self) -> JudgmentKind {
        match self {
            Self::Accepted(_) => JudgmentKind::Accepted,
            Self::Rejected(_) => JudgmentKind::Rejected,
        }
    }

    /// 付ける分類。受付は `free_text_unjudged`、拒否は `bad_arg_shape`。
    pub fn category(&self) -> Category {
        match self {
            Self::Accepted(_) => Category::FreeTextUnjudged,
            Self::Rejected(_) => Category::BadArgShape,
        }
    }

    /// 拒否の分類。受付は `None`（`free_text_unjudged` は拒否の分類ではない）。
    pub fn reject_category(&self) -> Option<RejectCategory> {
        match self {
            Self::Accepted(_) => None,
            Self::Rejected(_) => Some(RejectCategory::BadArgShape),
        }
    }
}

/// Step の宣言を読めない（IR の不正）。表示は固定文で、教材・受講者の値を含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextDeclError {
    /// Step の入力方式が `text` ではない。
    NotText,
    /// `input` が `TextSpec` として読めない、または検査に反する。
    InvalidSpec,
    /// 当たった規則の効果が語彙として読めない。
    InvalidEffect {
        /// 当たった規則の `effects` の添字（0 始まり）。
        item: usize,
    },
}

impl std::fmt::Display for TextDeclError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotText => f.write_str("step input is not a text input"),
            Self::InvalidSpec => f.write_str("text input declaration is invalid"),
            Self::InvalidEffect { item } => {
                write!(f, "rule effect at index {item} is not a valid effect")
            }
        }
    }
}

impl std::error::Error for TextDeclError {}

/// Step の `input` を `TextSpec` として読む。`kind` が `text` のオブジェクトだけを受ける。
pub fn text_spec(step: &Step) -> Result<TextSpec, TextDeclError> {
    let obj = step
        .input
        .as_ref()
        .and_then(Value::as_object)
        .ok_or(TextDeclError::NotText)?;
    if obj.get("kind").and_then(Value::as_str) != Some("text") {
        return Err(TextDeclError::NotText);
    }
    let mut rest = obj.clone();
    rest.remove("kind");
    let spec: TextSpec =
        serde_json::from_value(Value::Object(rest)).map_err(|_| TextDeclError::InvalidSpec)?;
    spec.check().map_err(|_| TextDeclError::InvalidSpec)?;
    Ok(spec)
}

fn is_text_accept_rule(rule: &Value) -> bool {
    let on_text = rule
        .get("on")
        .and_then(Value::as_object)
        .is_some_and(|on| on.get("text") == Some(&Value::Bool(true)));
    let accepts = rule
        .get("outcome")
        .and_then(|o| o.get("kind"))
        .and_then(Value::as_str)
        == Some("accept");
    on_text && accepts
}

/// 自然文を判定し、受付なら当たった規則の効果を型にして返す。
///
/// 規則は読み取りだけで、状態は変えない。適用は [`accept_text`] が行う。
pub fn judge_text(
    step: &Step,
    text: &str,
    run: &RunState,
    root: &Root,
    cond: &dyn ConditionEval,
) -> Result<TextJudgment, TextDeclError> {
    let verdict = text_spec(step)?.judge(text);
    if verdict != TextVerdict::Accepted {
        return Ok(TextJudgment::Rejected(verdict));
    }
    let hit = step
        .rules
        .iter()
        .filter(|r| is_text_accept_rule(r))
        .find(|r| match r.get("when") {
            None => true,
            Some(when) => cond.eval(when, run, root) == Some(true),
        });
    let mut accepted = AcceptedText {
        text: text.to_string(),
        rule_id: None,
        effects: Vec::new(),
    };
    if let Some(rule) = hit {
        accepted.rule_id = rule.get("id").and_then(Value::as_str).map(str::to_string);
        if let Some(items) = rule
            .get("outcome")
            .and_then(|o| o.get("effects"))
            .and_then(Value::as_array)
        {
            for (item, v) in items.iter().enumerate() {
                let e =
                    EffectItem::from_value(v).map_err(|_| TextDeclError::InvalidEffect { item })?;
                accepted.effects.push(e);
            }
        }
    }
    Ok(TextJudgment::Accepted(accepted))
}

/// 受付を `run` に適用する。Step を `Accepted` にし、`Event::Accepted` に続けて規則の効果のイベントを返す。
///
/// `Achieved` にする経路を持たない（受付だけでは達成にならない）。`current`・`busy`・`completed`・`generation` は
/// 変えず、`Achieved`・`Transitioned`・`ChapterCompleted` を出さない。操作 1 件の `Changed` の集約は
/// `session::apply` が行う前提で、ここは語ごとの `Changed` を返す。
pub fn accept_text(
    run: &mut RunState,
    uid: &StepUid,
    accepted: &AcceptedText,
    at: &Stamp,
    cond: &dyn ConditionEval,
) -> Vec<Event> {
    let root = uid.root();
    run.slices
        .entry(root.clone())
        .or_default()
        .status
        .insert(uid.clone(), StepStatus::Accepted);
    let mut events = vec![Event::Accepted { at: at.clone() }];
    events.extend(apply_effects(
        run,
        &root,
        &accepted.effects,
        at,
        Cause::Input,
        cond,
    ));
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn step(input: Option<Value>) -> Step {
        let mut v = json!({ "id": "s", "title": "t" });
        if let Some(i) = input {
            v["input"] = i;
        }
        serde_json::from_value(v).expect("step")
    }

    #[test]
    fn reads_text_spec() {
        let s = step(Some(json!({ "kind": "text", "min": 1, "max": 5 })));
        assert_eq!(text_spec(&s), Ok(TextSpec { min: 1, max: 5 }));
    }

    #[test]
    fn rejects_non_text_and_bad_spec() {
        assert_eq!(text_spec(&step(None)), Err(TextDeclError::NotText));
        let bad = step(Some(json!({ "kind": "text", "min": 5, "max": 1 })));
        assert_eq!(text_spec(&bad), Err(TextDeclError::InvalidSpec));
    }
}
