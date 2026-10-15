//! 入力・進行の判定結果の統合と `Judged` の材料（JUDGE-3、TASK-267）。
//!
//! 入力 1 件の判定結果は、受付・拒否（分類つき）・問い返し・確認・進行・無視の 6 種のどれか 1 つになり、
//! `Event::Judged` として出る。問い返しは拒否の 9 つ目の分類ではなく、結果の種類の 1 つである。
//! 結果の型は contract の `judgment::Judgment` が正で、ここでは再定義しない。
//!
//! 呼び出し元は、`session::apply` の入力の分岐（TASK-207）・進行の分岐（TASK-210）・教材テスト（pack）である。
//! 呼び出し先は、コマンドの字句化と照合（[`crate::command_lex`]・[`crate::command_judge`]）、選択・画面操作・承認の
//! 判定（[`crate::select_judge`]）、自然文の判定（[`crate::text_judge`]）である。本モジュールは、それらの結果を
//! 1 つの [`Judgment`] にまとめ、拒否・問い返しの文面の ID と次の行動を IR から解決する純関数を置く。
//! `&RunState` を読むだけで状態を変えず、I/O・時刻・乱数・panic を持たない。計算量は規則の数・字句の数・宣言の
//! 件数に線形で、正規表現は評価しない。
//!
//! # 判定の順序（最初に当たったものを返す）
//!
//! 1. 現在の Step が無い、または IR で引けない: 無視（[`IgnoreReason::NoCurrent`]）
//! 2. 処理中（`run.busy`）: 拒否 `busy`（暫定。精緻化は TASK-269、JUDGE-5）
//! 3. コマンドで、教材のコマンドの表に名前が無い: 拒否 `unknown_command`
//! 4. Step の状態で段を分ける。`Entered` は入力の段、`Accepted` は確認の段、それ以外は拒否 `not_in_this_step`
//!
//! 入力の段では、コマンドは宣言との照合の後に `rules` を上から読み、自然文は範囲の判定の後に `on.text` の規則を読み、
//! 選択・画面操作・承認は [`crate::select_judge::judge_select`] に渡す。取り消しは `on.cancel` が真の規則があれば
//! その結果に従い、無ければ `not_in_this_step` の拒否にする（EVT-10）。
//!
//! # 暫定の割り当て（後続のタスクが直す）
//!
//! 分類の割り当ては [`classify_command`] の 1 か所にまとめる。教材のコマンドの表に無い名前は `unknown_command`、
//! 表にあってもこの Step・この段で使わない名前は `not_in_this_step` とし、`wrong_order`・`not_supported` の
//! 見分けはしない（TASK-268・TASK-269）。問い返しの待ちの状態（EP-4）は持たない。
//!
//! # 文面の扱い（fail-closed）
//!
//! 文面は作らない。拒否・問い返しの文面の ID、または次の行動を IR から解決できない場合は、固定文の
//! [`JudgeError`] を返し、受付や固定の代替文には倒さない。`{{slot}}` の差し込み（EP-7）は行わず、IR の値を
//! そのまま [`NextAction`] にする（実行時の差し込みは TASK-274）。文字列は未信頼の値で、保持するだけである。
//! HTML へのエスケープは web 層の責務とする。存在の保証は pack の検査（`E-REF-REJECTION` など）が担う。
//!
//! 自然文の受付の注記（`free_text_unjudged`）は、文面を IR から解決できなくても受付を妨げず
//! `Accepted(None)` にする。注記は任意の欄で、文面の欠落は pack が検出する。
//!
//! # 扱わないこと
//!
//! - `session::apply` への結線と、受付・達成の遷移（TASK-207）。`Op::Input` では `InputReceived` の直後に
//!   `Judged` を 1 件置き、無視のときは続けて `Event::Ignored`（理由）を置くことを推奨する
//! - 進行（`Op::Tick`）での `Judged` の発行位置。EVT-8 は宛先の食い違いを「無視を 1 件だけ記録」とするため、
//!   `Judged{ignored}` と `Ignored` を両方出すかは TASK-210 が決める。ここは [`judge_tick`] の純関数だけを置く
//! - `Event::Refused.rejection` の型の置き換え、問い返しの待ちの状態

use std::fmt;

use fandhe_workshop_contract::input_spec::{CommandSpec, TextVerdict};
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::{
    AcceptNote, Category, Clarification, Judgment, LexedCommand, NextAction, RejectCategory,
    Rejection,
};
use fandhe_workshop_contract::profile::read_profile;
use serde_json::{Map, Value};

