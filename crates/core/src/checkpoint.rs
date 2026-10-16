//! チェックポイントの保存 `Control::Save` と復帰 `Control::Restore` の分岐（TASK-214 1/3、EVT-12）。
//!
//! 学習者が途中の状態を控え、先へ進んだ後にその時点へ戻れるようにする。保存は世代を変えず、復帰は成功した
//! ときだけ世代を 1 つ進める。世代を進めるのは、復帰前に予約された進行を古い宛先として無視させるためである
//! （無視の試験は TASK-214 3/3）。呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない
//! （契約の対象外）。世代の規則の表（`Keep`・`IncrementOnSuccess`）の強制は `apply` の関門が行い、
//! 本モジュールは表を再実装しない。教材（IR）は読まない（教材が保存・復帰の意味を変えられない）。
//!
//! # 保存の規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず空のイベント列を返す（`Stamp` を作れない。開始前の扱いは TASK-217）。
//!    これにより保存した `run.current` は必ず `Some` になり、復帰の `Restored.owner` を作れることが保証される
//! 2. 次の ID があふれるなら、固定文の `Warning` だけを出して状態を変えない（fail-closed）
//! 3. 実行状態と入口の写しを写し取り、`CheckpointSaved` を 1 件返す。`log_len` は保存操作の直前のログの長さ
//!    （`CheckpointSaved` 自身のログ上の位置）で、`Projection` が復習を控える位置と一致する
//!
//! 処理中でも保存できる。世代・ログ・下書き・ヒント・世界状態には触れない（`Changed` は出ない）。
//!
//! # 復帰の規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず空のイベント列を返す（開始前の復帰は TASK-217）
//! 2. 無い ID なら、状態を変えず `Refused`（`UnknownCheckpoint`）を 1 件返す
//! 3. 世代があふれるなら、固定文の `Warning` だけを出して状態を変えない
//! 4. 写しの `run.current` が無い（読み込んだ写しの不整合）なら、固定文の `Warning` だけで状態を変えない
//!    （一部だけ戻さない）
//! 5. `run` 全体を写しに戻し、世代だけは現在の世代 +1 にする（世代は減らない）。`busy` は写しのまま残す。
//!    入口の写しも保存時点のものへ戻す（移動で無効化された写しを戻さないと、復帰後の停止が写し無しで倒れる）。
//!    写しの中の世代は書き換えない（停止が戻すときに世代を上書きするので値は使われない）。
//!    `Restored` を 1 件返す。`log_len` は写しの値で、現在のログの長さではない（EVT-14）
//!
//! `Changed` は作らない。`apply` が前後の世界状態の差から `Cause::Restore` で作る。ログ（追記のみ）・
//! 下書き・ヒント・チェックポイント（同じ ID へ何度でも戻れる）・次の ID には触れない。
//! `log_len` の範囲検査はしない。不正な位置は `Projection` が写し無しに倒す。
//!
//! # 扱わないこと
//!
//! - 復習・変化の印の導出そのもの（`projection`。`apply()` を通した試験は `tests/restore_review.rs`、TASK-214 2/3）
//! - 復帰前の進行の無視と処理中の再開の試験（TASK-214 3/3）
//! - 実体（根）ごとの戻り方（MOD-9、TASK-241）。ここでは `run` 全体を戻す
//! - 開始前の保存・復帰（TASK-217）、章の終わりの自動保存、保存の数の上限
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! ID は履歴由来の信頼できない値で、鍵としてだけ使う。`Warning` の文面は固定文で値を埋め込まない。

use crate::addr::Stamp;
use crate::event::{ControlKind, Event, RefuseReason};
use crate::input::CheckpointId;
use crate::session::{Checkpoint, SessionState};

/// 次の ID があふれるときの固定文。
const ID_OVERFLOW: &str = "checkpoint id overflow; save ignored";
/// 世代があふれるときの固定文。
const GENERATION_OVERFLOW: &str = "generation overflow; restore ignored";
/// 写しに現在の Step が無いときの固定文。
const MISSING_CURRENT: &str = "checkpoint has no current step; restore ignored";

/// `Control::Save` の分岐。成功なら写しを控え、出たイベント列を返す。
pub(crate) fn on_save(state: &mut SessionState) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    // `at` は操作前の状態から作る（外から来た値は使わない）。
    let at = Stamp {
        generation: state.run.generation,
        step: current.clone(),
    };
    let Some(next_id) = state.next_checkpoint.0.checked_add(1) else {
        return vec![Event::Warning {
            at,
            message: ID_OVERFLOW.to_string(),
        }];
    };
    let id = state.next_checkpoint;
    // `projection` と同じく、ログの長さは飽和させる（panic しない）。
    let log_len = u32::try_from(state.log.len()).unwrap_or(u32::MAX);
    state.checkpoints.insert(
        id,
        Checkpoint {
            run: state.run.clone(),
            entry: state.entry.clone(),
            log_len,
            owner: Some(current.root()),
        },
    );
    state.next_checkpoint = CheckpointId(next_id);
    vec![Event::CheckpointSaved { at, id, log_len }]
}

/// `Control::Restore` の分岐。成功なら写しへ戻り、出たイベント列を返す。
pub(crate) fn on_restore(state: &mut SessionState, id: CheckpointId) -> Vec<Event> {
    let Some(current) = state.run.current.clone() else {
        return Vec::new();
    };
    let at = Stamp {
        generation: state.run.generation,
        step: current,
    };
    let Some(cp) = state.checkpoints.get(&id) else {
        return vec![Event::Refused {
            at,
            op: ControlKind::Restore,
            reason: RefuseReason::UnknownCheckpoint,
            rejection: None,
        }];
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
    let Some(owner) = cp.run.current.clone() else {
        return warn(MISSING_CURRENT);
    };
    let log_len = cp.log_len;
    let entry = cp.entry.clone();
    let mut run = cp.run.clone();
    run.generation = next;
    state.run = run;
    state.entry = entry;
    vec![Event::Restored {
        at,
        id,
        new_generation: next,
        log_len,
        owner,
    }]
}
