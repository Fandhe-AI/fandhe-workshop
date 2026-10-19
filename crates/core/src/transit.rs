//! 1 回の操作の中で起きる内部の遷移をたどる歩行器と、その上限（TASK-160 1/3、SAFE-23）。
//!
//! 作成者向けの静的検査を通さずに配られた IR では、`use` の呼び出し先（`enter`）や戻り先（`exit_to`）が巡回していることがある。
//! 試作のエンジンは再帰でたどってスタックが溢れ、受講者のタブが落ちた（PoC-7 手順 7）。本モジュールは
//! `next` → `enter` → `exit_to` を再帰を使わずループでたどり、着地の回数を数えて
//! [`MAX_TRANSITIONS_PER_INPUT`]（1,000 回）に達したら実行を止め、理由を `Warning` 1 件で残す。
//! 呼び出し元は [`crate::confirm::achieve_with`]（受付・進行の完了からの達成の後の進み）と [`crate::go_to`]（移動先への着地）だけで、
//! 公開 API には含めない（契約の対象外）。
//!
//! # 数え方
//!
//! 「Step に着いた回数」を数える。次の着地をそれぞれ 1 回とする。
//!
//! - 歩行の最初の着地先（移動先、または `next` の辺の先）
//! - `enter` による呼び出し先への着地
//! - `exit_to` による呼び出し元への着地
//!
//! 着く前に `n = 着地済み + 1` を計算し、`n > MAX_TRANSITIONS_PER_INPUT` なら止める。1,000 回目の着地は通り、
//! 1,001 回目で止まる。予算は歩行の入口で作るローカルの値で、`SessionState` には置かない（欄を足すと版上げになるため）。
//! 1 回の操作の中で歩行を始めるのは、`accept` → `achieve_with`・`tick` → `achieve_with`・`go_to` のどれか 1 回だけである。
//!
//! # 止めたとき
//!
//! 固定文の `Warning` を 1 件だけ足し、`current`・`busy`・入口の写しを空にする。以降の入力は、現在の Step が無いので
//! 落ちずに無視される。入口の写しも空にするのは、`go_to` が古い写しを残さないのと同じ理由である（停止が古い写しへ戻る誤りを防ぐ）。
//! 止めた後の見出し・まとめ表示（SAFE-34）と戻り方（SAFE-35）は後続のタスクが扱い、本モジュールは `Warning` のままにする。
//!
//! # 扱わないこと
//!
//! 実体の状態を `Session` に持つこと・切り身とフラグの分離・遷移先の `on_enter` の効果・`busy` を立てること・
//! `ChapterCompleted`・`completed`・開始（`Begin`）での連鎖（TASK-217）は、TASK-233 以降が扱う。
//! それらは本モジュールの歩行器を通して拡張する前提で、巡回の検出に要る最小のたどりだけを置く。
//! 遷移の条件（`when`）は評価できないので、`next` は無条件の最初の辺だけを採る（[`crate::confirm`] と同じ）。
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。再帰を使わず、
//! 手数は予算で必ず止まる。I/O・時刻・乱数を持たない。`Warning` の文面は固定文で、教材の値を埋め込まない。

use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_contract::limits::MAX_TRANSITIONS_PER_INPUT;
use serde_json::Value;

use crate::addr::{Generation, Stamp, StepUid};
use crate::event::Event;
use crate::session::SessionState;
use crate::step_status::StepStatus;

/// 遷移先が IR に無いときの固定文。
const UNKNOWN_TARGET: &str = "transition target is not in the course; transition skipped";

/// 上限に達して止めたときの固定文。
const LIMIT_REACHED: &str = "internal transitions reached the per-input limit; run stopped";

/// 1 回の歩行で着地した回数。
struct Budget(usize);

impl Budget {
    /// 次の着地を 1 回数える。上限を超えるなら数えず `false`。
    fn take(&mut self) -> bool {
        let n = self.0.saturating_add(1);
        if n > MAX_TRANSITIONS_PER_INPUT {
            return false;
        }
        self.0 = n;
        true
    }
}

fn find<'a>(ir: &'a Ir, uid: &str) -> Option<&'a Node> {
    ir.nodes.iter().find(|n| n.uid == uid)
}

fn stamp(generation: Generation, uid: &str) -> Stamp {
    Stamp {
        generation,
        step: StepUid(uid.to_string()),
    }
}

fn warning(at: Stamp, message: &str) -> Event {
    Event::Warning {
        at,
        message: message.to_string(),
    }
}

