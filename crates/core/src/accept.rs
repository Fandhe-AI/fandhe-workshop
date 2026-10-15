//! 入力の受付の分岐（TASK-207 1/3、EVT-3 の (a)）。
//!
//! 入力が現在の Step の入力条件に合い、受付の規則に当たったら、Step を受付済み（`StepStatus::Accepted`）にして
//! 規則の効果を世界状態へ適用する。受付は達成ではない。達成は確認の操作の後（TASK-207 2/3）であり、
//! 本モジュールは `StepStatus::Achieved` を書かず、`Achieved`・`Transitioned`・`Entered`・`ChapterCompleted` を出さない。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 判定の部品は [`crate::command_lex`]・[`crate::command_judge`]・[`crate::select_judge`]・[`crate::text_judge`]、
//! 効果の適用は [`crate::effect`] が持つ。本モジュールはそれらをつなぐだけで、判定の規則は再実装しない。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無ければ、状態を変えず空のイベント列を返す（`Stamp` が作れない。TASK-217 が決める）
//! 2. 以降は必ず先頭に `InputReceived`（入力は原文のまま）を出す
//! 3. 現在の Step が IR に無ければ、固定文の `Warning` を添えて状態を変えない
//! 4. Step が `Entered` でない、または処理中（`busy`）なら `InputReceived` だけを出す
//!    （受付済みへの再入力と処理中の再送。確認の操作は TASK-207 2/3、`busy` の分類は TASK-209 が担う）
//! 5. `Input::Cancel` は `InputReceived` だけ（意味づけは TASK-212）
//! 6. 入力方式で振り分ける。受付は「受付の規則に一致したとき」だけで、宣言の不正・未知の `kind`・
//!    読めない効果は受付に倒さず、拒否（宣言の不正には固定文の `Warning` を添える）にする（fail-closed）
//!
//! 受付のイベントの順は `InputReceived` → `Accepted` → 効果のイベント → `Changed`。
//! `Changed` は [`crate::session::apply`] が経路ごと 1 件へ集約して末尾に足す。
//!
//! # IR の形の正規化
//!
//! コンパイル済みの IR は、規則の `on` に使わない照合キーを `null`・空の配列・空の表で持ち、`when` も
//! `null` で持つことがある。判定の部品は「余分なキーのある規則は一致しない」ため、規則を読む前に
//! 既知の照合キー（`KNOWN_ON_KEYS`）の空の値だけを取り除いた写しを作る（`normalized`）。未知のキーは
//! 値が空でも残るので、余分なキーの規則が一致しない性質は保たれる。
//!
//! # 扱わないこと
//!
//! - 確認の操作による達成と外れた指し先（TASK-207 2/3）、確認前の達成 0 件の性質テスト（TASK-207 3/3）
//! - `confirm: none` の受付と同時の達成（TASK-208）、処理中の再送の `busy` 分類（TASK-209）
//! - 進行 `Op::Tick` の照合と進め方（TASK-210）、処理中でない Esc の意味づけ（TASK-212）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。
//! `Warning` の文面は固定文で、入力値・教材の値を埋め込まない。入力値は未信頼で、HTML への埋め込みの
//! エスケープは web 層の責務である。

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::input_spec::InputKind;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::profile::read_profile;
use serde_json::Value;

use crate::addr::{Root, Stamp, StepUid};
use crate::command_judge::{command_spec, judge_command, CommandVerdict};
use crate::command_lex::lex_command;
use crate::command_rule::{judge_command_rules, CommandRuleVerdict};
use crate::effect::{apply_effects, ConditionEval, NoConditions};
use crate::event::{Cause, Event};
use crate::input::Input;
use crate::judge::{judge_input, judged_event};
use crate::select_judge::{judge_select, SelectJudgeError, SelectVerdict};
use crate::session::{Busy, RunState, SessionState};
use crate::step_status::StepStatus;
use crate::text_judge::{judge_text, TextJudgment};

/// 宣言が読めなかったときの固定文。入力値・教材の値を含めない。
const INVALID_DECLARATION: &str =
    "input declaration or rule effects could not be read; input rejected";

/// 判定の結果。
enum Decision {
    /// 受付。適用する効果と、選んだ選択肢（`Input::Choice` のとき）を持つ。
    Accept {
        effects: Vec<EffectItem>,
        choice: Option<String>,
    },
    /// 拒否（状態は変えない）。
    Reject,
    /// 宣言の不正（状態は変えず、`Warning` を添える）。
    Invalid,
}

