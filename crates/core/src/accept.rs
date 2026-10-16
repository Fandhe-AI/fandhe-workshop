//! 入力の受付の分岐（TASK-207 1/3、EVT-3 の (a)）。
//!
//! 入力が現在の Step の入力条件に合い、受付の規則に当たったら、Step を受付済み（`StepStatus::Accepted`）にして
//! 規則の効果を世界状態へ適用する。受付は達成ではない。達成は確認の操作の後で、受付済みの Step への入力は
//! [`crate::confirm`]（TASK-207 2/3）が持つ。例外は、教材が理由を添えて確認を省くと明示した Step
//! （`confirm: none`、EVT-5、TASK-208）で、受付と同時に [`crate::confirm`] の達成の処理を呼ぶ。
//! `processing` を持つ Step は除く（進行の完了で達成にする。TASK-210）。
//! 本モジュール自身は `StepStatus::Achieved` を書かず、`ChapterCompleted` を出さない。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 入力の判定は [`crate::judge::judge_input`] が 1 回だけ行い、本モジュールはその結果（`Judged` に載せる判定と
//! 付帯情報）に従って状態を変える。規則を判定し直さないので、`Judged` と実際の遷移は食い違わない（JUDGE-3）。
//! 効果の適用は [`crate::effect`] が持つ。本モジュールは判定の規則を再実装しない。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無ければ、状態を変えず空のイベント列を返す（`Stamp` が作れない。TASK-217 が決める）
//! 2. 以降は必ず先頭に `InputReceived`（入力は原文のまま）を出す
//! 3. 現在の Step が IR に無ければ、固定文の `Warning` を添えて状態を変えない
//! 4. 判定できない宣言（`judge_input` の誤り）は、`Judged` を置かず固定文の `Warning` を添えて状態を変えない
//! 5. `Input::Cancel` は状態を変えない（意味づけは TASK-212）。拒否・問い返しは `Judged` だけを出す。
//!    `on.cancel` の受付の規則に当たったときは、受付の処理が未結線なので `Judged` を置かず固定文の `Warning` だけを出す
//! 6. 処理中（`busy`。[`crate::busy_class::classify_busy`] が決める）なら、状態に依らず `Judged`（拒否 `busy`）だけを
//!    出す（処理中の再送。受付は（世代, Step）につき 1 回で、2 回目以降は処理中の拒否になる。EVT-7）。
//!    Step が `Entered`・`Accepted` でない（達成済みなど）ときも `Judged` だけを出す
//! 7. Step が `Accepted` で処理中でなければ、判定が確認のときだけ確認の処理（[`crate::confirm`]）に渡す。
//!    それ以外の判定は `Judged` だけを出す。確認でも達成にならなければ（宣言・達成の効果が読めない）、
//!    `Judged` を置かず `confirm` の `Warning` だけを出す
//! 8. Step が `Entered` なら、判定が受付のときだけ受け付ける。拒否・問い返しは `Judged` だけで状態を変えない。
//!    受付の効果は、判定が指した規則（[`JudgeDetail::Rule`] の添字）の `outcome.effects`、または自然文の判定が
//!    返した効果（[`JudgeDetail::Text`]）で、規則を読み直さない。効果が読めない・付帯情報が受付の形でない・
//!    省略の宣言が不正なら、`Judged` を置かず固定文の `Warning` を足して状態を変えない（fail-closed）
//!
//! 受付のイベントの順は `InputReceived` → `Judged` → `Accepted` → 効果のイベント → `Changed`。省略を明示した
//! Step では `Accepted` と効果のイベントの後に `Achieved` → 達成の効果 → `Transitioned` → `Entered` が続く。
//! `Changed` は [`crate::session::apply`] が経路ごと 1 件へ集約して末尾に足す。
//!
//! # IR の形の正規化
//!
//! コンパイル済みの IR は、規則の `on` に使わない照合キーを `null`・空の配列・空の表で持ち、`when` も
//! `null` で持つことがある。判定の部品は「余分なキーのある規則は一致しない」ため、規則を読む前に
//! 既知の照合キー（`KNOWN_ON_KEYS`）の空の値だけを取り除いた写しを作る（`normalized`）。未知のキーは
//! 値が空でも残るので、余分なキーの規則が一致しない性質は保たれる。`judge_input` も同じ写しで判定するため、
//! 判定が返す規則の添字は、この写しの `rules` の添字と一致する。
//!
//! # 扱わないこと
//!
//! - 確認の操作による達成と外れた指し先（`confirm`、TASK-207 2/3）、確認前の達成 0 件の性質テスト（TASK-207 3/3）
//! - `busy` の文面の差し込み（TASK-274）、残り続けるボタンへの連打（TASK-209 2/2）、`processing` を持つ省略の Step の達成（TASK-210）
//! - 進行 `Op::Tick` の照合と進め方（TASK-210）、処理中でない Esc の意味づけ（TASK-212）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。
//! `Warning` の文面は固定文で、入力値・教材の値を埋め込まない。入力値は未信頼で、HTML への埋め込みの
//! エスケープは web 層の責務である。

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::Judgment;
use serde_json::Value;

