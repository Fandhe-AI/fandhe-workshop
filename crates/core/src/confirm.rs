//! 確認の操作による達成と、外れた指し先の扱い（TASK-207 2/3、EVT-3 の (b) と EVT-4）。
//!
//! 受付済み（`StepStatus::Accepted`）で処理中でない Step への入力を、Step の確認（`confirm`）の宣言と照らす。
//! 確認が成り立てば Step を達成（`StepStatus::Achieved`）にし、達成のときの効果を適用し、最小の遷移（`next` の
//! 先頭の無条件の辺）で次の Step に入る。成り立たなければ受付済みのまま何も変えない。
//! 呼び出し元は [`crate::accept::on_input`] だけで、公開 API には含めない（契約の対象外）。
//! 受付は `accept` が、確認の種類ごとの宣言の型は contract の `confirm_spec` が持つ。本モジュールは両者をつなぐ。
//!
//! # 規則
//!
//! 1. `confirm` が無ければ `approve` として扱う（`achievement(None)` と同じ安全側）
//! 2. `point_at`: `Input::Ui` が `targets` のどれかと完全一致したときだけ成立する
//! 3. `answer`: `Input::Choice` が `correct` の選択肢を指したときだけ成立する（誤答・未知の ID は外れ）
//! 4. `approve`: `Input::Approval` の決定が承認のときだけ成立する（`target` つきの承認は外れ。対象つきの「直す」等の経路は別）
//! 5. `none`: 成立させない（受付と同時の達成は TASK-208）
//! 6. 宣言が読めない・検査に通らない・`kind` が未知なら、固定文の `Warning` を足して状態を変えない（fail-closed）
//! 7. 外れは状態を変えず、イベントも足さない（版 1 は `Judged` が無い。文面の発行は TASK-267 の後）
//!
//! 成立したときの順は `Achieved` → 達成の効果のイベント → `Transitioned` → `Entered`。達成の効果が 1 件でも
//! 読めなければ、達成にせず `Warning` だけを返す（一部だけ適用しない）。遷移は `when` の無い（`null` を含む）
//! 最初の辺だけを採る。`when` は評価できないので採らない。採れる辺が無い・遷移先が IR に無い場合は遷移しない。
//!
//! # 扱わないこと
//!
//! - 章の完了（`ChapterCompleted`・`completed`）、`exit_to`・`use` への遷移（TASK-233 以降）
//! - 遷移先の `on_enter` の効果、入口の写し `SessionState.entry` の更新（TASK-211・213・217）
//! - `on_miss`・`on_wrong` の文面の発行と `Judged`（TASK-267）、`busy` の分類（TASK-209）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。遷移は 1 操作で
//! 高々 1 回。`Warning` の文面は固定文で、入力値・教材の値を埋め込まない。

use fandhe_workshop_contract::confirm_spec::{
    answer_spec_of, ApproveSpec, ConfirmKind, PointAtSpec,
};
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::{Ir, Node};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::addr::{Stamp, StepUid};
use crate::effect::{apply_effects, NoConditions};
use crate::event::{Cause, Event};
use crate::input::{Decision, Input};
use crate::session::SessionState;
use crate::step_status::StepStatus;

/// 宣言が読めなかったときの固定文。入力値・教材の値を含めない。
const INVALID_DECLARATION: &str =
    "confirm declaration or achievement effects could not be read; input ignored";

/// 遷移先が IR に無いときの固定文。
const UNKNOWN_TARGET: &str = "transition target is not in the course; transition skipped";

/// 確認の判定。
enum Verdict {
    /// 確認が成立した。
    Confirmed,
    /// 成立しない（受付済みのまま）。
    Missed,
    /// 宣言の不正。
    Invalid,
}

/// 受付済みの Step への入力の分岐。`InputReceived` の後に続くイベントだけを返し、成立なら `state` を更新する。
pub(crate) fn on_confirm(
    ir: &Ir,
    state: &mut SessionState,
    node: &Node,
    at: &Stamp,
    input: &Input,
) -> Vec<Event> {
    match judge(node.step.confirm.as_ref(), input) {
        Verdict::Missed => Vec::new(),
        Verdict::Invalid => vec![warning(at, INVALID_DECLARATION)],
        Verdict::Confirmed => achieve(ir, state, node, at),
    }
}

