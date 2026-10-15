//! コマンド入力の規則の照合と問い返しの判定（JUDGE-6、TASK-272）。
//!
//! 教材は「引数なしの `/new-idea`」のような入力を、拒否ではなく問い返しとして扱うと宣言できる
//! （`on: { command, missing }` と `outcome: { kind: clarify, clarification }`）。本モジュールは、
//! 字句化の結果（[`crate::command_lex`]）と宣言との照合（[`crate::command_judge`]）に続く段として、
//! `Step.rules` を上から照合して最初に一致した規則の `outcome` を返す純関数を置く。
//! 呼び出し元は、`Judged` の発行と受付・拒否の遷移（TASK-267・TASK-207）、規則による受付の適用（TASK-273）である。
//! 結果の型の語彙は `fandhe_workshop_contract::judgment`、宣言の型は `input_spec` が正で、ここでは再定義しない。
//!
//! # 規則
//!
//! - 照合の前段は [`judge_command`] で、宣言に合わなかった結果（名前の不一致・必須引数の欠け・引数の形の誤り）は
//!   規則を読まずに返す。必須の引数が欠けた入力は問い返しにならず、拒否の `missing_arg` のままである
//!   （問い返しで拒否を迂回させない）
//! - 規則は上から順に読み、最初に一致したものの `outcome` を結果にする
//! - `on.command` が無い・`null` の規則（取り消しなど）と、名前が違う規則は飛ばす（大文字小文字を区別する完全一致）
//! - `option`・`approval`・`target`・`ui`・`text`・`cancel` が `null` でない規則は、コマンドの規則ではないので一致しない
//! - `on.missing` は、書かれた引数がすべて値なしのときだけ一致する。無い・`null`・空は条件なし。
//!   宣言に無い引数の名前は [`CommandRuleError::InvalidDeclaration`] にする
//! - `on.args` は、中身があれば [`CommandRuleError::UnsupportedCondition`] にする（意味は TASK-267 が決める）
//! - `when` は `null` か無ければ条件なし。評価器が `Some(true)` を返したときだけ採り、`Some(false)`・`None` は飛ばす
//! - `null` の値は「無い」とみなす。コンパイル前の古い形の IR は、使わない照合キーを `null` で並べるため
//! - 未知の `outcome.kind` と型の合わない欄は [`CommandRuleError::InvalidDeclaration`] で、受付へは倒さない
//!
//! # 状態を変えない
//!
//! 問い返しの待ちを状態として保持することは、本モジュールの対象外である。関数は `RunState` を受け取らないため、
//! 問い返しになった Step が受付済みになることは型の上で起こらない。待ちの保持は、公開 API の契約の版を上げる
//! 変更になるため `Session::apply` への結線（TASK-207・TASK-267）と一緒に決める。
//!
//! 外部への呼び出し（ネットワーク・ファイル・時刻・乱数・LLM）と正規表現の評価は持たない。全域（panic しない）で
//! 決定的で、計算量は規則の件数と引数の宣言の件数の積に比例する。エラーの `Display` は固定の英文で、
//! 入力値・宣言の値を含めない。
//!
//! # 扱わないこと
//!
//! - `Judged` の発行・`session::apply` への結線・規則の効果の適用（TASK-267・TASK-273・TASK-207）
//! - 規則に当たらないとき（[`CommandRuleVerdict::NoRule`]）と名前の不一致の拒否の分類（TASK-268・TASK-269）
//! - `rejections[id].category` の解決、文面の解決・差し込み・`NextAction` の生成（TASK-267・TASK-274）

use std::fmt;

use fandhe_workshop_contract::input_spec::{CommandSpec, InputKind};
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{JudgmentKind, LexedCommand, RejectCategory};
use serde_json::{Map, Value};

use crate::command_judge::{judge_command, CommandVerdict, MatchedCommand};

