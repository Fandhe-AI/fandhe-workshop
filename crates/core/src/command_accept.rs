//! 受け付けたうえで止まる場面の判定（JUDGE-7、TASK-273）。
//!
//! 教材は、上流の挙動として「入力を受け付けたうえで処理が止まる」場面を宣言できる（試作の ST-7.3 は、事前確認を
//! 飛ばした `/implement-issue-tree` が CI が通っても止まる）。この入力は拒否ではなく受付で、結果として世界状態が
//! 止まった状態になる。止まった状態は教材の宣言（規則の効果）だけで表し、エンジンは `blocked`・`stopped` という語を
//! 知らない。必須引数が欠けた入力（ST-7.1）は [`crate::command_judge`] の段で `missing_arg` の拒否になり、
//! 規則を読まないため、2 つは別の結果として区別される。
//!
//! 呼び出し元は、`session::apply` の入力の分岐（TASK-207・TASK-267）と教材テスト（TASK-404、pack）である。
//! [`crate::command_rule::judge_command_step`] が返した `Accepted` を受け、規則の効果の読み出し
//! （[`accepted_command`]）と適用（[`accept_command`]）を、I/O・時刻・乱数・正規表現・panic を持たない
//! 決定的な関数として置く。判定（読み取り）と適用を分ける形は [`crate::text_judge`] にそろえている。
//!
//! # 規則
//!
//! 1. 判定が `Accepted` でなければ [`CommandAcceptError::NotAccepted`] にする
//! 2. `rule_index` の規則が無い・オブジェクトでない・`id` が判定と違う・`outcome.kind` が `accept` でなければ
//!    [`CommandAcceptError::InvalidDeclaration`] にする（判定と宣言の食い違いを受付へ倒さない）
//! 3. `effects` が 1 項目でも語彙として読めなければ [`CommandAcceptError::InvalidEffect`] にする。読み出しはすべて
//!    適用の前に終えるため、一部だけ適用する状態は作らない。配列でなければ [`CommandAcceptError::InvalidEffects`]
//!    （省略は効果なしの正常な宣言）
//! 4. 適用は Step を `Accepted` にし、`Event::Accepted` に続けて効果のイベントを並べる。`Achieved`・`Transitioned`・
//!    `ChapterCompleted` を出さず、`current`・`busy`・`completed`・`generation` を変えない
//!
//! 受講者の入力値は照合にだけ使い、状態変化の経路・属性へ差し込まない（効果の値は宣言の固定値）。
//! エラーの `Display` は固定の英文で、入力値・宣言の値を含めない。計算量は規則の件数と効果の件数に比例する。
//!
//! # 扱わないこと
//!
//! - `Judged` の発行と `session::apply` への結線（TASK-207・TASK-267）
//! - `rejections[id].category` の解決、文面の解決・`NextAction` の生成（TASK-267・TASK-274）
//! - `on_achieved`、確認による達成への遷移（TASK-207・TASK-306）

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::{Ir, Step};
use serde_json::Value;

use crate::addr::{Stamp, StepUid};
use crate::command_rule::CommandRuleVerdict;
use crate::effect::ConditionEval;
use crate::event::Event;
use crate::session::RunState;
use crate::text_judge::apply_acceptance;

/// 規則が受け付けた入力。適用する効果を型にして持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptedCommand {
    /// 一致した規則の `id`。
    pub rule_id: String,
    /// 適用する効果の列。規則の `outcome.effects` で、省略なら空。
    pub effects: Vec<EffectItem>,
}

/// 受付の効果を読めない理由。表示は固定文で、教材・受講者の値を含めない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CommandAcceptError {
    /// 渡された判定が `Accepted` ではない。
    NotAccepted,
    /// 判定の規則が宣言に無い、または宣言と食い違う。
    InvalidDeclaration,
    /// `effects` があるのに配列ではない。
    InvalidEffects,
    /// 添字 `item` の効果が語彙として読めない。
    InvalidEffect {
        /// `effects` の添字（0 始まり）。
        item: usize,
    },
}

impl std::fmt::Display for CommandAcceptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAccepted => f.write_str("verdict is not an accepted rule"),
            Self::InvalidDeclaration => f.write_str("accepted rule does not match the declaration"),
            Self::InvalidEffects => f.write_str("rule effects is not an array"),
            Self::InvalidEffect { item } => {
                write!(f, "rule effect at index {item} is not a valid effect")
            }
        }
    }
}

impl std::error::Error for CommandAcceptError {}

/// 規則が `Accepted` のとき、その規則の効果を読んで返す。状態は変えない。
pub fn accepted_command(
    step: &Step,
    verdict: &CommandRuleVerdict,
) -> Result<AcceptedCommand, CommandAcceptError> {
    let CommandRuleVerdict::Accepted {
        rule_index,
        rule_id,
    } = verdict
    else {
        return Err(CommandAcceptError::NotAccepted);
    };
    let rule = step
        .rules
        .get(*rule_index)
        .and_then(Value::as_object)
        .ok_or(CommandAcceptError::InvalidDeclaration)?;
    let outcome = rule.get("outcome");
    let consistent = rule.get("id").and_then(Value::as_str) == Some(rule_id.as_str())
        && outcome.and_then(|o| o.get("kind")).and_then(Value::as_str) == Some("accept");
    if !consistent {
        return Err(CommandAcceptError::InvalidDeclaration);
    }
    let mut effects = Vec::new();
    // `effects` の省略は効果なしの正常な宣言。キーがあって配列でなければ宣言の誤りで、握りつぶさない。
    if let Some(raw) = outcome.and_then(|o| o.get("effects")) {
        let items = raw.as_array().ok_or(CommandAcceptError::InvalidEffects)?;
        for (item, v) in items.iter().enumerate() {
            let e = EffectItem::from_value(v)
                .map_err(|_| CommandAcceptError::InvalidEffect { item })?;
            effects.push(e);
        }
    }
    Ok(AcceptedCommand {
        rule_id: rule_id.clone(),
        effects,
    })
}

/// 受付を `run` に適用する。Step を `Accepted` にし、`Event::Accepted` に続けて規則の効果のイベントを返す。
///
/// `Achieved` にする経路を持たない（受付だけでは達成にならない）。`current`・`busy`・`completed`・`generation` は
/// 変えず、`Achieved`・`Transitioned`・`ChapterCompleted` を出さない。操作 1 件の `Changed` の集約は
/// `session::apply` が行う前提で、ここは語ごとの `Changed` を返す。
pub fn accept_command(
    ir: &Ir,
    run: &mut RunState,
    uid: &StepUid,
    accepted: &AcceptedCommand,
    at: &Stamp,
    cond: &dyn ConditionEval,
) -> Vec<Event> {
    apply_acceptance(ir, run, uid, &accepted.effects, at, cond)
}
