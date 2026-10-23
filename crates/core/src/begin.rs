//! 次の周回の開始 `Control::Begin` の分岐（TASK-237 1/2、MOD-3・MOD-5）。
//!
//! 閉じた実行の続きに、新しい開始点から次の周回を始める。呼び出し元は [`crate::session::apply`] だけで、
//! 公開 API には含めない（契約の対象外）。世代の規則（`IncrementOnSuccess`）の強制は `apply` の関門が行い、
//! 本モジュールは表を再実装しない。世界状態・前の周回の切り身・フラグ・下書き・ヒントは引き継ぎ、
//! 開始点の `effects` が書く経路だけが変わる（周回の初期状態は TASK-235 の持ち主の規則が保つ）。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 開始条件を [`crate::start_gate::check_start`] で状態に触れる前に評価する。開始 ID が無い・条件を満たさない
//!    なら `Refused`（`UnknownStart`・`StartUnmet`）を 1 件だけ返し、状態を変えない（fail-closed。完全な保証の試験は TASK-240）
//! 2. 世代があふれるなら、固定文の `Warning` だけを出して状態を変えない
//! 3. 開始点の `effects` が 1 件でも読めないなら、固定文の `Warning` だけを出して状態を変えない（全件か無し）
//! 4. 世代を +1、`busy` と完了を解く。入口の写しは無効化する（作り直しは `transit` の着地の無効化と導出。TASK-242 1/2。`go_to` の前例に合わせる）
//! 5. `Begun` を返し、開始の節点より前の Step を省略にする（下記）。続けて開始点の `effects` を原因 `Start` で適用し、
//!    開始の節点へ [`crate::transit`] で着地する。`use` の節点は呼び出し先まで進み、巡回は歩行器の予算で止まって
//!    `apply` が操作の前へ戻す（状態は戻り、ログの `Begun` は残る。`Skipped` は止まったときに取り消し、ログにも残さない）
//!
//! # 飛ばした Step の省略（TASK-217 1/2、EVT-21・MOD-6）
//!
//! 規則 1〜3 のすべての関門を通った後にだけ、IR の順で開始の節点より前にある節点のうち次の 4 条件をすべて満たすものを
//! `Skipped` にし、`Skipped` を 1 件ずつ返す（世代は `Begun` の新しい世代）。「達成」にはしない。
//!
//! - 開始の節点と同じ `unit`（別の章・モジュールの Step を巻き込まない）
//! - 開始の節点と同じ `scope`（前の周回など別の実体の切り身を書かない）
//! - 呼び出しの Step（`use`）ではない（持ち主が呼び出し先の実体になるため。実体ごとの途中開始は TASK-239）
//! - 状態が `NotStarted`（達成済み・受付済みの Step を上書きしない。同じ開始点への再度の開始で重複しない）
//!
//! `at` は操作前の世代と現在の Step で作る。現在の Step が無い新しい状態では空の uid を使う（`hint_open` の前例）。
//!
//! # 扱わないこと
//!
//! - `start.guidance` による案内の水準（TASK-294）、
//!   開始用の `${名前}` の置換（TASK-239）、閉じた根の引き継ぎ（TASK-238）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 開始 ID は未信頼の値で、`Warning` の文面に埋め込まない。`Begun.start` は IR から引いた値を入れる。

use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Ir;

use crate::addr::{Root, Stamp, StepUid};
use crate::condition::VocabConditions;
use crate::effect::apply_effects;
use crate::event::{Cause, ControlKind, Event, RefuseReason};
use crate::instance;
use crate::session::SessionState;
use crate::start_gate::check_start;
use crate::step_status::StepStatus;
use crate::transit;

/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; begin ignored";
/// 開始点の効果が読めないときの固定文。
const EFFECTS_UNREADABLE: &str = "start effects unreadable; begin ignored";

/// `Control::Begin` の分岐。成功なら新しい開始点の周回へ入り、出たイベント列を返す。
pub(crate) fn on_begin(ir: &Ir, state: &mut SessionState, start_id: &str) -> Vec<Event> {
    let at = Stamp {
        generation: state.run.generation,
        step: state
            .run
            .current
            .clone()
            .unwrap_or_else(|| StepUid(String::new())),
    };
    let start = match check_start(ir, &state.run, start_id) {
        Ok(start) => start,
        Err(refusal) => {
            return vec![Event::Refused {
                at,
                op: ControlKind::Begin,
                reason: refusal.reason(),
                rejection: None,
            }];
        }
    };
    let Some(next) = state.run.generation.next() else {
        return vec![Event::Warning {
            at,
            message: GENERATION_OVERFLOW.to_string(),
        }];
    };
    let items: Option<Vec<EffectItem>> = start
        .effects
        .iter()
        .map(|v| EffectItem::from_value(v).ok())
        .collect();
    let Some(items) = items else {
        return vec![Event::Warning {
            at,
            message: EFFECTS_UNREADABLE.to_string(),
        }];
    };
    let Some(target_idx) = ir.nodes.iter().position(|n| n.uid == start.node) else {
        return vec![Event::Refused {
            at,
            op: ControlKind::Begin,
            reason: RefuseReason::UnknownStart,
            rejection: None,
        }];
    };
    let Some(node) = ir.nodes.get(target_idx) else {
        return vec![Event::Refused {
            at,
            op: ControlKind::Begin,
            reason: RefuseReason::UnknownStart,
            rejection: None,
        }];
    };
    state.run.generation = next;
    state.run.busy = None;
    state.run.completed = false;
    state.entry = None;
    let mut events = vec![Event::Begun {
        at,
        start: start.id.clone(),
        new_generation: next,
    }];
    let skipped: Vec<StepUid> = ir
        .nodes
        .iter()
        .take(target_idx)
        .filter(|n| n.unit == node.unit && n.scope == node.scope && n.step.use_module.is_none())
        .map(|n| StepUid(n.uid.clone()))
        .filter(|uid| instance::status_of(ir, &state.run, uid) == StepStatus::NotStarted)
        .collect();
    let skipped_uids = skipped.clone();
    for uid in skipped {
        instance::slice_mut(&mut state.run, instance::owner_of(ir, &uid))
            .status
            .insert(uid.clone(), StepStatus::Skipped);
        events.push(Event::Skipped {
            at: Stamp {
                generation: next,
                step: uid,
            },
        });
    }
    events.extend(apply_effects(
        &mut state.run,
        &Root(node.scope.clone()),
        &items,
        &Stamp {
            generation: next,
            step: StepUid(start.node.clone()),
        },
        Cause::Start,
        &VocabConditions::new(&ir.instances),
    ));
    transit::move_to(ir, state, next, &start.node, &mut events);
    if events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. }))
    {
        // 着地が上限で止まると `apply` が操作の前へ戻す（SAFE-35）。戻す先が無い新しい状態でも省略だけは
        // 残さないよう、省略の状態とイベントをここで取り消す（`Begun` は残る）。
        for uid in &skipped_uids {
            instance::slice_mut(&mut state.run, instance::owner_of(ir, uid))
                .status
                .remove(uid);
        }
        events.retain(|e| !matches!(e, Event::Skipped { .. }));
    }
    events
}
