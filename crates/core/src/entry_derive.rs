//! 処理中の Step の入口の写し（受付の時点の実行状態）を、ログから導く（SAFE-35・EVT-9）。
//!
//! 入口の写しは状態（`SessionState.entry`）に書かない。書くと、保存した履歴の写し（`snapshot`）の形が変わり、
//! 同じ版の履歴が適用し直しの照合で開けなくなる（VER-2・VER-3。`history_replay::first_difference` は `entry` も
//! 比べる）。そこで、写しが要る場面でだけ、いまの `run` を起点にログを末尾から逆にたどり、受付の時点の `run` を
//! 組み立て直す。呼び出し元は、処理中の停止（[`crate::stop`]）と、上限で止めた後の戻り方
//! （[`crate::transit::return_after_limit`]）だけで、公開 API には含めない（契約の対象外）。
//! チェックポイントの保存と復帰は写しを導かない。保存は `SessionState.entry` をそのまま写し、復帰の後は
//! ログの `Restored` からチェックポイントの `run` へ起点を移して導く（下の規則 3）。
//! 復習・変化の印の側（`projection`）は、同じ受付の時点をイベント列だけで決め、ここと同じ時点へ戻す。
//!
//! # 導き方
//!
//! 起点は「ログの位置 `P` と、その位置での `run`」の組で、最初は（ログの末尾, いまの `run`）。ログを `P` から
//! 前へ 1 件ずつ見て、次の規則で受付の時点を探す。
//!
//! 1. `Changed` は、変更前の値を控える（後で戻す）。`apply` は操作 1 件につき経路ごと 1 件・変更前は操作前の値で
//!    記録する（EVT-13）ので、新しい方から順に変更前の値を当てると、受付の直前の世界状態に一意に戻る
//! 2. 処理中に起きても実行状態を変えないか、世代だけを変えるイベント（`Progress`・`Flag`・`Warning`・`Ignored`・
//!    `InputReceived`・`Judged`・`Refused`・`CheckpointSaved`・`Reloaded`）は読み飛ばす。処理中に拒否された
//!    再送は `InputReceived` と `Judged` だけなので、受付の時点を動かさない
//! 3. `Restored` は、そのチェックポイントの `run` で起点を置き換え、位置を保存の時点（`log_len`）へ移す。
//!    復帰は `run` を写しへ丸ごと戻すので、それより後の差分は捨てる。写しが無い・`log_len` が控えと合わない・
//!    いまの位置より前を指さない・写しが同じ Step の処理中でない、のどれかなら導かない
//! 4. 現在の Step の `Accepted` に当たったら、そこが受付の時点である。控えた変更前の値を世界状態へ当て、Step の
//!    状態を `Entered`、`busy` を空にする。受付は `Entered` で処理中でない Step でだけ起きる（`accept` の
//!    `Phase::AwaitInput`）ので、この 2 つは一意に決まる。写しの `log_len` は受付の操作の先頭（`InputReceived`）の位置
//! 5. それ以外のイベント（Step の出入り・達成・遷移・停止・移動・上限での停止・別の Step の受付・未知のもの）に
//!    当たる、ログの先頭まで受付が無い、のどちらかなら、受付の時点を一意に決められないので導かない（`None`）
//!
//! 導けないときの扱いは呼び出し元が決め、どちらも取り消しをしない側に倒す。停止は `Warning` だけで状態を変えず、
//! 上限で止めた後は操作の前の `run` から `busy` だけを空にして戻す（現在の Step は残る）。
//!
//! # 戻さない欄
//!
//! フラグ（`Slice::flags`・`global_flags`）と、選択の入力で選んだ選択肢（`Slice::choices`）は、受付の時点の値へ
//! 戻さず、起点の値のまま残す。`Event::Flag` は変更前の値を持たず、選択肢の上書きはイベントに変更前を残さない
//! ため、ログだけでは受付の前の値を一意に決められない。どちらも受付・進行の効果で同じ値を置き直す（冪等）ので、
//! 同じ規則で受け付け直したときの効果は二重にならない。案内レベル・章の完了は処理中に変わらない。
//!
//! # 計算量
//!
//! 1 回の導出で見るイベントは、ログの長さ以下である（位置は単調に減り、`Restored` の移動も必ず前へ進む）。
//! 戻す差分も、見た `Changed` の数以下である。導出は処理中の停止と処理中の上限停止の操作でだけ、1 操作につき
//! 高々 1 回行う（受付・進行のたびには行わない）ので、操作の列全体を適用し直しても 2 乗にはならない。
//! ログの長さは履歴の読み込み口の上限（`history_size`）で抑えられている。
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 教材（IR）を読まない。ログ・チェックポイントは保存から読んだ信頼できない値でありうるため、食い違いは
//! 導かない側に倒す（A04）。