use crate::addr::{Root, Stamp, TickAddr};
use crate::command_judge::{judge_command, CommandVerdict, MatchedCommand, ResolvedArg};
use crate::command_lex::{lex_command, CommandLexError};
use crate::effect::ConditionEval;
use crate::event::{Event, IgnoreReason};
use crate::input::Input;
use crate::select_judge::{
    judge_confirm, judge_select, ConfirmVerdict, SelectJudgeError, SelectVerdict,
};
use crate::session::RunState;
use crate::step_status::StepStatus;
use crate::text_judge::{judge_text, text_spec, AcceptedText, TextDeclError, TextJudgment};

/// 判定できない理由。`Display` は入力値・宣言の値を埋め込まない固定の英文である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum JudgeError {
    /// `Step.input`・`rules`・`confirm`・`pack.profile` が読めない。
    InvalidDeclaration,
    /// 文面の ID を `rejections` で引けない。
    UnknownRejection,
    /// 文面の ID を `clarifications` で引けない。
    UnknownClarification,
    /// `rejections` の `category` が欠けている・読めない・`free_text_unjudged` である。
    InvalidRejectionCategory,
    /// 次の行動が欠けている、または空である。
    EmptyNextAction,
    /// 分類に対応する文面の ID が `on_reject` にも `defaults` にも無い。
    NoRejectionText,
}

impl fmt::Display for JudgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDeclaration => "step declaration cannot be read",
            Self::UnknownRejection => "rejection text id is not defined",
            Self::UnknownClarification => "clarification text id is not defined",
            Self::InvalidRejectionCategory => "rejection category is missing or invalid",
            Self::EmptyNextAction => "next action is missing or empty",
            Self::NoRejectionText => "no rejection text is defined for the category",
        })
    }
}

impl std::error::Error for JudgeError {}

/// 呼び出し側が状態を変えるのに要る付帯情報。
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum JudgeDetail {
    /// 付帯情報なし。
    None,
    /// 受付をもたらした規則。効果の適用は呼び出し側（TASK-207）。
    Rule {
        /// 一致した規則の `id`（文字列でなければ `None`）。
        rule_id: Option<String>,
    },
    /// 受け付けた自然文と、その効果。
    Text(AcceptedText),
    /// 無視の理由。続けて `Event::Ignored` に載せる。
    Ignored(IgnoreReason),
}

/// 判定の結果。`Judged` に載せる [`Judgment`] と、付帯情報。
#[derive(Debug, Clone, PartialEq)]
pub struct JudgeOutcome {
    /// 判定結果。6 種のどれか 1 つ。
    pub judgment: Judgment,
    /// 付帯情報。
    pub detail: JudgeDetail,
}

impl JudgeOutcome {
    fn new(judgment: Judgment, detail: JudgeDetail) -> Self {
        Self { judgment, detail }
    }

    fn ignored(reason: IgnoreReason) -> Self {
        Self::new(Judgment::Ignored, JudgeDetail::Ignored(reason))
    }
}

/// `Event::Judged` を作る。
pub fn judged_event(at: Stamp, judgment: Judgment) -> Event {
    Event::Judged { at, judgment }
}

/// 進行の宛先を現在の状態と照らし、進行か無視かを決める。
///
/// 3 要素（世代・Step・番号）が現在の状態と一致したときだけ進行になる。食い違いは `Stale`、現在の Step が無ければ
/// `NoCurrent`、処理中でなければ `NoProcessing`（宛先の食い違いが先。EVT-8）。発行位置は TASK-210 が決める。
pub fn judge_tick(run: &RunState, addr: &TickAddr) -> JudgeOutcome {
    let Some(current) = &run.current else {
        return JudgeOutcome::ignored(IgnoreReason::NoCurrent);
    };
    if addr.generation != run.generation || addr.step != *current {
        return JudgeOutcome::ignored(IgnoreReason::Stale);
    }
    let Some(busy) = &run.busy else {
        return JudgeOutcome::ignored(IgnoreReason::NoProcessing);
    };
    if addr.index != busy.done {
        return JudgeOutcome::ignored(IgnoreReason::Stale);
    }
    JudgeOutcome::new(Judgment::Progressed, JudgeDetail::None)
}

