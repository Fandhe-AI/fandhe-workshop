//! 実行環境の操作（保存）の成功を Step の入力として受け付ける（TASK-64 1/2、PACK-34、D-69）。
//!
//! 教材は Step の `input` に `{ kind: env_action, action: save }` を宣言できる。この宣言を持つ Step は、受講者の
//! 打鍵ではなく、チェックポイントの保存の成功（`CheckpointSaved`）で受付になる。呼び出し元は
//! [`crate::session::apply`] が使う `step` の `Control::Save` の腕だけで、[`crate::checkpoint::on_save`] が保存に
//! 成功した後に呼ぶ。保存そのものの分岐（`checkpoint`）は教材（IR）を読まない不変条件を持つため、教材を読む結線は
//! ここに分ける。公開 API には含めない（契約の対象外。世代の規則の表・`Op`・イベントの形は変えない）。
//!
//! # 規則
//!
//! 1. 現在の Step が無い、IR に Step が無い、宣言が `env_action` + `save` と型で読めない、Step が `Entered` でない、
//!    処理中である、のどれかなら何もしない（従来どおり `CheckpointSaved` だけになる）。`export` の Step は `Save` では
//!    受け付けない（偽の達成を作らない）
//! 2. 受付と達成は別である（EVT-3・EVT-5）。確認の宣言が `none`（理由つき）のときは受付と同時に
//!    [`crate::confirm::achieve_with`] で達成・効果・遷移へ進み、それ以外は `Accepted` で止まって確認の操作を待つ
//! 3. 省略の宣言が不正（理由が無い・`on_achieved` が読めない）なら、固定文の `Warning` だけを出して状態を変えない
//!
//! イベントの順は `Accepted` → （`none` なら）`Achieved` → 達成の効果 → `Transitioned` → `Entered`。
//! `InputReceived`・`Judged` は伴わない（受講者の入力ではないため）。規則（`rules`）は照合しないので受付の効果は無く、
//! 効果は `on_achieved` に書く。世代は変えない。
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。`Warning` の文面は
//! 固定文で、教材の値を埋め込まない。

use fandhe_workshop_contract::input_spec::{EnvAction, EnvActionSpec};
use fandhe_workshop_contract::ir::Ir;
use serde_json::Value;

use crate::addr::Stamp;
use crate::confirm::{self, OnAccept};
use crate::event::Event;
use crate::session::SessionState;
use crate::step_status::StepStatus;

/// 省略の宣言が読めないときの固定文。教材の値を含めない。
const INVALID_SKIP: &str = "confirm none declaration could not be read; save ignored";

/// 保存が成功した直後に呼ぶ。現在の Step が保存を入力とする宣言なら受付にし、出たイベント列を返す。
pub(crate) fn on_saved(ir: &Ir, state: &mut SessionState) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    let Some(node) = ir.nodes.iter().find(|n| n.uid == current.0) else {
        return Vec::new();
    };
    if declared_action(node.step.input.as_ref()) != Some(EnvAction::Save) {
        return Vec::new();
    }
    let status = state
        .run
        .slices
        .get(&current.root())
        .and_then(|s| s.status.get(&current))
        .copied()
        .unwrap_or_default();
    if status != StepStatus::Entered || state.run.busy.is_some() {
        return Vec::new();
    }
    let at = Stamp {
        generation: state.run.generation,
        step: current.clone(),
    };
    // 状態を書く前に達成の扱いを決める。不正な宣言では何も変えない。
    let items = match confirm::on_accept_mode(&node.step) {
        OnAccept::Invalid => {
            return vec![Event::Warning {
                at,
                message: INVALID_SKIP.to_string(),
            }];
        }
        OnAccept::AwaitConfirm => None,
        OnAccept::Achieve(items) => Some(items),
    };
    state
        .run
        .slices
        .entry(current.root())
        .or_default()
        .status
        .insert(current, StepStatus::Accepted);
    let mut events = vec![Event::Accepted { at: at.clone() }];
    if let Some(items) = items {
        events.extend(confirm::achieve_with(ir, state, node, &at, &items));
    }
    events
}

/// Step の `input` が `env_action` と型で読めるなら、その操作の種類。読めなければ `None`。
fn declared_action(input: Option<&Value>) -> Option<EnvAction> {
    let obj = input?.as_object()?;
    if obj.get("kind").and_then(Value::as_str) != Some("env_action") {
        return None;
    }
    let mut rest = obj.clone();
    rest.remove("kind");
    serde_json::from_value::<EnvActionSpec>(Value::Object(rest))
        .ok()
        .map(|s| s.action)
}