/// `kind` を除いた宣言の写しを型で読む。
fn read_spec<T: DeserializeOwned>(decl: &Value) -> Option<T> {
    let mut map = decl.as_object()?.clone();
    map.remove("kind");
    serde_json::from_value(Value::Object(map)).ok()
}

fn judge(confirm: Option<&Value>, input: &Input) -> Verdict {
    let kind = match confirm {
        None => ConfirmKind::Approve,
        Some(decl) => match decl
            .get("kind")
            .and_then(Value::as_str)
            .and_then(ConfirmKind::from_str_opt)
        {
            Some(k) => k,
            None => return Verdict::Invalid,
        },
    };
    let empty = Value::Object(serde_json::Map::new());
    let decl = confirm.unwrap_or(&empty);
    match kind {
        ConfirmKind::PointAt => {
            let Some(spec) = read_spec::<PointAtSpec>(decl).filter(|s| s.check().is_ok()) else {
                return Verdict::Invalid;
            };
            match input {
                Input::Ui { view, path }
                    if spec
                        .targets
                        .iter()
                        .any(|t| t.view == *view && t.path == *path) =>
                {
                    Verdict::Confirmed
                }
                _ => Verdict::Missed,
            }
        }
        ConfirmKind::Answer => {
            let Some(spec) = answer_spec_of(decl).filter(|s| s.check().is_ok()) else {
                return Verdict::Invalid;
            };
            match input {
                Input::Choice(id) if spec.option(id).is_some_and(|o| o.correct) => {
                    Verdict::Confirmed
                }
                _ => Verdict::Missed,
            }
        }
        ConfirmKind::Approve => {
            if read_spec::<ApproveSpec>(decl).is_none() {
                return Verdict::Invalid;
            }
            match input {
                Input::Approval {
                    decision: Decision::Approve,
                    target: None,
                } => Verdict::Confirmed,
                _ => Verdict::Missed,
            }
        }
        ConfirmKind::None => Verdict::Missed,
    }
}

fn warning(at: &Stamp, message: &str) -> Event {
    Event::Warning {
        at: at.clone(),
        message: message.to_string(),
    }
}

/// 達成の処理。効果が読めなければ達成にせず `Warning` だけを返す。
fn achieve(ir: &Ir, state: &mut SessionState, node: &Node, at: &Stamp) -> Vec<Event> {
    let Some(items) = node
        .step
        .on_achieved
        .iter()
        .map(|v| EffectItem::from_value(v).ok())
        .collect::<Option<Vec<_>>>()
    else {
        return vec![warning(at, INVALID_DECLARATION)];
    };
    let uid = at.step.clone();
    let root = uid.root();
    state
        .run
        .slices
        .entry(root.clone())
        .or_default()
        .status
        .insert(uid, StepStatus::Achieved);
    let mut events = vec![Event::Achieved {
        at: at.clone(),
        explicit_skip: false,
    }];
    events.extend(apply_effects(
        &mut state.run,
        &root,
        &items,
        at,
        Cause::Input,
        &NoConditions,
    ));
    // 無条件の最初の辺だけを採る。`when` は評価できないので、条件つきの辺は採らない。
    let Some(edge) = node
        .step
        .next
        .iter()
        .find(|t| t.when.as_ref().is_none_or(Value::is_null))
    else {
        return events;
    };
    let Some(target) = ir.nodes.iter().find(|n| n.uid == edge.to) else {
        events.push(warning(at, UNKNOWN_TARGET));
        return events;
    };
    let to = StepUid(target.uid.clone());
    events.push(Event::Transitioned {
        at: at.clone(),
        to: to.clone(),
    });
    state.run.current = Some(to.clone());
    state
        .run
        .slices
        .entry(to.root())
        .or_default()
        .status
        .insert(to.clone(), StepStatus::Entered);
    events.push(Event::Entered {
        at: Stamp {
            generation: at.generation,
            step: to,
        },
    });
    events
}
