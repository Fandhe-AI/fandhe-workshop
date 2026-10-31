//! チェックポイントの保存 `Control::Save` と復帰 `Control::Restore` の分岐（TASK-214 1/3、EVT-12）。
//!
//! 学習者が途中の状態を控え、先へ進んだ後にその時点へ戻れるようにする。保存は世代を変えず、復帰は成功した
//! ときだけ世代を 1 つ進める。世代を進めるのは、復帰前に予約された進行を古い宛先として無視させるためである
//! （無視と処理中の再開の試験は `tests/restore_resume.rs`、TASK-214 3/3）。呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない
//! （契約の対象外）。世代の規則の表（`Keep`・`IncrementOnSuccess`）の強制は `apply` の関門が行い、
//! 本モジュールは表を再実装しない。教材（IR）から読むのは実体の呼び出し元の表（持ち主の根の決定）だけで、保存・復帰の意味を教材が変えることはできない。
//!
//! # 保存の規則（上から評価し、最初に当たったものを返す）
//!
//! 1. 現在の Step が無いなら、状態を変えず空のイベント列を返す（`Stamp` を作れない。開始前の扱いは TASK-217）。
//!    これにより保存した `run.current` は必ず `Some` になり、復帰の `Restored.owner` を作れることが保証される
//! 2. 次の ID があふれるなら、固定文の `Warning` だけを出して状態を変えない（fail-closed）
//! 3. 実行状態と入口の写しを写し取り、`CheckpointSaved` を 1 件返す。持ち主の根は `owner_of`（呼び出しの Step なら呼ばれた実体の根）で決める。`log_len` は保存操作の直前のログの長さ
//!    （`CheckpointSaved` 自身のログ上の位置）で、`Projection` が復習を控える位置と一致する
//!
//! # 章の終わりの自動保存（SAVE-31、TASK-339 1/2）
//!
//! `apply` が、章の終わりを含む操作（[`crate::chapter_end`]。上限で止めた操作を除く）のイベント列の末尾に
//! 保存を 1 件足す。手動の保存と同じ通し番号を使い、写しは操作の後の状態。`log_len` は `CheckpointSaved` 自身の
//! ログ上の位置。`ops` には記録されず、`ops` の適用し直しで同じ保存が再現される。
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
//! 5. 写しの `log_len` が現在のログの長さを超えるなら、固定文の `Warning` だけで状態を変えない（fail-closed。
//!    戻す経路の集合を決められないまま一部だけ戻さない。保存から読んだ履歴は再生で照合されるので、届くのは手で組んだ状態だけ）
//! 6. 持ち主の根（保存時の `Checkpoint::owner`。無ければ `owner_of` で写しの現在の Step から導く）の切り身だけを写しへ戻し（写しに無ければ
//!    取り除く）、他の根の切り身は復帰の前のまま残す。`current`・`busy`・`completed` は写しの値、世代は現在の世代 +1
//!    にする（世代は減らない）。案内レベルは写しの値に戻す。`global_flags` は戻さない（復帰の直前のまま）。
//!    世界状態は、保存の後のログ（`log_len` 以降）の `Changed` のうち宛先の持ち主が復帰の持ち主と一致するものの経路
//!    だけを写しの値へ戻し（写しに無ければ取り除く）、他の経路は復帰の前のまま残す。原因（`Cause`）は問わない。
//!    保存の後に持ち主と他の根の両方が書いた経路は戻る（世界状態は教材全体で 1 つで、持ち主の書き込みを優先する）。
//!    宛先の無い（空の Step の）`Changed` は持ち主と一致しないので戻さない。根が 1 つの教材では全体を戻すのと結果が一致する。
//!    入口の写しも保存時点のものへ戻す（移動で無効化された写しを戻さないと、復帰後の停止が写し無しで倒れる）。
//!    写しの中の世代は書き換えない（停止が戻すときに世代を上書きするので値は使われない）。
//!    `Restored` を 1 件返す。`log_len` は写しの値で、現在のログの長さではない（EVT-14）
//!
//! `Changed` は作らない。`apply` が前後の世界状態の差から `Cause::Restore` で作る。ログ（追記のみ）・
//! 下書き・ヒント・チェックポイント（同じ ID へ何度でも戻れる）・次の ID には触れない。
//! `log_len` の範囲は規則 5 で検査する（現在のログの長さを超える位置は `Warning` だけで状態を変えない fail-closed）。
//! `Projection` は同じ状況で写し無しに倒す（fail-soft）。
//!
//! # 扱わないこと
//!
//! - 復習・変化の印の導出そのもの（`projection`。`apply()` を通した試験は `tests/restore_review.rs`、TASK-214 2/3）
//! - 復帰前の進行の無視の判定そのもの（`judge`・`tick`。復帰の側からの試験は `tests/restore_resume.rs`、TASK-214 3/3）
//! - 案内レベルの切り替えと開始点の反映（TASK-294）
//! - 開始前の保存・復帰（TASK-217）、保存の数の上限（TASK-339 2/2）
//! - 周回単位の復帰・周回単位のチェックポイント・周回の「完了」の記録（MOD-16。#781 の決定で実体単位だけにした。
//!   同じ周回の別の実体を戻さないことの試験は `crates/pack/tests/round_instance_restore.rs`）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! ID は履歴由来の信頼できない値で、鍵としてだけ使う。`Warning` の文面は固定文で値を埋め込まない。