use crate::addr::{Stamp, StepUid};
use crate::busy_class::classify_busy;
use crate::confirm;
use crate::effect::{apply_effects, NoConditions};
use crate::event::{Cause, Event};
use crate::input::Input;
use crate::judge::{judge_input, judged_event, JudgeDetail};
use crate::session::{Busy, RunState, SessionState};
use crate::step_status::StepStatus;

/// 宣言が読めなかったときの固定文。入力値・教材の値を含めない。
const INVALID_DECLARATION: &str =
    "input declaration or rule effects could not be read; input rejected";

/// 取り消しの受付の規則に当たったときの固定文（受付の処理は TASK-212 まで結線しない）。規則の ID・入力値を含めない。
const UNSUPPORTED_CANCEL: &str = "cancel acceptance is not supported yet; input ignored";

/// 確認を省く宣言が不正なときの固定文。理由・入力値・教材の値を含めない。
const INVALID_SKIP: &str = "confirm none declaration could not be read; input rejected";

/// `Op::Input` の分岐。新しいイベント列を返し、受付・確認なら `state` を更新する。
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
    // 判定は 1 回だけ行い、以降の分岐はこの結果に従う（TASK-267）。判定できない宣言は、固定文の `Warning` を
    // 添えて状態を変えない（fail-closed）。
    let outcome = match judge_input(ir, &state.run, &input, &NoConditions) {
        Ok(outcome) => outcome,
        Err(_) => {
            events.push(warning(&at));
            return events;
        }
    };
    let judged = judged_event(at.clone(), outcome.judgment.clone());
    if matches!(input, Input::Cancel) {
        // 取り消しの受付の意味づけ（`on.cancel` の受付の規則の遷移と効果）は TASK-212 が持ち、まだ結線しない。
        // それまでは受付の `Judged` を出さず、固定文の `Warning` だけにして状態を変えない（受付と判定したのに
        // 遷移が無い食い違いを作らない。fail-closed）。拒否・問い返しは `Judged` だけで、状態を変えない。
        if matches!(outcome.judgment, Judgment::Accepted(_)) {
            events.push(Event::Warning {
                at: at.clone(),
                message: UNSUPPORTED_CANCEL.to_string(),
            });
        } else {
            events.push(judged);
        }
        return events;
    }
    match phase(&state.run, &current, &input) {
        Phase::Busy | Phase::Idle => {
            events.push(judged);
            return events;
        }
        Phase::AwaitConfirm => {
            // 確認と判定したときだけ達成の処理へ進む。拒否の `Judged` の後に達成を出さない。
            if outcome.judgment != Judgment::Confirmed {
                events.push(judged);
                return events;
            }
            // 達成にならなかった（宣言・達成の効果が読めない）ときは、受付と同じく `Judged` を置かず
            // `confirm` の固定文の `Warning` だけにする（確認の `Judged` の後に達成が無い食い違いを作らない）。
            let confirmed = confirm::on_confirm(ir, state, node, &at, &input);
            if confirmed
                .iter()
                .any(|e| matches!(e, Event::Achieved { .. }))
            {
                events.push(judged);
            }
            events.extend(confirmed);
            return events;
        }
        Phase::AwaitInput => {}
    }
    // 拒否・問い返しは `Judged` だけを出し、状態を変えない。後ろの受付の規則へ進まない。
    if !matches!(outcome.judgment, Judgment::Accepted(_)) {
        events.push(judged);
        return events;
    }
    let step = normalized(&node.step);
    let Some((effects, choice)) = accepted_effects(&step, &input, outcome.detail) else {
        events.push(warning(&at));
        return events;
    };
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
    events.push(judged);
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
    events
}

/// 入力を受けたときの Step の段階。
enum Phase {
    /// 入力を待つ（`Entered` で処理中でない）。受付の判定に進む。
    AwaitInput,
    /// 受付済みで処理中でない。確認の判定に進む（TASK-207 2/3）。
    AwaitConfirm,
    /// 処理中（`busy`）。`InputReceived` と `Judged`（拒否 `busy`）だけを出し、状態を変えない（EVT-7）。
    Busy,
    /// 上のいずれでもない（達成済みなど）。`InputReceived` と `Judged` だけを出す。
    Idle,
}

fn phase(run: &RunState, uid: &StepUid, input: &Input) -> Phase {
    // 処理中の判定の正は `busy_class` の 1 か所にし、受付・確認の分類より先に評価する（JUDGE-5）。
    if classify_busy(run, input).is_some() {
        return Phase::Busy;
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

/// 受付の判定の付帯情報から、適用する効果と選んだ選択肢（`Input::Choice` のとき）を取り出す。
///
/// 規則は判定し直さず、判定が指した規則（添字）の `outcome.effects` だけを読む。自然文は判定が読んだ効果を
/// そのまま使う。付帯情報が受付の形でない・効果が 1 件でも読めなければ `None`（受付に倒さない）。
fn accepted_effects(
    step: &Step,
    input: &Input,
    detail: JudgeDetail,
) -> Option<(Vec<EffectItem>, Option<String>)> {
    let effects = match detail {
        JudgeDetail::Rule { rule_index, .. } => {
            step.rules.get(rule_index).and_then(read_effects)?
        }
        JudgeDetail::Text(accepted) => accepted.effects,
        JudgeDetail::None | JudgeDetail::Ignored(_) => return None,
    };
    let choice = match input {
        Input::Choice(id) => Some(id.clone()),
        _ => None,
    };
    Some((effects, choice))
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
