//! 確認の操作による達成と、外れた指し先の扱い（TASK-207 2/3、EVT-3 の (b) と EVT-4）。
//!
//! 受付済み（`StepStatus::Accepted`）で処理中でない Step への入力は、Step の確認（`confirm`）の宣言と照らして
//! 判定する。その判定は [`crate::judge::judge_input`]（`select_judge::judge_confirm`）が 1 回だけ行い、本モジュールは
//! 確認と決まった後の達成の処理だけを持つ。宣言は読み直さない（判定と遷移の食い違いを作らない。JUDGE-3）。
//! 確認と決まれば Step を達成（`StepStatus::Achieved`）にし、達成のときの効果を適用し、最小の遷移（`next` の
//! 先頭の無条件の辺）で次の Step に入る。確認でなければ `accept` が受付済みのまま何も変えない。
//! 呼び出し元は [`crate::accept::on_input`] だけで、公開 API には含めない（契約の対象外）。
//! 確認の種類ごとの成立条件と宣言の検査は `select_judge` が、宣言の型は contract の `confirm_spec` が持つ。
//!
//! # 規則
//!
//! 1. 確認の種類ごとの成立条件（`confirm` 無しは `approve` 扱い、`point_at`・`answer`・`approve`、`none` は成立させない）は
//!    `select_judge::judge_confirm` が決める。理由つきの省略は受付と同時に達成になる（[`crate::accept`]、TASK-208）
//! 2. 宣言が読めない・検査に通らない・`kind` が未知なら、判定が誤りを返し、`accept` が固定文の `Warning` を足して
//!    状態を変えない（fail-closed）
//! 3. 外れは状態を変えず、イベントも足さない（判定結果の `Judged` は `accept` が置く。TASK-267）
//!
//! 成立したときの順は `Achieved` → 達成の効果のイベント → `Transitioned` → `Entered`。達成の効果が 1 件でも
//! 読めなければ、達成にせず `Warning` だけを返す（一部だけ適用しない）。遷移は `when` の無い（`null` を含む）
//! 最初の辺だけを採る。`when` は評価できないので採らない。採れる辺が無い・遷移先が IR に無い場合は遷移しない。
//!
//! # 扱わないこと
//!
//! - 章の完了（`ChapterCompleted`・`completed`）、`exit_to`・`use` への遷移（TASK-233 以降）
//! - 遷移先の `on_enter` の効果、入口の写し `SessionState.entry` の更新（TASK-242・213・217）
//! - `on_miss`・`on_wrong` の文面の解決と `Judged` の発行（`crate::judge`、TASK-267）、`busy` の分類（[`crate::busy_class`]、TASK-209 1/2）。
//!   どちらも受付の分岐が先に行い、本モジュールは判定が確認のときだけ呼ばれる
//! - `processing` を持つ Step の省略の達成（進行の完了で [`crate::tick`] が達成にする。TASK-210 2/3）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。遷移は 1 操作で
//! 高々 1 回。`Warning` の文面は固定文で、入力値・教材の値を埋め込まない。

use fandhe_workshop_contract::confirm_spec::{achievement, Achievement, ConfirmKind, NoneSpec};
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::{Ir, Node, Step};
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::addr::{Stamp, StepUid};
use crate::effect::{apply_effects, NoConditions};
use crate::event::{Cause, Event};
use crate::session::SessionState;
use crate::step_status::StepStatus;

/// 宣言が読めなかったときの固定文。入力値・教材の値を含めない。
const INVALID_DECLARATION: &str =
    "confirm declaration or achievement effects could not be read; input ignored";

/// 遷移先が IR に無いときの固定文。
const UNKNOWN_TARGET: &str = "transition target is not in the course; transition skipped";

/// 受付の時点での達成の扱い（EVT-5）。
pub(crate) enum OnAccept {
    /// 確認の操作を待つ（受付済みで止まる）。
    AwaitConfirm,
    /// 受付と同時に達成にする。読めた `on_achieved` の効果を持つ。
    Achieve(Vec<EffectItem>),
    /// 省略の宣言が不正（理由が無い・空白だけ・読めない欄・読めない `on_achieved`）。受け付けない。
    Invalid,
}

/// Step の `confirm` から、受付と同時に達成にするかを決める。
///
/// 呼び出し元は [`crate::accept::on_input`] だけ。教材が理由を添えて確認を省くと明示した Step（`none`）だけが
/// `Achieve` になる。`kind` が未知・読めないものは `AwaitConfirm` に倒し、受付後の判定（`judge_confirm` の宣言の誤り）で
/// accept が `Warning` を出す。理由は読むだけで、返さず、文面にも出さない。
pub(crate) fn on_accept_mode(step: &Step) -> OnAccept {
    let kind = match step.confirm.as_ref() {
        None => None,
        Some(decl) => match decl
            .get("kind")
            .and_then(Value::as_str)
            .and_then(ConfirmKind::from_str_opt)
        {
            Some(k) => Some(k),
            None => return OnAccept::AwaitConfirm,
        },
    };
    if achievement(kind) != Achievement::OnAccept {
        return OnAccept::AwaitConfirm;
    }
    let reason_ok = step
        .confirm
        .as_ref()
        .and_then(read_spec::<NoneSpec>)
        .is_some_and(|s| s.has_reason());
    if !reason_ok {
        return OnAccept::Invalid;
    }
    match read_achieved_effects(step) {
        Some(items) => OnAccept::Achieve(items),
        None => OnAccept::Invalid,
    }
}

/// 判定（[`crate::judge::judge_input`] → `judge_confirm`）が確認と決めた後の達成の処理。
///
/// 確認の宣言は判定が 1 回だけ評価済みで、ここでは読み直さない。呼び出し元は [`crate::accept::on_input`] だけで、
/// 判定が確認のときにしか呼ばない。`on_achieved` が読めなければ達成にせず、固定文の `Warning` だけを返す。
pub(crate) fn on_confirmed(
    ir: &Ir,
    state: &mut SessionState,
    node: &Node,
    at: &Stamp,
) -> Vec<Event> {
    let Some(items) = read_achieved_effects(&node.step) else {
        return vec![warning(at, INVALID_DECLARATION)];
    };
    achieve_with(ir, state, node, at, &items)
}

/// `kind` を除いた宣言の写しを型で読む。
fn read_spec<T: DeserializeOwned>(decl: &Value) -> Option<T> {
    let mut map = decl.as_object()?.clone();
    map.remove("kind");
    serde_json::from_value(Value::Object(map)).ok()
}

fn warning(at: &Stamp, message: &str) -> Event {
    Event::Warning {
        at: at.clone(),
        message: message.to_string(),
    }
}

/// `on_achieved` を全件読む。1 件でも読めなければ `None`（一部だけ適用しない）。
fn read_achieved_effects(step: &Step) -> Option<Vec<EffectItem>> {
    step.on_achieved
        .iter()
        .map(|v| EffectItem::from_value(v).ok())
        .collect()
}

/// 読めた効果で達成にし、効果を適用し、最小の遷移を行う。確認の経路と受付と同時の達成の経路が共有する。
pub(crate) fn achieve_with(
    ir: &Ir,
    state: &mut SessionState,
    node: &Node,
    at: &Stamp,
    items: &[EffectItem],
) -> Vec<Event> {
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
        items,
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