use fandhe_workshop_contract::ir::Ir;

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
/// 写しのログ位置が現在のログの長さを超えるときの固定文。
const LOG_POSITION_OUT_OF_RANGE: &str = "checkpoint log position is out of range; restore ignored";

/// `Control::Save` の分岐。成功なら写しを控え、出たイベント列を返す。
pub(crate) fn on_save(ir: &Ir, state: &mut SessionState) -> Vec<Event> {
    // `projection` と同じく、ログの長さは飽和させる（panic しない）。
    let log_len = u32::try_from(state.log.len()).unwrap_or(u32::MAX);
    save_at(ir, state, log_len)
}

/// 章の終わりの自動保存（SAVE-31、TASK-339 1/2）。`apply` が `Changed` を足した後、ログへ足す直前に呼ぶ。
/// `log_len` は足す直前のログの長さ + 操作のイベント数（`CheckpointSaved` 自身のログ上の位置）。
/// 手動の保存と同じ通し番号を使う。ID があふれるときは固定文の `Warning` だけで状態を変えない。
pub(crate) fn on_chapter_end_save(ir: &Ir, state: &mut SessionState, log_len: usize) -> Vec<Event> {
    save_at(ir, state, u32::try_from(log_len).unwrap_or(u32::MAX))
}

/// 保存の実体。`log_len` を自身の位置として写しを控える。
fn save_at(ir: &Ir, state: &mut SessionState, log_len: u32) -> Vec<Event> {
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
    state.checkpoints.insert(
        id,
        Checkpoint {
            run: state.run.clone(),
            entry: state.entry.clone(),
            log_len,
            owner: Some(crate::instance::owner_of(ir, &current)),
        },
    );
    state.next_checkpoint = CheckpointId(next_id);
    vec![Event::CheckpointSaved { at, id, log_len }]
}

/// `Control::Restore` の分岐。成功なら写しへ戻り、出たイベント列を返す。
pub(crate) fn on_restore(ir: &Ir, state: &mut SessionState, id: CheckpointId) -> Vec<Event> {
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
    // 位置は履歴由来の信頼できない値。範囲外は一部だけ戻さず、状態を変えない。
    let Some(since) = usize::try_from(log_len)
        .ok()
        .and_then(|n| state.log.get(n..))
    else {
        return warn(LOG_POSITION_OUT_OF_RANGE);
    };
    let entry = cp.entry.clone();
    // 持ち主の根は保存時の記録を使う。旧データ（記録なし）は写しの現在の Step の根に倒す（panic しない）。
    let owner_root = cp
        .owner
        .clone()
        .unwrap_or_else(|| crate::instance::owner_of(ir, &owner));
    // 保存の後に持ち主が書いた世界状態の経路。宛先の持ち主は実体の表から引く。
    let touched = crate::entry_derive::owner_touched_paths(since, &ir.instances, &owner_root);
    // 持ち主の根の切り身・位置・世界状態の経路・案内レベルだけを写しへ戻す。他は復帰の直前のまま（`global_flags` を含む）。
    let mut run = crate::entry_derive::revert_owner(&state.run, &cp.run, &owner_root, &touched);
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
