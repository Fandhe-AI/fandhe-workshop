//! 再読み込みの分岐（TASK-320 1/2、SAVE-4）。
//!
//! 処理中にページを再読み込みしても停止にせず、処理中のまま続ける。再読み込みでは世代を 1 つ進め、
//! 進行の宛先を新しい世代で作り直す（古い世代の宛先は `tick` の照合で `Ignored` に記録される。
//! 試験は `tests/reload_busy.rs`・`tests/history_verify.rs`、TASK-320 2/2）。
//! 呼び出し元は [`crate::session::reload`] と [`crate::session::Session::reload`] だけで、公開 API には
//! 含めない（契約の対象外）。`apply()` の `Op` ではないので、世代の規則の表（`GenerationRule`）は変えない。
//! 履歴の `ops` の `Reload` は、記録側（[`crate::ops_record::reload_recorded`]）も適用し直し側
//! （[`crate::history_replay::replay`]）も同じ関数を通る。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず空のイベント列を返す（`Stamp` の Step を作れないため。fail-closed）
//! 2. 世代があふれるなら、固定文の `Warning` だけを出して状態を変えない
//! 3. 世代を操作前の +1 にし、`Reloaded`（`at` は操作前の世代と現在の Step）を 1 件返す
//!
//! 世代以外は変えない。`busy`・`current`・`world`・`slices`・`entry`・`checkpoints`・`drafts`・
//! `hints_opened` は再読み込みの前のままである（入口の写しは作り直さない。SAVE-4）。処理中かどうかに
//! 関わらず世代は +1 する。
//!
//! # 扱わないこと
//!
//! - 右のログの行の文言（行は `Projection` が 1 件につき 1 行出す。SAVE-6・TASK-322。文言は web の `log_lines` が持つ）
//! - web の結線（履歴を読み込んで復元する経路は TASK-319・TASK-331 に依存する）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 教材（IR）を読まない。`Warning` の文面は固定文で値を埋め込まない。

use crate::addr::Stamp;
use crate::event::Event;
use crate::session::SessionState;

/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; reload ignored";

/// 再読み込みの分岐。世代を +1 し、出たイベント列を返す。
pub(crate) fn on_reload(state: &mut SessionState) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    // `at` は操作前の状態から作る。
    let at = Stamp {
        generation: state.run.generation,
        step: current,
    };
    let Some(next) = state.run.generation.next() else {
        return vec![Event::Warning {
            at,
            message: GENERATION_OVERFLOW.to_string(),
        }];
    };
    state.run.generation = next;
    vec![Event::Reloaded {
        at,
        new_generation: next,
    }]
}