/// `Op::Input` の分岐。新しいイベント列を返し、受付なら `state` を更新する。
pub(crate) fn on_input(ir: &Ir, state: &mut SessionState, input: Input) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    let at = Stamp {
        generation: state.run.generation,
        step: current.clone(),
    };
    let mut events = vec![Event::InputReceived {
        at: at.clone(),
        input: input.clone(),
    }];
    let Some(node) = ir.nodes.iter().find(|n| n.uid == current.0) else {
        events.push(warning(&at));
        return events;
    };
    // 判定結果は `InputReceived` の直後に 1 件だけ置く（TASK-267）。判定できない宣言は、固定文の `Warning` を
    // 添えて状態を変えない（fail-closed）。
    match judge_input(ir, &state.run, &input, &NoConditions) {
        Ok(outcome) => events.push(judged_event(at.clone(), outcome.judgment)),
        Err(_) => {
            events.push(warning(&at));
            return events;
        }
    }
    if !awaits_input(&state.run, &current) || matches!(input, Input::Cancel) {
        return events;
    }
    let step = normalized(&node.step);
    let root = current.root();
    match decide(ir, &step, &input, &state.run, &root) {
        Decision::Reject => {}
        Decision::Invalid => events.push(warning(&at)),
        Decision::Accept { effects, choice } => {
            events.extend(accept(
                &mut state.run,
                &current,
                &step,
                &effects,
                choice,
                &at,
            ));
        }
    }
    events
}

/// 受付の前提。Step が入力を待ち（`Entered`）、処理中でないこと。
///
/// 受付済みの Step への確認の操作（TASK-207 2/3）は、この条件が「`Accepted` かつ処理中でない」のときの
/// 分岐として足す継ぎ目である。
fn awaits_input(run: &RunState, uid: &StepUid) -> bool {
    run.busy.is_none()
        && run
            .slices
            .get(&uid.root())
            .and_then(|s| s.status.get(uid))
            .copied()
            .unwrap_or_default()
            == StepStatus::Entered
}

fn warning(at: &Stamp) -> Event {
    Event::Warning {
        at: at.clone(),
        message: INVALID_DECLARATION.to_string(),
    }
}

/// 規則の `on` に現れうる既知の照合キー。空の値を取り除くのはこれらだけで、未知のキーは空でも残す
/// （余分なキーのある規則が一致しない防御を、正規化で失わないため）。
const KNOWN_ON_KEYS: [&str; 9] = [
    "command", "args", "missing", "option", "ui", "approval", "target", "text", "cancel",
];

/// 空の値（`null`・空の配列・空の表）か。
fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::Array(a) => a.is_empty(),
        Value::Object(o) => o.is_empty(),
        _ => false,
    }
}

/// 規則の `on` から空の値の欄を、規則から `null` の `when` を取り除いた Step の写し。
pub(crate) fn normalized(step: &Step) -> Step {
    let mut s = step.clone();
    for rule in &mut s.rules {
        let Some(obj) = rule.as_object_mut() else {
            continue;
        };
        if obj.get("when").is_some_and(Value::is_null) {
            obj.remove("when");
        }
        if let Some(on) = obj.get_mut("on").and_then(Value::as_object_mut) {
            on.retain(|k, v| !(KNOWN_ON_KEYS.contains(&k.as_str()) && is_blank(v)));
        }
    }
    s
}

/// 規則の `outcome.effects` を全件読む。省略は効果なし。1 件でも読めなければ `None`（一部だけ適用しない）。
fn read_effects(rule: &Value) -> Option<Vec<EffectItem>> {
    let Some(raw) = rule.get("outcome").and_then(|o| o.get("effects")) else {
        return Some(Vec::new());
    };
    raw.as_array()?
        .iter()
        .map(|v| EffectItem::from_value(v).ok())
        .collect()
}

fn decide(ir: &Ir, step: &Step, input: &Input, run: &RunState, root: &Root) -> Decision {
    let kind = step
        .input
        .as_ref()
        .and_then(|i| i.get("kind"))
        .and_then(Value::as_str)
        .and_then(InputKind::from_str_opt);
    match kind {
        Some(InputKind::Command) => decide_command(ir, step, input, run, root),
        Some(InputKind::Choice | InputKind::UiAction | InputKind::Approval) => {
            decide_select(step, input, run, root)
        }
        Some(InputKind::Text) => decide_text(step, input, run, root),
        // `wait` は進行の完了で受付になる（TASK-210）。入力では受け付けない。未知の `kind` も拒否する。
        Some(InputKind::Wait) | None => Decision::Reject,
    }
}