fn set_status(state: &mut SessionState, uid: &str, status: StepStatus) {
    let uid = StepUid(uid.to_string());
    state
        .run
        .slices
        .entry(uid.root())
        .or_default()
        .status
        .insert(uid, status);
}

/// 上限で止める。fail-closed で、現在の Step・処理中・入口の写しを空にする。
fn halt(state: &mut SessionState, at: Stamp, events: &mut Vec<Event>) {
    events.push(warning(at, LIMIT_REACHED));
    state.run.current = None;
    state.run.busy = None;
    state.entry = None;
}

/// 移動先 `to` に着地する（`go_to` の入口）。`to` が IR にあることは呼び出し元が確かめ済み。
///
/// `generation` は移動で進めた後の世代で、このとき出す `ModuleEntered`・`Entered` の `Stamp` に使う。
pub(crate) fn move_to(
    ir: &Ir,
    state: &mut SessionState,
    generation: Generation,
    to: &str,
    events: &mut Vec<Event>,
) {
    let mut budget = Budget(0);
    let Some(node) = find(ir, to) else {
        events.push(warning(stamp(generation, to), UNKNOWN_TARGET));
        return;
    };
    if !budget.take() {
        halt(state, stamp(generation, to), events);
        return;
    }
    land(ir, state, generation, node, &mut budget, events);
}

/// 達成にした `node` から先へ進む（`achieve_with` の入口）。
///
/// 無条件の最初の辺があればその先へ着地し、無ければ `final` の Step の `exit_to`（呼び出し元）へ戻って同じことを
/// 繰り返す。どちらも無ければ何もしない（章の完了は扱わない）。`at` は `node` の達成時の `Stamp`。
pub(crate) fn after_achieved(
    ir: &Ir,
    state: &mut SessionState,
    node: &Node,
    at: &Stamp,
    events: &mut Vec<Event>,
) {
    let generation = at.generation;
    let mut budget = Budget(0);
    let mut node = node;
    let mut at = at.clone();
    loop {
        // 無条件の最初の辺だけを採る。`when` は評価できないので、条件つきの辺は採らない。
        if let Some(edge) = node
            .step
            .next
            .iter()
            .find(|t| t.when.as_ref().is_none_or(Value::is_null))
        {
            let Some(target) = find(ir, &edge.to) else {
                events.push(warning(at, UNKNOWN_TARGET));
                return;
            };
            if !budget.take() {
                halt(state, at, events);
                return;
            }
            events.push(Event::Transitioned {
                at,
                to: StepUid(target.uid.clone()),
            });
            land(ir, state, generation, target, &mut budget, events);
            return;
        }
        let Some(back) = node
            .exit_to
            .as_deref()
            .filter(|_| node.step.is_final)
            .and_then(|c| find(ir, c))
        else {
            if node.step.is_final && node.exit_to.is_some() {
                events.push(warning(at, UNKNOWN_TARGET));
            }
            return;
        };
        if !budget.take() {
            halt(state, at, events);
            return;
        }
        set_status(state, &back.uid, StepStatus::Achieved);
        // 戻り先の先へ進めない場合に備え、現在位置も呼び出し元へ移す（ModuleExited の復帰先と一致させる）。
        state.run.current = Some(StepUid(back.uid.clone()));
        let back_at = stamp(generation, &back.uid);
        events.push(Event::ModuleExited {
            at: back_at.clone(),
            instance: node.scope.clone(),
        });
        node = back;
        at = back_at;
    }
}

/// 着地済み（予算を数え済み）の `node` に着く。`use` の Step なら呼び出し先へ進み続け、通常の Step で止まる。
fn land(
    ir: &Ir,
    state: &mut SessionState,
    generation: Generation,
    node: &Node,
    budget: &mut Budget,
    events: &mut Vec<Event>,
) {
    let mut node = node;
    loop {
        let Some(enter) = node.enter.as_deref() else {
            let uid = StepUid(node.uid.clone());
            state.run.current = Some(uid.clone());
            set_status(state, &node.uid, StepStatus::Entered);
            events.push(Event::Entered {
                at: Stamp {
                    generation,
                    step: uid,
                },
            });
            return;
        };
        let here = stamp(generation, &node.uid);
        let Some(callee) = find(ir, enter) else {
            events.push(warning(here, UNKNOWN_TARGET));
            return;
        };
        set_status(state, &node.uid, StepStatus::Entered);
        events.push(Event::ModuleEntered {
            at: here.clone(),
            instance: callee.scope.clone(),
        });
        if !budget.take() {
            halt(state, here, events);
            return;
        }
        node = callee;
    }
}
