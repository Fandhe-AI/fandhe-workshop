//! ヒントを 1 段開く操作 `Control::OpenHint` の分岐（TASK-295、GUIDE-17）。
//!
//! 現在の Step で現在の案内レベルに出せるヒントの数 n（[`crate::hint_select::current_hints`] の長さ）を上限に、
//! 1 回の操作で 1 段だけ開く。段数は教材が決め、エンジンは 3 段に固定しない（D-55 で固定しないと決まった。
//! 規則の試験は `tests/hint_limit_rule.rs`、39 本の確認は pack の `tests/hint_limit_39.rs`）。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 世代は成功でも拒否でも変わらない（`Keep`）。`apply` の関門が表を強制し、本モジュールは世代に触れない。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず `Refused`（`NoHint`）を 1 件返す。印の Step は空の uid にする
//!    （`changed` の既定の印と同じ。移動・保存が空の列を返すのと違い、GUIDE-17 は拒否の記録を求める）
//! 2. 開いた段数が出せる数 n 以上なら、状態を変えず `Refused`（`NoHint`）を 1 件返す。IR に無い Step・読めない定義だけの
//!    Step・ヒントの無い Step は n が 0 なのでここに当たる（fail-closed）。案内レベルを下げて n が開いた段数を
//!    下回った場合もここで拒否し、開いた段数は減らさない
//! 3. 段数を 1 増やして `HintOpened` を 1 件返す
//!
//! 鍵は Step の実体 ID（根を含む完全な uid）で、周回・実体ごとに分かれる（GUIDE-7、TASK-297 1/2）。
//! 実体（根）ごとの合計は [`crate::session::SessionState::hints_opened_in`] で読む。
//!
//! 処理中でも開ける（処理中の扱いの規定が無いため制限を足さない）。`run` には触れず、世界状態も変えないので
//! `Changed` は出ない。
//!
//! # 扱わないこと
//!
//! - 開いた段数が減らないことは、欄を `run` の外に置く構造で成り立つ。分岐ごとの試験は `tests/hint_count_monotone.rs`（変異 `GUIDE7-3`〜`GUIDE7-5`）、
//!   周回をまたぐ性質は pack の `round_hint_counts`、送信 0 件の計数は `tools/browser/egress-count.test.mjs` の G3 が持つ（TASK-297 2/2）
//! - 案内レベルの切り替え（TASK-294）、ヒントの表示と開くボタン（TASK-530）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 印は操作前の状態からだけ作り、イベントに教材の本文・利用者の入力を埋め込まない。

use fandhe_workshop_contract::ir::Ir;

use crate::addr::{Stamp, StepUid};
use crate::event::{ControlKind, Event, RefuseReason};
use crate::hint_select;
use crate::session::SessionState;

/// `Control::OpenHint` の分岐。開けたら `HintOpened`、開けなければ状態を変えずに `Refused` を返す。
pub(crate) fn on_open_hint(ir: &Ir, state: &mut SessionState) -> Vec<Event> {
    let generation = state.run.generation;
    let refuse = |step: StepUid| {
        vec![Event::Refused {
            at: Stamp { generation, step },
            op: ControlKind::OpenHint,
            reason: RefuseReason::NoHint,
            rejection: None,
        }]
    };
    let Some(current) = state.run.current.clone() else {
        return refuse(StepUid(String::new()));
    };
    let total = u32::try_from(hint_select::current_hints(ir, &state.run).len()).unwrap_or(u32::MAX);
    let key = current.clone();
    let opened = state.hints_opened.get(&key).copied().unwrap_or(0);
    if opened >= total {
        return refuse(current);
    }
    let Some(next) = opened.checked_add(1) else {
        return refuse(current);
    };
    state.hints_opened.insert(key, next);
    vec![Event::HintOpened {
        at: Stamp {
            generation,
            step: current,
        },
        opened: next,
        total,
    }]
}
