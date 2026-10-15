//! 入力の受付の分岐（TASK-207 1/3、EVT-3 の (a)）。
//!
//! 入力が現在の Step の入力条件に合い、受付の規則に当たったら、Step を受付済み（`StepStatus::Accepted`）にして
//! 規則の効果を世界状態へ適用する。受付は達成ではない。達成は確認の操作の後で、受付済みの Step への入力は
//! [`crate::confirm`]（TASK-207 2/3）が持つ。例外は、教材が理由を添えて確認を省くと明示した Step
//! （`confirm: none`、EVT-5、TASK-208）で、受付と同時に [`crate::confirm`] の達成の処理を呼ぶ。
//! `processing` を持つ Step は除く（進行の完了で達成にする。TASK-210）。
//! 本モジュール自身は `StepStatus::Achieved` を書かず、`ChapterCompleted` を出さない。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 判定の部品は [`crate::command_lex`]・[`crate::command_judge`]・[`crate::select_judge`]・[`crate::text_judge`]、
//! 効果の適用は [`crate::effect`] が持つ。本モジュールはそれらをつなぐだけで、判定の規則は再実装しない。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無ければ、状態を変えず空のイベント列を返す（`Stamp` が作れない。TASK-217 が決める）
//! 2. 以降は必ず先頭に `InputReceived`（入力は原文のまま）を出す
//! 3. 現在の Step が IR に無ければ、固定文の `Warning` を添えて状態を変えない
//! 4. `Input::Cancel` は `InputReceived` だけ（意味づけは TASK-212）
//! 5. Step が `Accepted` で処理中でなければ、確認の判定（[`crate::confirm`]）に渡す
//! 6. Step が `Entered` でない、または処理中（`busy`）なら `InputReceived` だけを出す
//!    （処理中の再送と、達成済みなどへの入力。`busy` の分類は TASK-209 が担う）
//! 7. 入力方式で振り分ける。受付は「受付の規則に一致したとき」だけで、宣言の不正・未知の `kind`・
//!    読めない効果は受付に倒さず、拒否（宣言の不正には固定文の `Warning` を添える）にする（fail-closed）
//!
//! 受付のイベントの順は `InputReceived` → `Accepted` → 効果のイベント → `Changed`。省略を明示した Step では
//! `Accepted` と効果のイベントの後に `Achieved` → 達成の効果 → `Transitioned` → `Entered` が続く。
//! 省略の宣言が不正（理由が無い等）なら受け付けず、固定文の `Warning` を足して状態を変えない（fail-closed）。
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
//! - 確認の操作による達成と外れた指し先（`confirm`、TASK-207 2/3）、確認前の達成 0 件の性質テスト（TASK-207 3/3）
//! - 処理中の再送の `busy` 分類（TASK-209）、`processing` を持つ省略の Step の達成（TASK-210）
//! - 進行 `Op::Tick` の照合と進め方（TASK-210）、処理中でない Esc の意味づけ（TASK-212）
//! - `Judged` の発行とコマンドの規則の `on.args`・`on.missing` との照合（TASK-267）
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
use crate::confirm;
use crate::effect::{apply_effects, ConditionEval, NoConditions};
use crate::event::{Cause, Event};
use crate::input::Input;
use crate::select_judge::{judge_select, SelectJudgeError, SelectVerdict};
use crate::session::{Busy, RunState, SessionState};
use crate::step_status::StepStatus;
use crate::text_judge::{judge_text, TextJudgment};

/// 宣言が読めなかったときの固定文。入力値・教材の値を含めない。
const INVALID_DECLARATION: &str =
    "input declaration or rule effects could not be read; input rejected";

/// 確認を省く宣言が不正なときの固定文。理由・入力値・教材の値を含めない。
const INVALID_SKIP: &str = "confirm none declaration could not be read; input rejected";

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
    if matches!(input, Input::Cancel) {
        return events;
    }
    match phase(&state.run, &current) {
        Phase::Idle => return events,
        Phase::AwaitConfirm => {
            events.extend(confirm::on_confirm(ir, state, node, &at, &input));
            return events;
        }
        Phase::AwaitInput => {}
    }
    let step = normalized(&node.step);
    let root = current.root();
    match decide(ir, &step, &input, &state.run, &root) {
        Decision::Reject => {}
        Decision::Invalid => events.push(warning(&at)),
        Decision::Accept { effects, choice } => {
            // 確認を省くと明示した Step だけが、受付と同時に達成になる（EVT-5、TASK-208）。
            let items = match confirm::on_accept_mode(&node.step) {
                confirm::OnAccept::Invalid => {
                    events.push(Event::Warning {
                        at: at.clone(),
                        message: INVALID_SKIP.to_string(),
                    });
                    return events;
                }
                confirm::OnAccept::AwaitConfirm => None,
                confirm::OnAccept::Achieve(items) => Some(items),
            };
            events.extend(accept(
                &mut state.run,
                &current,
                &step,
                &effects,
                choice,
                &at,
            ));
            // 処理中の Step は、進行の完了で達成にする（TASK-210）。`busy` と `current` を食い違わせない。
            if let Some(items) = items.filter(|_| step.processing.is_empty()) {
                events.extend(confirm::achieve_with(ir, state, node, &at, &items));
            }
        }
    }
    events
}

/// 入力を受けたときの Step の段階。
enum Phase {
    /// 入力を待つ（`Entered` で処理中でない）。受付の判定に進む。
    AwaitInput,
    /// 受付済みで処理中でない。確認の判定に進む（TASK-207 2/3）。
    AwaitConfirm,
    /// 上のどちらでもない（処理中・達成済みなど）。`InputReceived` だけを出す。
    Idle,
}

fn phase(run: &RunState, uid: &StepUid) -> Phase {
    if run.busy.is_some() {
        return Phase::Idle;
    }
    let status = run
        .slices
        .get(&uid.root())
        .and_then(|s| s.status.get(uid))
        .copied()
        .unwrap_or_default();
    match status {
        StepStatus::Entered => Phase::AwaitInput,
        StepStatus::Accepted => Phase::AwaitConfirm,
        _ => Phase::Idle,
    }
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
fn normalized(step: &Step) -> Step {
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

/// コマンド。受付は、判定が `Matched` で、`on` がちょうど `{ command: <名前> }` の受付の規則があるときだけである。
/// 規則が無いときは受付にしない（`on.args`・`on.missing` との照合と見直しは TASK-267）。
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
    if !matches!(judge_command(&spec, &lexed), CommandVerdict::Matched(_)) {
        return Decision::Reject;
    }
    // 上から最初に `on` と `when` が一致した規則の outcome を採用する（`select_judge` と同じ契約）。
    // 先の拒否・問い返しの規則を飛ばして、後の受付の規則に進まない。
    let hit = step.rules.iter().find(|r| {
        let on_ok = r.get("on").and_then(Value::as_object).is_some_and(|on| {
            on.len() == 1 && on.get("command").and_then(Value::as_str) == Some(lexed.name.as_str())
        });
        // `when` は評価できないときも一致とみなさない（安全側）。
        let when_ok = match r.get("when") {
            None => true,
            Some(w) => NoConditions.eval(w, run, root) == Some(true),
        };
        on_ok && when_ok
    });
    let hit = hit.filter(|r| {
        r.get("outcome")
            .and_then(|o| o.get("kind"))
            .and_then(Value::as_str)
            == Some("accept")
    });
    match hit {
        None => Decision::Reject,
        Some(rule) => match read_effects(rule) {
            Some(effects) => Decision::Accept {
                effects,
                choice: None,
            },
            None => Decision::Invalid,
        },
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
