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
//! 2. 1 回の処理（下の「処理の区間」）の中の `Changed` を、経路ごとに 1 件にまとめる。置き場は最初の 1 件の位置、変更前は
//!    最初の 1 件の値、変更後と原因は最後の 1 件の値にする。まとめた結果の前後が同じなら除く（途中で戻った変化）
//! 3. 区間の外の `Changed` と、それ以外のイベントは、そのまま順に残す
//!
//! # 処理の区間
//!
//! 世界状態の差分は操作ごとに 1 経路 1 件で、操作の末尾にまとめて記録される（`changed::record_changes`）。区間は
//! 「処理を始めた操作」から「最後の進行の操作」までの操作の列で、次で決める。
//!
//! - 始まり: 区間の外で、`Accepted`（受付。入力の操作の受付は、その操作の変化より前に出る）か、`Progress` の 0 番。
//!   受付で始めた区間は、続く操作が進行ならそのまま処理の途中になり、別の操作（入力・停止・移動・開始・復帰・保存・
//!   ヒント・再読み込み・拒否）が始まれば、受付の操作だけで閉じる（処理を持たない受付）
//!   （入った時点から処理中の `wait` の Step。#2205）を見たとき
//! - 終わり: 最後の進行（`Progress` の `index + 1 == total`）を見た後、その操作の末尾の変化が済んだとき。末尾の変化の
//!   後の最初の `Changed` でないイベントで閉じる（最後の進行の操作に変化が無いときは、次の操作の始まりで閉じる）
//! - 途中で閉じる: `Stopped`（停止の巻き戻しは別の変化として残す）・`Moved`・`Begun`・`Restored`・`CheckpointSaved`
//!   （保存の時点の値を、後の変化と混ぜない）。閉じるイベント自身は区間の外に残す
//!
//! 処理の中で起きる、ヒントを開く・処理中に断られた入力・達成と遷移（`confirm: none` の最後の進行）は、区間を閉じない。
//! 処理の後の確認の操作は、最後の進行で区間が閉じた後なので、まとめない。
//!
//! 返す列は表示用で、元の記録（`SessionState.log`）との位置の対応を持たない（`Restored.log_len` などは元の記録の位置を指したまま）。
//! 位置で索引する導出（`projection::Projection` など）には元の記録を渡し、この関数は表示の直前にだけ使う。
//! 不変条件: 全域で決定的。I/O・時刻・乱数を持たない。`unwrap`・`expect`・`panic!` を使わない。

use std::collections::BTreeMap;

use crate::event::Event;

/// 区間の状態。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Segment {
    /// 区間の外。
    Outside,
    /// 受付の操作の中。続く操作が進行なら処理の途中へ、別の操作が始まれば閉じる（処理を持たない受付）。
    Accepted,
    /// 処理の途中。
    Running,
    /// 最後の進行を見た。その操作の末尾の変化を待つ（`seen_change` はその変化を 1 件以上見たか）。
    Ending { seen_change: bool },
}

/// 受付の操作の後に、別の操作の始まりとして現れるイベントか（受付の操作の中の達成・遷移・変化は含まない）。
fn starts_another_operation(e: &Event) -> bool {
    matches!(
        e,
        Event::InputReceived { .. }
            | Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Begun { .. }
            | Event::Restored { .. }
            | Event::CheckpointSaved { .. }
            | Event::HintOpened { .. }
            | Event::Reloaded { .. }
            | Event::Refused { .. }
    )
}

/// 結果だけのイベント列を返す（規則はモジュールの説明）。入力は変えない。
#[must_use]
pub fn result_only(events: &[Event]) -> Vec<Event> {
    let mut out: Vec<Event> = Vec::with_capacity(events.len());
    // 経路から、まとめ先の `out` の添字（いまの区間の中だけ）。
    let mut slot: BTreeMap<String, usize> = BTreeMap::new();
    let mut seg = Segment::Outside;
    for e in events {
        // 処理を持たない受付の操作は、次の操作が始まったところで閉じる（進行が来れば処理の途中へ移る）。
        if seg == Segment::Accepted && starts_another_operation(e) {
            seg = Segment::Outside;
            slot.clear();
        }
        // 最後の進行の後、末尾の変化が済んだら（または変化の無いまま次のイベントが来たら）閉じる。
        if let Segment::Ending { seen_change } = seg {
            let is_change = matches!(e, Event::Changed { .. });
            // 最後の進行の操作の中の受付（`wait` の最後の進行が出す `Accepted`）は、その操作の一部なので閉じない。
            let opens_next = matches!(e, Event::InputReceived { .. } | Event::Progress { .. });
            if (seen_change && !is_change) || (!seen_change && opens_next) {
                seg = Segment::Outside;
                slot.clear();
            }
        }
        match e {
            Event::Progress { index, total, .. } => {
                if seg == Segment::Accepted {
                    seg = Segment::Running;
                }
                if seg == Segment::Outside && *index == 0 {
                    seg = Segment::Running;
                    slot.clear();
                }
                if seg == Segment::Running && index.saturating_add(1) >= *total {
                    seg = Segment::Ending { seen_change: false };
                }
            }
            Event::Accepted { .. } => {
                if seg == Segment::Outside {
                    seg = Segment::Accepted;
                    slot.clear();
                }
                out.push(e.clone());
            }
            Event::Changed {
                path, after, cause, ..
            } => {
                if seg == Segment::Outside {
                    out.push(e.clone());
                    continue;
                }
                if let Segment::Ending { .. } = seg {
                    seg = Segment::Ending { seen_change: true };
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
            Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Begun { .. }
            | Event::Restored { .. }
            | Event::CheckpointSaved { .. } => {
                seg = Segment::Outside;
                slot.clear();
                out.push(e.clone());
            }
            other => out.push(other.clone()),
        }
    }
    out.retain(|e| !matches!(e, Event::Changed { before, after, .. } if before == after));
    out
}
