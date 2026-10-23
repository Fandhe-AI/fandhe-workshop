//! 処理中の停止 `Control::Stop` の分岐（TASK-211 1/2、EVT-9）。
//!
//! 処理中に停止されたとき、世代を 1 つ進め、処理中を解き、入口の写し（通常は処理を始めた受付の直前の状態）へ
//! 現在の Step の持ち主の分だけを戻す（MOD-10）。持ち主が受付から停止までに書いた世界状態の変化は、写しへ戻ることで取り消される。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//! 世代の規則の表（`IncrementIfProcessing`）の強制は `apply` の関門が行い、本モジュールは表を再実装しない。
//! `Changed` は本モジュールでは作らない。`apply` が前後の世界状態の差分から経路ごとに 1 件を作る（EVT-13）。
//!
//! # 規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 処理中でない、または現在の Step が無いなら、状態を変えず空のイベント列を返す（処理中でない停止は
//!    [`crate::session::apply`] が取り消しの入力として `accept` へ渡すので、通常ここへは来ない。TASK-212）
//! 2. 世代があふれる、入口の写しが得られない、写しのログ位置がログの長さを超える、のいずれかなら、固定文の `Warning` だけを出して状態を変えない
//!    （fail-closed。戻り先が不確かなまま一部だけ戻さない）。写しは、状態に現在の Step の写しがあればそれを、
//!    無ければログから導いたもの（受付の時点の `run`）を使う（[`crate::entry_derive`]）。通常の操作では状態の
//!    写しは空なので、導いたものを使う。受付の時点を一意に決められないときは得られない
//! 3. 持ち主の根（`owner_of`。保存・`Changed` の宛先の判定と同じ。処理中の Step は呼び出しの Step にならないので
//!    `root_of` と同じ値になる）の切り身と `current`・`completed` を写しの値へ戻す（写しに無い切り身は取り除く）。
//!    世界状態は、写しのログ位置（`log_len`）以降の `Changed` のうち宛先の持ち主が停止の持ち主と一致するものの経路だけを
//!    写しの値へ戻す（写しに無ければ取り除く）。他の根の切り身・他の経路・`global_flags` は停止の直前のまま残す
//!    （F-3 の変異 X2「写しを丸ごと戻すと他の実体まで巻き戻る」を防ぐ。復帰の規則 6 と同じ形で、補助関数は
//!    [`crate::entry_derive::revert_owner`] を共有する）。案内レベルは写しの値に戻す（処理中には変わらないので同値）。
//!    世代を操作前の +1、`busy` を `None` にし、`Stopped`（`at` は操作前の世代と現在の Step）を 1 件返す。
//!    根が 1 つの教材では、受付の後の `Changed` はすべて持ち主のものなので、結果は全体を戻すのと一致する
//!
//! 戻すのは `run` の上の持ち主の分だけで、`log`・`drafts`・`hints_opened`・`checkpoints`・`entry` には触れない（MOD-4・GUIDE-7）。
//!
//! # 扱わないこと
//!
//! - 処理中でない停止の取り消しの入力としての扱い（`session::step` と `accept`、TASK-212）
//! - Step に入った時点の写し（置き換えは `transit` が済ませた。TASK-242 1/2・217。移動は状態の写しを無効化する、TASK-213。
//!   導く写しは受付の時点のもので、移動の後の受付からは移動先の受付の時点を導く）
//! - 停止前の宛先の進行は、`tick` が世代の照合（`judge_tick`）で `Ignored` に記録する。試験は `tests/stop_restore.rs`（TASK-211 2/2）
//! - 変換中の Esc を停止にしない処理（JUDGE-16。web の担当）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 教材から読むのは実体の表（持ち主の決定）だけで、停止の意味を教材は変えられない。`Warning` の文面は固定文で値を埋め込まない。

use fandhe_workshop_contract::ir::Ir;

use crate::addr::Stamp;
use crate::event::Event;
use crate::session::SessionState;

/// 現在の Step の入口の写しが無い（または別の Step のもの）ときの固定文。
const MISSING_ENTRY: &str = "entry copy for the current step is missing; stop ignored";

/// 写しのログ位置が現在のログの長さを超えるときの固定文。
const LOG_POSITION_OUT_OF_RANGE: &str = "entry copy log position is out of range; stop ignored";

/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; stop ignored";

/// `Control::Stop` の分岐。処理中なら入口の写しへ戻し、出たイベント列を返す。
pub(crate) fn on_stop(ir: &Ir, state: &mut SessionState) -> Vec<Event> {
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
    // 写しは状態に書かず、ログから導く（`entry_derive`）。導けないときは戻らない（fail-closed）。
    // 従来は処理中の停止が `Warning` だけだった。未配信（エンジンの版が `0.0.0` のまま）の間の変更として
    // 版を上げず、変更前の開発版で保存した履歴は照合で開けない。版を上げる運用は #1058・#1059 で扱う。
    let Some(entry) = crate::entry_derive::entry_for_current(state) else {
        return warn(MISSING_ENTRY);
    };
    // 位置は履歴由来の信頼できない値。範囲外は経路の集合を決められないので、一部だけ戻さず状態を変えない。
    let Some(since) = usize::try_from(entry.log_len)
        .ok()
        .and_then(|n| state.log.get(n..))
    else {
        return warn(LOG_POSITION_OUT_OF_RANGE);
    };
    // 持ち主は保存（`checkpoint::on_save`）・`Changed` の宛先の判定と同じ `owner_of` で決める。
    let owner = crate::instance::owner_of(ir, &current);
    let touched = crate::entry_derive::owner_touched_paths(since, &ir.instances, &owner);
    let mut run = crate::entry_derive::revert_owner(&state.run, &entry.run, &owner, &touched);
    run.generation = next;
    run.busy = None;
    state.run = run;
    vec![Event::Stopped {
        at,
        new_generation: next,
    }]
}