/// 入力 1 件を判定する。順序と暫定の割り当ては、モジュール文書のとおり。
pub fn judge_input(
    ir: &Ir,
    run: &RunState,
    input: &Input,
    cond: &dyn ConditionEval,
) -> Result<JudgeOutcome, JudgeError> {
    let Some(uid) = &run.current else {
        return Ok(JudgeOutcome::ignored(IgnoreReason::NoCurrent));
    };
    let Some(node) = ir.nodes.iter().find(|n| n.uid == uid.0) else {
        return Ok(JudgeOutcome::ignored(IgnoreReason::NoCurrent));
    };
    let step = &node.step;
    let root = uid.root();
    let ctx = Ctx {
        ir,
        step,
        run,
        root: &root,
        cond,
    };

    if run.busy.is_some() {
        return ctx.reject(RejectCategory::Busy);
    }

    let lexed = match input {
        Input::Command(raw) => match ctx.lex(raw)? {
            Lexed::Command(l) => {
                if classify_command(ir, &l) == Some(RejectCategory::UnknownCommand) {
                    return ctx.reject(RejectCategory::UnknownCommand);
                }
                Some(l)
            }
            Lexed::NotUsable => return ctx.reject(RejectCategory::NotInThisStep),
        },
        _ => None,
    };

    let status = run
        .slices
        .get(&root)
        .and_then(|s| s.status.get(uid))
        .copied()
        .unwrap_or_default();
    match status {
        StepStatus::Entered => ctx.entered(input, lexed.as_ref()),
        StepStatus::Accepted => ctx.accepted(input),
        _ => ctx.reject(RejectCategory::NotInThisStep),
    }
}

/// コマンドの分類の暫定の割り当て（D9）。教材のコマンドの表に無い名前は `unknown_command`。
/// 表にある名前は、ここでは分類を決めず（`None`）、Step の宣言との照合に任せる。
/// `wrong_order`・`not_supported` の見分けは TASK-268・TASK-269 がこの 1 か所を直す。
fn classify_command(ir: &Ir, lexed: &LexedCommand) -> Option<RejectCategory> {
    if crate::command_judge::is_unknown_command(ir.commands.keys().map(String::as_str), &lexed.name)
    {
        Some(RejectCategory::UnknownCommand)
    } else {
        None
    }
}

enum Lexed {
    Command(LexedCommand),
    NotUsable,
}

/// 規則の結果の読み出し。
enum RuleOutcome {
    Accept,
    Reject(String),
    Clarify(String),
}

struct Ctx<'a> {
    ir: &'a Ir,
    step: &'a Step,
    run: &'a RunState,
    root: &'a Root,
    cond: &'a dyn ConditionEval,
}