/// 規則の照合の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandRuleVerdict {
    /// 規則が受付を返した。効果の適用は呼び出し側（TASK-273）。
    Accepted {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
    },
    /// 規則が拒否を返した。
    RejectedByRule {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
        /// `rejections` の ID（分類の解決は呼び出し側）。
        rejection: String,
    },
    /// 規則が問い返しを返した。拒否ではない。
    Clarified {
        /// 一致した規則の添字。
        rule_index: usize,
        /// 一致した規則の ID。
        rule_id: String,
        /// `clarifications` の ID。
        clarification: String,
    },
    /// 一致する規則が無い。分類は TASK-268・TASK-269 が決める。
    NoRule,
}

impl CommandRuleVerdict {
    /// 判定結果の種類。一致する規則が無いときは `None`。
    pub fn kind(&self) -> Option<JudgmentKind> {
        match self {
            Self::Accepted { .. } => Some(JudgmentKind::Accepted),
            Self::RejectedByRule { .. } => Some(JudgmentKind::Rejected),
            Self::Clarified { .. } => Some(JudgmentKind::Clarified),
            Self::NoRule => None,
        }
    }
}

/// コマンド入力の Step 1 件分の判定結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandStepVerdict {
    /// 宣言との照合で止まった（名前の不一致・必須引数の欠け・引数の形の誤り）。規則は読んでいない。
    Declared(CommandVerdict),
    /// 宣言に合い、規則を照合した。
    Rule(CommandRuleVerdict),
}

impl CommandStepVerdict {
    /// 判定結果の種類。名前の不一致と規則に当たらないときは、この層では定まらないので `None`。
    pub fn kind(&self) -> Option<JudgmentKind> {
        match self {
            Self::Declared(v) => v.category().map(|_| JudgmentKind::Rejected),
            Self::Rule(r) => r.kind(),
        }
    }

    /// 分類による拒否の分類。問い返しと受付、規則による拒否（分類は呼び出し側が解決する）は `None`。
    pub fn reject_category(&self) -> Option<RejectCategory> {
        match self {
            Self::Declared(v) => v.category(),
            Self::Rule(_) => None,
        }
    }
}

/// 判定できない理由。`Display` は入力値・宣言の値を埋め込まない固定文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRuleError {
    /// `Step.input` がコマンドの宣言ではない。
    NotCommand,
    /// `input`・`rules` が読めない（型の違い・未知の `outcome.kind`・宣言に無い引数名など）。
    InvalidDeclaration,
    /// 意味が未定の条件（中身のある `on.args`）。黙って一致・不一致にしない。
    UnsupportedCondition,
}

impl fmt::Display for CommandRuleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCommand => f.write_str("step input is not a command"),
            Self::InvalidDeclaration => f.write_str("step declaration cannot be read"),
            Self::UnsupportedCondition => f.write_str("rule condition is not supported"),
        }
    }
}

impl std::error::Error for CommandRuleError {}

/// コマンド以外の入力方式の照合キー。`null` でない値を持つ規則はコマンドの規則ではない。
const OTHER_KEYS: [&str; 6] = ["option", "approval", "target", "ui", "text", "cancel"];

/// `Step.input` をコマンドの宣言として読む。
pub fn command_spec(step: &Step) -> Result<CommandSpec, CommandRuleError> {
    let decl = step
        .input
        .as_ref()
        .and_then(Value::as_object)
        .ok_or(CommandRuleError::NotCommand)?;
    let kind = decl
        .get("kind")
        .and_then(Value::as_str)
        .and_then(InputKind::from_str_opt)
        .ok_or(CommandRuleError::NotCommand)?;
    if kind != InputKind::Command {
        return Err(CommandRuleError::NotCommand);
    }
    let mut map = decl.clone();
    map.remove("kind");
    let spec: CommandSpec = serde_json::from_value(Value::Object(map))
        .map_err(|_| CommandRuleError::InvalidDeclaration)?;
    spec.check()
        .map_err(|_| CommandRuleError::InvalidDeclaration)?;
    Ok(spec)
}

/// `null` を「無い」とみなして欄を引く。
fn present<'a>(map: &'a Map<String, Value>, key: &str) -> Option<&'a Value> {
    map.get(key).filter(|v| !v.is_null())
}

