//! 処理中の停止 `Control::Stop` の分岐（TASK-211 1/2、EVT-9）。
//!
//! 処理中に停止されたとき、世代を 1 つ進め、処理中を解き、現在の Step に入った時点の状態（入口の写し）へ
//! 実行状態を戻す。受付から停止までの世界状態の変化は、写しへ戻ることで取り消される。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 世代の規則の表（`IncrementIfProcessing`）の強制は `apply` の関門が行い、本モジュールは表を再実装しない。
//! `Changed` は本モジュールでは作らない。`apply` が前後の世界状態の差分から経路ごとに 1 件を作る（EVT-13）。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 処理中でない、または現在の Step が無いなら、状態を変えず空のイベント列を返す
//! 2. 世代があふれる、入口の写しが無い、写しの Step が現在の Step と違う、のいずれかなら、
//!    固定文の `Warning` だけを出して状態を変えない（fail-closed。戻り先が不確かなまま一部だけ戻さない）
//! 3. `run` を写しの `run` で置き換え、世代を操作前の +1、`busy` を `None` にする。
//!    `Stopped`（`at` は操作前の世代と現在の Step）を 1 件返す
//!
//! 戻すのは `run` だけで、`log`・`drafts`・`hints_opened`・`checkpoints`・`entry` には触れない（MOD-4・GUIDE-7）。
//! 案内レベルは `run` の中にあるため写しの値へ戻る（根ごとの戻り方と併せて TASK-242 が見直しうる）。
//!
//! # 扱わないこと
//!
//! - 処理中でない停止の取消としての扱い（TASK-212）
//! - 実体（根）ごとの戻り方と、Step に入るたびの入口の写しの作り直し（TASK-242・217。移動は写しを無効化する、TASK-213）
//! - 停止前の宛先の進行は、`tick` が世代の照合（`judge_tick`）で `Ignored` に記録する。試験は `tests/stop_restore.rs`（TASK-211 2/2）
//! - 変換中の Esc を停止にしない処理（JUDGE-16。web の担当）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 教材（IR）を読まないので、停止の意味を教材が変えることはできない。`Warning` の文面は固定文で値を埋め込まない。

use crate::addr::Stamp;
use crate::event::Event;
use crate::session::SessionState;

/// 現在の Step の入口の写しが無い（または別の Step のもの）ときの固定文。
const MISSING_ENTRY: &str = "entry copy for the current step is missing; stop ignored";

/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; stop ignored";

/// `Control::Stop` の分岐。処理中なら入口の写しへ戻し、出たイベント列を返す。
pub(crate) fn on_stop(state: &mut SessionState) -> Vec<Event> {
    let (Some(_), Some(current)) = (state.run.busy.as_ref(), state.run.current.clone()) else {
        return Vec::new();
    };
    // `at` は操作前の状態から作る（外から来た値は使わない）。
    let at = Stamp {
        generation: state.run.generation,
        step: current.clone(),
    };
    let warn = |message: &str| {
        vec![Event::Warning {
            at: at.clone(),
            message: message.to_string(),
        }]
    };
    let Some(next) = state.run.generation.next() else {
        return warn(GENERATION_OVERFLOW);
    };
    let Some(entry) = state
        .entry
        .as_ref()
        .filter(|e| e.run.current.as_ref() == Some(&current))
    else {
        return warn(MISSING_ENTRY);
    };
    let mut run = entry.run.clone();
    run.generation = next;
    run.busy = None;
    state.run = run;
    vec![Event::Stopped {
        at,
        new_generation: next,
    }]
}