impl Ctx<'_> {
    fn lex(&self, raw: &str) -> Result<Lexed, JudgeError> {
        let prefix = read_profile(self.ir.pack.profile.as_ref())
            .map_err(|_| JudgeError::InvalidDeclaration)?
            .command_prefix;
        Ok(match lex_command(&prefix, raw) {
            Ok(l) => Lexed::Command(l),
            // 接頭辞が無い・字句が不正な行は、この Step で使えない入力として扱う（試作と同じ）。
            Err(CommandLexError::NotCommand | CommandLexError::Malformed(_)) => Lexed::NotUsable,
        })
    }

    fn when_ok(&self, rule: &Value) -> bool {
        match rule.get("when") {
            None | Some(Value::Null) => true,
            Some(w) => self.cond.eval(w, self.run, self.root) == Some(true),
        }
    }

    /// 分類からの拒否。文面は `on_reject`、無ければ `defaults` で引く（pack の `resolve_rejection` と同じ順）。
    fn reject(&self, category: RejectCategory) -> Result<JudgeOutcome, JudgeError> {
        let key = category.as_str();
        let text_id = self
            .step
            .on_reject
            .get(key)
            .or_else(|| self.ir.defaults.get(key))
            .ok_or(JudgeError::NoRejectionText)?;
        let entry = self
            .ir
            .rejections
            .get(text_id)
            .ok_or(JudgeError::UnknownRejection)?;
        let next_action = next_action_of(entry, "next_action")?;
        Ok(JudgeOutcome::new(
            Judgment::Rejected(Rejection {
                category,
                text: text_id.clone(),
                next_action,
            }),
            JudgeDetail::None,
        ))
    }

    /// 文面の ID からの拒否。分類は `rejections` の `category` が持つ。
    fn reject_by_id(&self, id: &str) -> Result<JudgeOutcome, JudgeError> {
        let entry = self
            .ir
            .rejections
            .get(id)
            .ok_or(JudgeError::UnknownRejection)?;
        let category = entry
            .get("category")
            .and_then(Value::as_str)
            .and_then(Category::from_str_opt)
            .and_then(|c| RejectCategory::try_from(c).ok())
            .ok_or(JudgeError::InvalidRejectionCategory)?;
        let next_action = next_action_of(entry, "next_action")?;
        Ok(JudgeOutcome::new(
            Judgment::Rejected(Rejection {
                category,
                text: id.to_string(),
                next_action,
            }),
            JudgeDetail::None,
        ))
    }

    fn clarify_by_id(&self, id: &str) -> Result<JudgeOutcome, JudgeError> {
        let entry = self
            .ir
            .clarifications
            .get(id)
            .ok_or(JudgeError::UnknownClarification)?;
        let next_action = next_action_of(entry, "how_to_answer")?;
        Ok(JudgeOutcome::new(
            Judgment::Clarified(Clarification {
                text: id.to_string(),
                next_action,
            }),
            JudgeDetail::None,
        ))
    }

    /// 一致した規則の結果から判定を作る。
    fn judge_by_rule(
        &self,
        outcome: RuleOutcome,
        rule_id: Option<String>,
    ) -> Result<JudgeOutcome, JudgeError> {
        match outcome {
            RuleOutcome::Accept => Ok(JudgeOutcome::new(
                Judgment::Accepted(None),
                JudgeDetail::Rule { rule_id },
            )),
            RuleOutcome::Reject(id) => self.reject_by_id(&id),
            RuleOutcome::Clarify(id) => self.clarify_by_id(&id),
        }
    }

    fn entered(
        &self,
        input: &Input,
        lexed: Option<&LexedCommand>,
    ) -> Result<JudgeOutcome, JudgeError> {
        match (input, lexed) {
            (Input::Command(_), Some(l)) => self.command_stage(l),
            (Input::Command(_), None) => self.reject(RejectCategory::NotInThisStep),
            (Input::Text(text), _) => self.text_stage(text),
            (Input::Cancel, _) => self.cancel_stage(),
            (Input::Choice(_) | Input::Ui { .. } | Input::Approval { .. }, _) => {
                // `when: null` は条件なし（command・text・cancel の `when_ok` と同じ）。
                let when = |w: &Value| {
                    if w.is_null() {
                        Some(true)
                    } else {
                        self.cond.eval(w, self.run, self.root)
                    }
                };
                match judge_select(self.step, input, &when).map_err(select_err)? {
                    SelectVerdict::Accepted { rule_id, .. } => Ok(JudgeOutcome::new(
                        Judgment::Accepted(None),
                        JudgeDetail::Rule {
                            rule_id: Some(rule_id),
                        },
                    )),
                    SelectVerdict::RejectedByRule { rejection, .. } => {
                        self.reject_by_id(&rejection)
                    }
                    SelectVerdict::Clarified { clarification, .. } => {
                        self.clarify_by_id(&clarification)
                    }
                    SelectVerdict::Rejected(c) => self.reject(c),
                }
            }
        }
    }

    fn accepted(&self, input: &Input) -> Result<JudgeOutcome, JudgeError> {
        match input {
            Input::Choice(_) | Input::Ui { .. } | Input::Approval { .. } => {
                match judge_confirm(self.step, input).map_err(select_err)? {
                    ConfirmVerdict::Confirmed => {
                        Ok(JudgeOutcome::new(Judgment::Confirmed, JudgeDetail::None))
                    }
                    ConfirmVerdict::WrongAnswer { text, .. } | ConfirmVerdict::Missed { text } => {
                        self.reject_by_id(&text)
                    }
                    ConfirmVerdict::Rejected(c) => self.reject(c),
                }
            }
            Input::Cancel => self.cancel_stage(),
            Input::Command(_) | Input::Text(_) => self.reject(RejectCategory::NotInThisStep),
        }
    }

    /// 宣言の `kind` が `command` のときだけ `CommandSpec` を読む。別の方式・宣言なしは `None`。
    fn command_spec(&self) -> Result<Option<CommandSpec>, JudgeError> {
        let Some(obj) = self.step.input.as_ref().and_then(Value::as_object) else {
            return Ok(None);
        };
        if obj.get("kind").and_then(Value::as_str) != Some("command") {
            return Ok(None);
        }
        let mut rest = obj.clone();
        rest.remove("kind");
        let spec: CommandSpec = serde_json::from_value(Value::Object(rest))
            .map_err(|_| JudgeError::InvalidDeclaration)?;
        spec.check().map_err(|_| JudgeError::InvalidDeclaration)?;
        Ok(Some(spec))
    }

    fn command_stage(&self, lexed: &LexedCommand) -> Result<JudgeOutcome, JudgeError> {
        let Some(spec) = self.command_spec()? else {
            return self.reject(RejectCategory::NotInThisStep);
        };
        match judge_command(&spec, lexed) {
            CommandVerdict::NameMismatch => self.reject(RejectCategory::NotInThisStep),
            CommandVerdict::MissingArg => self.reject(RejectCategory::MissingArg),
            CommandVerdict::BadArgShape => self.reject(RejectCategory::BadArgShape),
            CommandVerdict::Matched(m) => {
                for rule in &self.step.rules {
                    let rule = rule.as_object().ok_or(JudgeError::InvalidDeclaration)?;
                    let Some(on) = rule.get("on").and_then(Value::as_object) else {
                        return Err(JudgeError::InvalidDeclaration);
                    };
                    if !command_rule_matches(on, &spec.name, &m)? || !self.when_ok_obj(rule) {
                        continue;
                    }
                    return self.judge_by_rule(rule_outcome(rule)?, rule_id_of(rule));
                }
                self.reject(RejectCategory::NotInThisStep)
            }
        }
    }

    fn when_ok_obj(&self, rule: &Map<String, Value>) -> bool {
        match rule.get("when") {
            None | Some(Value::Null) => true,
            Some(w) => self.cond.eval(w, self.run, self.root) == Some(true),
        }
    }

    fn text_stage(&self, text: &str) -> Result<JudgeOutcome, JudgeError> {
        let spec = match text_spec(self.step) {
            Ok(s) => s,
            Err(TextDeclError::NotText) => return self.reject(RejectCategory::NotInThisStep),
            Err(_) => return Err(JudgeError::InvalidDeclaration),
        };
        if spec.judge(text) != TextVerdict::Accepted {
            return self.reject(RejectCategory::BadArgShape);
        }
        // 最初に当たった `on.text` の規則が拒否・問い返しならそれに従う。受付なら下の自然文の受付へ進む。
        for rule in &self.step.rules {
            if !is_text_rule(rule) || !self.when_ok(rule) {
                continue;
            }
            let rule = rule.as_object().ok_or(JudgeError::InvalidDeclaration)?;
            match rule_outcome(rule)? {
                RuleOutcome::Accept => break,
                other => return self.judge_by_rule(other, rule_id_of(rule)),
            }
        }
        let accepted = match judge_text(self.step, text, self.run, self.root, self.cond)
            .map_err(|_| JudgeError::InvalidDeclaration)?
        {
            TextJudgment::Accepted(a) => a,
            // 範囲は上で確かめ済みで、ここには来ない。来た場合も受付には倒さない。
            TextJudgment::Rejected(_) => return self.reject(RejectCategory::BadArgShape),
        };
        Ok(JudgeOutcome::new(
            Judgment::Accepted(self.free_text_note()),
            JudgeDetail::Text(accepted),
        ))
    }

    /// 自然文の受付の注記。文面を解決できなければ `None`（受付は妨げない）。
    fn free_text_note(&self) -> Option<AcceptNote> {
        let key = Category::FreeTextUnjudged.as_str();
        let text_id = self
            .step
            .on_reject
            .get(key)
            .or_else(|| self.ir.defaults.get(key))?;
        let entry = self.ir.rejections.get(text_id)?;
        let next_action = next_action_of(entry, "next_action").ok()?;
        Some(AcceptNote {
            text: text_id.clone(),
            next_action,
        })
    }

    fn cancel_stage(&self) -> Result<JudgeOutcome, JudgeError> {
        for rule in &self.step.rules {
            let is_cancel = rule
                .get("on")
                .and_then(Value::as_object)
                .is_some_and(|on| on.get("cancel") == Some(&Value::Bool(true)));
            if !is_cancel || !self.when_ok(rule) {
                continue;
            }
            let rule = rule.as_object().ok_or(JudgeError::InvalidDeclaration)?;
            return self.judge_by_rule(rule_outcome(rule)?, rule_id_of(rule));
        }
        self.reject(RejectCategory::NotInThisStep)
    }
}

