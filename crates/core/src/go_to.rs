//! 別の Step への移動 `Control::GoTo` の分岐（TASK-213 1/2、EVT-11）。
//!
//! 移動は教材の語彙の入力ではなく実行環境の操作である。成功したときだけ世代を 1 つ進め、処理中を解き、
//! 移動先の Step に入る。呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 世代の規則の表（`IncrementOnSuccess`）の強制は `apply` の関門が行い、本モジュールは表を再実装しない。
//! `Changed` は作らない。世界状態を変えないので出ない（停止との違い）。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず空のイベント列を返す（`Stamp` を作れない。開始前の移動は TASK-217）
//! 2. 移動先が IR に無いなら、状態を変えず `Refused`（`UnknownStep`）を 1 件返す
//! 3. 世代があふれるなら、固定文の `Warning` だけを出して状態を変えない（fail-closed）
//! 4. 世代を +1、`busy` を `None` にし、入口の写しは無効化する。`Moved` を返し、移動先への着地は [`crate::transit`] が行う。
//!    `use` でない移動先は現在の Step と状態 `Entered` を置き、`Entered` を返す（合計 2 件）。`use` の移動先は
//!    `ModuleEntered` を挟んで呼び出し先まで進む。巡回して 1,000 回に達したら `Warning` 1 件で止め、`apply` が移動元へ戻す（世代は +1 のまま。SAFE-23・SAFE-35）
//!
//! 開始（`Control::Begin`）が未実装の間、巡回の試験はこの移動で代わりに巡回の入口へ入る（開始での連鎖は TASK-217）。
//!
//! 出発側の Step の状態・選択・フラグ・世界状態・ログ・下書き・ヒント・チェックポイントには触れない。
//! 同じ Step への移動は入り直しとして成功する。
//!
//! # 入口の写しの扱い
//!
//! 写しを残したまま移動すると、写しの Step へ戻った後の停止が古い写しへ戻り、移動の間の達成と世界状態を
//! 消す誤った巻き戻しになる。新しい写しの作成は TASK-242・217 の決定なので、ここでは `None` にして、
//! 移動後の停止は `stop` の `Warning` で状態を変えない側に倒す。
//!
//! # 移動前に予約された進行
//!
//! 移動は世代を +1 するので、移動前の進行は `tick` が世代の照合（`judge_tick`）で `Ignored` に記録する。
//! 試験は `tests/move_step.rs`（TASK-213 2/2）。
//!
//! # 扱わないこと
//!
//! - 移動先の `on_enter` の効果と、入口の写しの作り直し（TASK-242・217）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 移動先の uid は IR の Node と一致したときだけ受け入れ、識別子としてだけ保持する。`Warning` の文面は固定文で値を埋め込まない。

use fandhe_workshop_contract::ir::Ir;

use crate::addr::{Stamp, StepUid};
use crate::event::{ControlKind, Event, RefuseReason};
use crate::session::SessionState;
use crate::transit;

/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; move ignored";

/// `Control::GoTo` の分岐。成功なら移動先へ入り、出たイベント列を返す。
pub(crate) fn on_go_to(ir: &Ir, state: &mut SessionState, to: StepUid) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    // `at` は操作前の状態から作る（外から来た値は使わない）。
    let at = Stamp {
        generation: state.run.generation,
        step: current,
    };
    if !ir.nodes.iter().any(|n| n.uid == to.0) {
        return vec![Event::Refused {
            at,
            op: ControlKind::GoTo,
            reason: RefuseReason::UnknownStep,
            rejection: None,
        }];
    }
    let Some(next) = state.run.generation.next() else {
        return vec![Event::Warning {
            at,
            message: GENERATION_OVERFLOW.to_string(),
        }];
    };
    state.run.generation = next;
    state.run.busy = None;
    state.entry = None;
    // 着地は歩行器が行う。`use` の移動先は呼び出し先まで進み、巡回は上限で止まる（TASK-160、SAFE-23）。
    let mut events = vec![Event::Moved {
        at,
        to: to.clone(),
        new_generation: next,
    }];
    transit::move_to(ir, state, next, &to.0, &mut events);
    events
}
