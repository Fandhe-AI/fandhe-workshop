//! 進行イベントの列から、途中の表示を省いて結果だけを導く（EVT-19・A11Y-29・D-82。TASK-230 1/2。#981 の決定）。
//!
//! 動きを抑える設定（`prefers-reduced-motion: reduce`）の画面は、処理中の段階を出さず結果だけを出す。そのための導出を
//! 進行イベントの列から提供する純関数である。状態（`SessionState`）は変えず、受け取ったイベント列の写しだけを作るので、
//! 最終の状態は動きを抑えない場合と同じになる（状態はイベントからではなく操作の適用で決まる）。
//! 呼び出し元は web の画面（TASK-230 2/2）を想定する。
//!
//! # 規則
//!
//! 1. `Progress`（段階の表示）を除く
//! 2. 同じ時点（世代と Step。`Stamp`）の `Changed` が続く区間の中で、同じ経路の `Changed` を 1 件にまとめる。置き場は最初の
//!    1 件の位置、変更前は最初の 1 件の値、変更後と原因は最後の 1 件の値にする。まとめた結果の前後が同じなら除く（途中で戻った変化）。
//!    別の時点の `Changed` が現れたら区間を切る（同じ世代で別の Step を挟んで戻ったとき、間の Step の変化を飛び越えてまとめない）。
//!    新しい操作の始まり（受付になった入力の `InputReceived`・`Stopped`・`Moved`・`Begun`）と、チェックポイントの保存・復帰（`CheckpointSaved`・
//!    `Restored`）でも区間を切る（入力を挟んで同じ Step に戻ったときや、保存の時点の値を、後の変化と混ぜない）。処理の中の
//!    達成・遷移・ヒント（`Achieved`・`Transitioned`・`Entered`・`HintOpened` など）では切らない
//! 3. それ以外のイベントは順のまま残す
//!
//! 時点が違えば（停止で世代が進む・別の Step に移る）まとめない。停止の巻き戻しの `Changed` を、その前の処理の変化と混ぜないため。
//!
//! 返す列は表示用で、元の記録（`SessionState.log`）との位置の対応を持たない（`Restored.log_len` などは元の記録の位置を指したまま）。
//! 位置で索引する導出（`projection::Projection` など）には元の記録を渡し、この関数は表示の直前にだけ使う。
//! 不変条件: 全域で決定的。I/O・時刻・乱数を持たない。`unwrap`・`expect`・`panic!` を使わない。

use std::collections::BTreeMap;

use crate::addr::{Generation, StepUid};
use crate::event::Event;

/// 位置 `at` の `InputReceived` の操作が、次の `InputReceived` までに受付（`Accepted`）を出すか。
fn accepted_before_next_input(events: &[Event], at: usize) -> bool {
    events
        .iter()
        .skip(at.saturating_add(1))
        .take_while(|e| !matches!(e, Event::InputReceived { .. }))
        .any(|e| matches!(e, Event::Accepted { .. }))
}

/// 結果だけのイベント列を返す（規則はモジュールの説明）。入力は変えない。
#[must_use]
pub fn result_only(events: &[Event]) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    // 経路から、まとめ先の `out` の添字（いまの区間の中だけ）。
    let mut slot: BTreeMap<String, usize> = BTreeMap::new();
    // いまの区間の時点（世代と Step）。別の時点の `Changed` か、区間を切るイベントが来たら区間を切る。
    let mut segment: Option<(Generation, StepUid)> = None;
    for (i, e) in events.iter().enumerate() {
        match e {
            Event::Progress { .. } => {}
            // 入力の操作は、受付（`Accepted`）になったときだけ新しい処理の始まりとして区間を切る。処理中に断られた入力は
            // 処理を止めないので切らない。
            Event::InputReceived { .. } if !accepted_before_next_input(events, i) => {
                out.push(e.clone());
            }
            Event::Changed {
                at,
                path,
                after,
                cause,
                ..
            } => {
                let here = (at.generation, at.step.clone());
                if segment.as_ref() != Some(&here) {
                    slot.clear();
                    segment = Some(here);
                }
                if let Some(Event::Changed {
                    after: a, cause: c, ..
                }) = slot.get(path).and_then(|&i| out.get_mut(i))
                {
                    a.clone_from(after);
                    *c = *cause;
                } else {
                    slot.insert(path.clone(), out.len());
                    out.push(e.clone());
                }
            }
            // 新しい操作の始まり（受講者の入力・停止・移動・周回の開始）と、保存・復帰は区間を切る。
            // 世界状態の変化は操作の末尾にまとめて記録される（`changed::record_changes`）ので、受付の操作の変化と
            // 続く進行の変化は、間に入力が無ければ 1 つの区間に入る。処理の中の達成・遷移・ヒントは切らない
            // （`confirm: none` の最後の進行は `Achieved` と遷移の後に変化を記録する）。
            Event::InputReceived { .. }
            | Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Begun { .. }
            | Event::CheckpointSaved { .. }
            | Event::Restored { .. } => {
                slot.clear();
                segment = None;
                out.push(e.clone());
            }
            other => out.push(other.clone()),
        }
    }
    out.retain(|e| !matches!(e, Event::Changed { before, after, .. } if before == after));
    out
}