fn select_err(_: SelectJudgeError) -> JudgeError {
    JudgeError::InvalidDeclaration
}

fn is_text_rule(rule: &Value) -> bool {
    rule.get("on")
        .and_then(Value::as_object)
        .is_some_and(|on| on.get("text") == Some(&Value::Bool(true)))
}

fn rule_id_of(rule: &Map<String, Value>) -> Option<String> {
    rule.get("id").and_then(Value::as_str).map(str::to_string)
}

/// 規則の `outcome` を読む。未知の `kind`・文面の ID の欠落は宣言の誤り。
fn rule_outcome(rule: &Map<String, Value>) -> Result<RuleOutcome, JudgeError> {
    let outcome = rule
        .get("outcome")
        .and_then(Value::as_object)
        .ok_or(JudgeError::InvalidDeclaration)?;
    let text_of = |key: &str| {
        outcome
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or(JudgeError::InvalidDeclaration)
    };
    match outcome.get("kind").and_then(Value::as_str) {
        Some("accept") => Ok(RuleOutcome::Accept),
        Some("reject") => Ok(RuleOutcome::Reject(text_of("rejection")?)),
        Some("clarify") => Ok(RuleOutcome::Clarify(text_of("clarification")?)),
        _ => Err(JudgeError::InvalidDeclaration),
    }
}

