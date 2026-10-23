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
//! 固定文の `Warning` を 1 件と、その直後に専用の `TransitionLimitReached`（SAFE-34）を 1 件だけ足し、
//! 内側では `current`・`busy`・入口の写しを空にし（fail-closed）、`apply` が直後に操作の前の状態へ戻す（[`return_after_limit`]、SAFE-35）。
//! 着地先の `enter` が IR に無いときも、`UNKNOWN_TARGET` の `Warning` を 1 件足して同じ空の状態に確定する（移動元が現在位置に残らない）。
//! 受講者向けの見出し・まとめ表示（SAFE-34）は web が `TransitionLimitReached` から作る。止めた後は、操作の前の Step が現在の Step に戻り、次の入力が受け付けられる（SAFE-35）。
//!
//! # 扱わないこと
//!
//! 復帰の範囲のうち世界状態・案内レベル・呼び出しの Step の持ち主（TASK-241 2/2）・遷移先の `on_enter` の効果・`busy` を立てること・
//! `ChapterCompleted`・`completed`・飛ばした Step の記録（TASK-217。開始の着地そのものは `begin` が `move_to` を通す）は、後続（TASK-235〜241）が扱う。
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
use crate::instance;
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

fn set_status(ir: &Ir, state: &mut SessionState, uid: &str, status: StepStatus) {
    let uid = StepUid(uid.to_string());
    instance::slice_mut(&mut state.run, instance::owner_of(ir, &uid))
        .status
        .insert(uid, status);
}

/// 上限で止める。fail-closed で、現在の Step・処理中・入口の写しを空にする。
///
/// 作成者向けの `Warning`（SAFE-23）の直後に、受講者向けの専用の `TransitionLimitReached`（SAFE-34）を 1 件足す。
/// 受講者向けの文面は web が固定文で持つので、ここでは着地の回数だけを渡す。
fn halt(state: &mut SessionState, at: Stamp, budget: &Budget, events: &mut Vec<Event>) {
    events.push(warning(at.clone(), LIMIT_REACHED));
    events.push(Event::TransitionLimitReached {
        at,
        landings: u32::try_from(budget.0).unwrap_or(u32::MAX),
    });
    // 内側の fail-closed。戻す先が無い場合（操作の前に現在の Step が無い）に備え、まず空にする。
    // 通常は呼び出し元 `apply` が直後に [`return_after_limit`] で操作の前へ戻す（SAFE-35）。
    state.run.current = None;
    state.run.busy = None;
    state.entry = None;
}

/// 上限で止めた操作を戻した結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LimitReturn {
    /// 戻す先が無く、空のまま止めた（fail-closed）。
    Halted,
    /// 操作の前の状態へ戻した。
    Restored,
    /// 処理中だったので入口の写しの状態へ戻した。世界状態が操作の前から変わりうる。
    RewoundToEntry,
}

/// 上限で止めた操作を、その操作の前の状態へ戻す（SAFE-35、D-45）。呼び出し元は `session::apply` だけ。
///
/// 戻す単位は `run` 全体と入口の写しで、巡回の途中の着地点ではなく受講者が操作の前にいた Step へ戻る。
/// 処理中（`busy`）だった操作は、次のとおり戻す。
///
/// - 入口の写しが得られれば、入口の状態（入力を受け付け直せる状態）へ戻す。世界状態も入口へ戻るので、
///   呼び出し元 `apply` が差分を原因 `Stop` の `Changed` にして記録する（学習者の変更として併合させない）
/// - 写しは状態に書かず、操作の前のログから処理を始めた受付の時点を導く（[`crate::entry_derive`]。状態に
///   現在の Step の写しがあればそれを使う）。受付から処理中になった通常の経路はこちらに当たり、受付の効果を
///   二重に適用せずに入力を受け付け直せる
/// - 受付の時点を一意に決められないとき（ログに受付が無い・処理中の間にあり得ないイベントがある・復帰の写しが
///   食い違う）は、操作の前の `run` から `busy` だけを空にして戻す。現在の Step を失わないので、保存・移動・
///   復帰などは働く。受付済みのまま入力で進めない Step が残りうるが、受付の効果の二重適用より安全側とする
///
/// 処理中だった操作は世代を +1 する（`go_to` のように既に進んでいるときは、その値を残す）。復帰前に出した
/// 進行の宛先（世代・Step・番号）を、再入力で同じ Step が処理中になった後も無効にするためである。世代が
/// あふれるときは戻さずに空のまま止める（fail-closed）。`busy` は必ず空にする。
/// 操作の前に現在の Step が無かったときも戻す先が無いので、空のまま止める。
/// 止めた操作が出した `Accepted` 等はログに残る（ログは追記だけ）。復習は `Projection` が
/// `TransitionLimitReached` で操作の前の復習へ戻して Step の状態と揃える。
pub(crate) fn return_after_limit(
    state: &mut SessionState,
    run_before: &crate::session::RunState,
    entry_before: &Option<crate::session::EntryCopy>,
) -> LimitReturn {
    if run_before.current.is_none() {
        return LimitReturn::Halted;
    }
    let mut generation = state.run.generation;
    let mut outcome = LimitReturn::Restored;
    let mut run = if run_before.busy.is_some() {
        if generation == run_before.generation {
            let Some(next) = generation.next() else {
                return LimitReturn::Halted;
            };
            generation = next;
        }
        // `state.log`・`state.checkpoints` は、まだ操作の前のもの（`apply` がログへ足すのはこの後）。
        match crate::entry_derive::stored_or_derived(
            run_before,
            entry_before.as_ref(),
            &state.log,
            &state.checkpoints,
        ) {
            Some(entry) => {
                outcome = LimitReturn::RewoundToEntry;
                entry.run
            }
            None => run_before.clone(),
        }
    } else {
        run_before.clone()
    };
    run.generation = generation;
    run.busy = None; // 戻した後は必ず処理中でない
    state.run = run;
    state.entry = entry_before.clone();
    outcome
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
        halt(state, stamp(generation, to), &budget, events);
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
                halt(state, at, &budget, events);
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
            halt(state, at, &budget, events);
            return;
        }
        set_status(ir, state, &back.uid, StepStatus::Achieved);
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
            set_status(ir, state, &node.uid, StepStatus::Entered);
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
            // 呼び出し先が無いと着地先の Step を入力待ちにできない。移動元を現在位置に残すと
            // `Moved`・`Transitioned` の宛先と食い違うので、上限で止めるときと同じ空の状態に確定する。
            events.push(warning(here, UNKNOWN_TARGET));
            state.run.current = None;
            state.run.busy = None;
            state.entry = None;
            return;
        };
        set_status(ir, state, &node.uid, StepStatus::Entered);
        events.push(Event::ModuleEntered {
            at: here.clone(),
            instance: callee.scope.clone(),
        });
        if !budget.take() {
            halt(state, here, budget, events);
            return;
        }
        node = callee;
    }
}