/// コマンド。受付は、判定が `Matched` で、上から最初に一致した規則が受付のときだけである。
/// 規則が無いときは受付にしない。`on.args`・`on.missing` との照合は `command_rule` が持つ。
fn decide_command(ir: &Ir, step: &Step, input: &Input, run: &RunState, root: &Root) -> Decision {
    let Input::Command(raw) = input else {
        return Decision::Reject;
    };
    let Ok(profile) = read_profile(ir.pack.profile.as_ref()) else {
        return Decision::Invalid;
    };
    let Ok(spec) = command_spec(step) else {
        return Decision::Invalid;
    };
    let Ok(lexed) = lex_command(&profile.command_prefix, raw) else {
        return Decision::Reject;
    };
    let CommandVerdict::Matched(matched) = judge_command(&spec, &lexed) else {
        return Decision::Reject;
    };
    // 規則の照合は `command_rule` に集約する（`judge::judge_input` と同じ結果を得るため）。上から最初に一致した
    // 規則の結果に従い、先の拒否・問い返しの規則を飛ばして後の受付の規則に進まない。
    // `when` は評価できないときも一致とみなさない（安全側）。
    let when = |w: &Value| {
        if w.is_null() {
            Some(true)
        } else {
            NoConditions.eval(w, run, root)
        }
    };
    match judge_command_rules(step, &matched, &when) {
        Ok(CommandRuleVerdict::Accepted { rule_index, .. }) => {
            match step.rules.get(rule_index).and_then(read_effects) {
                Some(effects) => Decision::Accept {
                    effects,
                    choice: None,
                },
                None => Decision::Invalid,
            }
        }
        Ok(_) => Decision::Reject,
        Err(_) => Decision::Invalid,
    }
}

/// 選択肢・画面操作・承認。
fn decide_select(step: &Step, input: &Input, run: &RunState, root: &Root) -> Decision {
    match judge_select(step, input, &|w| NoConditions.eval(w, run, root)) {
        Ok(SelectVerdict::Accepted { rule_index, .. }) => {
            let effects = step.rules.get(rule_index).and_then(read_effects);
            match effects {
                Some(effects) => Decision::Accept {
                    effects,
                    choice: match input {
                        Input::Choice(id) => Some(id.clone()),
                        _ => None,
                    },
                },
                None => Decision::Invalid,
            }
        }
        Ok(
            SelectVerdict::RejectedByRule { .. }
            | SelectVerdict::Clarified { .. }
            | SelectVerdict::Rejected(_),
        )
        | Err(SelectJudgeError::NotSelectInput) => Decision::Reject,
        Err(SelectJudgeError::InvalidDeclaration) => Decision::Invalid,
    }
}

/// 自然文。
fn decide_text(step: &Step, input: &Input, run: &RunState, root: &Root) -> Decision {
    let Input::Text(text) = input else {
        return Decision::Reject;
    };
    match judge_text(step, text, run, root, &NoConditions) {
        Ok(TextJudgment::Accepted(a)) => Decision::Accept {
            effects: a.effects,
            choice: None,
        },
        Ok(TextJudgment::Rejected(_)) => Decision::Reject,
        Err(_) => Decision::Invalid,
    }
}

/// 受付の共通の処理。Step を `Accepted` にし、`Accepted` と効果のイベントを返す。
///
/// 自然文も同じ経路で適用する（`text_judge::accept_text` と同じ結果になる）。`processing` を持つ Step は
/// 処理中（`busy`）にする。`total` が `u32` に収まらなければ最大値に倒す。
fn accept(
    run: &mut RunState,
    uid: &StepUid,
    step: &Step,
    effects: &[EffectItem],
    choice: Option<String>,
    at: &Stamp,
) -> Vec<Event> {
    let root = uid.root();
    let slice = run.slices.entry(root.clone()).or_default();
    slice.status.insert(uid.clone(), StepStatus::Accepted);
    if let Some(id) = choice {
        slice.choices.insert(uid.clone(), id);
    }
    let mut events = vec![Event::Accepted { at: at.clone() }];
    events.extend(apply_effects(
        run,
        &root,
        effects,
        at,
        Cause::Input,
        &NoConditions,
    ));
    if !step.processing.is_empty() {
        run.busy = Some(Busy {
            done: 0,
            total: u32::try_from(step.processing.len()).unwrap_or(u32::MAX),
        });
    }
    events
}