fn next_action_of(entry: &Value, key: &str) -> Result<NextAction, JudgeError> {
    let s = entry
        .get(key)
        .and_then(Value::as_str)
        .ok_or(JudgeError::EmptyNextAction)?;
    NextAction::new(s).map_err(|_| JudgeError::EmptyNextAction)
}

fn normalize_key(key: &str) -> String {
    key.replace('-', "_")
}

/// `on` に書かれた引数名を、宣言済みの引数へ解決する。宣言名との完全一致を先に取り、
/// 無ければ `-` と `_` の違いだけで一致する宣言がちょうど 1 つのときに限りそれを採る。
/// 未宣言・曖昧な名前は宣言の誤りとし、`a-b` と `a_b` を取り違えて別の引数に当てない。
fn resolve_declared<'a>(m: &'a MatchedCommand, arg: &str) -> Result<&'a ResolvedArg, JudgeError> {
    if let Some(a) = m.args.iter().find(|a| a.name == arg) {
        return Ok(a);
    }
    let key = normalize_key(arg);
    let mut found = m.args.iter().filter(|a| normalize_key(&a.name) == key);
    match (found.next(), found.next()) {
        (Some(a), None) => Ok(a),
        _ => Err(JudgeError::InvalidDeclaration),
    }
}

/// `on` が `null` の欄を「無い」とみなして、コマンドに一致するか。
/// 解釈するキーは `command`・`args`・`missing` だけで、ほかに値のあるキーがあれば一致しない。
/// 型違い・未宣言の引数名は不一致に丸めず `InvalidDeclaration` にし、後続の受付の規則へ進ませない。
fn command_rule_matches(
    on: &Map<String, Value>,
    name: &str,
    m: &MatchedCommand,
) -> Result<bool, JudgeError> {
    let mut command_ok = false;
    let mut matched = true;
    for (k, v) in on {
        if v.is_null() {
            continue;
        }
        match k.as_str() {
            "command" => match v.as_str() {
                Some(c) if c == name => command_ok = true,
                Some(_) => matched = false,
                None => return Err(JudgeError::InvalidDeclaration),
            },
            "args" => {
                let map = v.as_object().ok_or(JudgeError::InvalidDeclaration)?;
                for (arg, want) in map {
                    let want = want.as_str().ok_or(JudgeError::InvalidDeclaration)?;
                    if resolve_declared(m, arg)?.value.as_deref() != Some(want) {
                        matched = false;
                    }
                }
            }
            "missing" => {
                let list = v.as_array().ok_or(JudgeError::InvalidDeclaration)?;
                for item in list {
                    let arg = item.as_str().ok_or(JudgeError::InvalidDeclaration)?;
                    if resolve_declared(m, arg)?.value.is_some() {
                        matched = false;
                    }
                }
            }
            _ => matched = false,
        }
    }
    Ok(matched && command_ok)
}