/// `on.missing` の条件。書かれた引数がすべて値なしなら真。
fn missing_holds(
    on: &Map<String, Value>,
    spec: &CommandSpec,
    matched: &MatchedCommand,
) -> Result<bool, CommandRuleError> {
    let Some(v) = present(on, "missing") else {
        return Ok(true);
    };
    let names = v.as_array().ok_or(CommandRuleError::InvalidDeclaration)?;
    let mut all_missing = true;
    for n in names {
        let n = n.as_str().ok_or(CommandRuleError::InvalidDeclaration)?;
        if !spec.args.iter().any(|a| a.name == n) {
            return Err(CommandRuleError::InvalidDeclaration);
        }
        let resolved = matched.args.iter().find(|a| a.name == n);
        if !matches!(resolved, Some(a) if a.value.is_none()) {
            all_missing = false;
        }
    }
    Ok(all_missing)
}

/// 宣言に合ったコマンドを `Step.rules` と上から照合する。
///
/// `when` は規則の `when` の評価器。`Some(true)` のときだけ採り、`Some(false)` と `None` は規則を飛ばす。
/// 純関数であること。状態を受け取らないので、問い返しでも Step は受付済みにならない。
pub fn judge_command_rules(
    step: &Step,
    matched: &MatchedCommand,
    when: &dyn Fn(&Value) -> Option<bool>,
) -> Result<CommandRuleVerdict, CommandRuleError> {
    let spec = command_spec(step)?;
    for (rule_index, rule) in step.rules.iter().enumerate() {
        let rule = rule
            .as_object()
            .ok_or(CommandRuleError::InvalidDeclaration)?;
        let rule_id = rule
            .get("id")
            .and_then(Value::as_str)
            .ok_or(CommandRuleError::InvalidDeclaration)?;
        let on = rule
            .get("on")
            .and_then(Value::as_object)
            .ok_or(CommandRuleError::InvalidDeclaration)?;
        let Some(command) = present(on, "command") else {
            continue;
        };
        let command = command
            .as_str()
            .ok_or(CommandRuleError::InvalidDeclaration)?;
        if command != spec.name || OTHER_KEYS.iter().any(|k| present(on, k).is_some()) {
            continue;
        }
        if !missing_holds(on, &spec, matched)? {
            continue;
        }
        if let Some(args) = present(on, "args") {
            let args = args
                .as_object()
                .ok_or(CommandRuleError::InvalidDeclaration)?;
            if !args.is_empty() {
                return Err(CommandRuleError::UnsupportedCondition);
            }
        }
        if let Some(cond) = present(rule, "when") {
            if when(cond) != Some(true) {
                continue;
            }
        }
        let outcome = rule
            .get("outcome")
            .and_then(Value::as_object)
            .ok_or(CommandRuleError::InvalidDeclaration)?;
        let text_of = |key: &str| {
            outcome
                .get(key)
                .and_then(Value::as_str)
                .map(str::to_string)
                .ok_or(CommandRuleError::InvalidDeclaration)
        };
        let rule_id = rule_id.to_string();
        return match outcome.get("kind").and_then(Value::as_str) {
            Some("accept") => Ok(CommandRuleVerdict::Accepted {
                rule_index,
                rule_id,
            }),
            Some("reject") => Ok(CommandRuleVerdict::RejectedByRule {
                rule_index,
                rule_id,
                rejection: text_of("rejection")?,
            }),
            Some("clarify") => Ok(CommandRuleVerdict::Clarified {
                rule_index,
                rule_id,
                clarification: text_of("clarification")?,
            }),
            _ => Err(CommandRuleError::InvalidDeclaration),
        };
    }
    Ok(CommandRuleVerdict::NoRule)
}

/// コマンド入力の Step 1 件分の判定。宣言との照合に合ったときだけ規則を読む。
pub fn judge_command_step(
    step: &Step,
    lexed: &LexedCommand,
    when: &dyn Fn(&Value) -> Option<bool>,
) -> Result<CommandStepVerdict, CommandRuleError> {
    let spec = command_spec(step)?;
    match judge_command(&spec, lexed) {
        CommandVerdict::Matched(m) => {
            judge_command_rules(step, &m, when).map(CommandStepVerdict::Rule)
        }
        other => Ok(CommandStepVerdict::Declared(other)),
    }
}