use std::collections::BTreeMap;

use crate::event::Event;
use crate::input::CheckpointId;
use crate::session::{Checkpoint, EntryCopy, RunState, SessionState};
use crate::step_status::StepStatus;

/// 現在の Step の入口の写しを返す。状態に写しがあればそれを、無ければログから導いたものを使う。
///
/// 状態の写し（`SessionState.entry`）は、写しを持つチェックポイントへの復帰でだけ値が入る（通常の操作では
/// 空のまま）。現在の Step のものであれば、従来どおりそれを優先する。
pub(crate) fn entry_for_current(state: &SessionState) -> Option<EntryCopy> {
    stored_or_derived(
        &state.run,
        state.entry.as_ref(),
        &state.log,
        &state.checkpoints,
    )
}

/// `run` の時点の入口の写し。`stored` が `run` の現在の Step のものならそれを、そうでなければ導いたものを返す。
///
/// `log` と `checkpoints` は `run` の時点のもの（`apply` の中では、操作を適用する前のログ）を渡す。
pub(crate) fn stored_or_derived(
    run: &RunState,
    stored: Option<&EntryCopy>,
    log: &[Event],
    checkpoints: &BTreeMap<CheckpointId, Checkpoint>,
) -> Option<EntryCopy> {
    match stored.filter(|e| run.current.is_some() && e.run.current == run.current) {
        Some(entry) => Some(entry.clone()),
        None => derive(run, log, checkpoints),
    }
}

/// ログを末尾から逆にたどり、処理中の現在の Step の受付の時点の `run` を組み立てる（モジュールの規則 1〜5）。
///
/// 処理中でない・現在の Step が無いときは `None`（写しが要らない）。受付の時点を一意に決められないときも `None`。
pub(crate) fn derive(
    run: &RunState,
    log: &[Event],
    checkpoints: &BTreeMap<CheckpointId, Checkpoint>,
) -> Option<EntryCopy> {
    let current = run.current.as_ref()?;
    run.busy.as_ref()?;
    let mut base = run.clone();
    // 控えた変更前の値（新しいものから順）。
    let mut undo: Vec<(&str, &Option<crate::world::Element>)> = Vec::new();
    let mut at = log.len();
    while at > 0 {
        at -= 1;
        match log.get(at)? {
            Event::Changed { path, before, .. } => undo.push((path.as_str(), before)),
            Event::Progress { .. }
            | Event::Flag { .. }
            | Event::Warning { .. }
            | Event::Ignored { .. }
            | Event::InputReceived { .. }
            | Event::Judged { .. }
            | Event::Refused { .. }
            | Event::CheckpointSaved { .. }
            | Event::Reloaded { .. } => {}
            Event::Restored { id, log_len, .. } => {
                let cp = checkpoints.get(id)?;
                let saved_at = usize::try_from(*log_len).ok().filter(|p| *p < at)?;
                if cp.log_len != *log_len
                    || cp.run.current.as_ref() != Some(current)
                    || cp.run.busy.is_none()
                {
                    return None;
                }
                base = cp.run.clone();
                undo.clear();
                // 保存の時点の `CheckpointSaved`（位置 `saved_at`）より前から続ける。
                at = saved_at;
            }
            Event::Accepted { at: stamp } if &stamp.step == current => {
                let start = op_start(log, at)?;
                for (path, before) in undo {
                    match before {
                        Some(el) => {
                            base.world.insert(path.to_string(), el.clone());
                        }
                        None => {
                            base.world.remove(path);
                        }
                    }
                }
                // 処理中（`busy`）の Step は呼び出しの Step にならないので、`owner_of` でなく `root_of` で足りる。
                base.slices
                    .get_mut(&crate::instance::root_of(current))?
                    .status
                    .insert(current.clone(), StepStatus::Entered);
                base.busy = None;
                return Some(EntryCopy {
                    run: base,
                    log_len: u32::try_from(start).unwrap_or(u32::MAX),
                });
            }
            _ => return None,
        }
    }
    None
}

/// 位置 `accepted` の `Accepted` を出した受付の操作の先頭（`InputReceived`）の位置。
///
/// 受付のイベントの順は `InputReceived` → `Judged` → `Accepted`（`accept` のモジュール文書）。間に `Judged` 以外が
/// あれば、受付の操作の形ではないので `None`。
fn op_start(log: &[Event], accepted: usize) -> Option<usize> {
    let mut at = accepted;
    while at > 0 {
        at -= 1;
        match log.get(at)? {
            Event::Judged { .. } => {}
            Event::InputReceived { .. } => return Some(at),
            _ => break,
        }
    }
    None
}
