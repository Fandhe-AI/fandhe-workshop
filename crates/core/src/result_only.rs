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
//!    別の時点の `Changed` が現れたら区間を切る（同じ世代で別の Step を挟んで戻ったとき、間の Step の変化を飛び越えてまとめない）
//! 3. それ以外のイベントは順のまま残す
//!
//! 時点が違えば（停止で世代が進む・別の Step に移る）まとめない。停止の巻き戻しの `Changed` を、その前の処理の変化と混ぜないため。
//! 不変条件: 全域で決定的。I/O・時刻・乱数を持たない。`unwrap`・`expect`・`panic!` を使わない。

use std::collections::BTreeMap;

use crate::addr::{Generation, StepUid};
use crate::event::Event;

/// 結果だけのイベント列を返す（規則はモジュールの説明）。入力は変えない。
#[must_use]
pub fn result_only(events: &[Event]) -> Vec<Event> {
    let mut out: Vec<Option<Event>> = Vec::with_capacity(events.len());
    // (時点, 経路) から、まとめ先の `out` の添字。
    let mut slot: BTreeMap<String, usize> = BTreeMap::new();
    // いまの区間の時点（世代と Step）。別の時点の `Changed` が来たら区間を切る。
    let mut segment: Option<(Generation, StepUid)> = None;
    for e in events {
        match e {
            Event::Progress { .. } => {}
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
                let key = path.clone();
                if let Some(Some(Event::Changed {
                    after: a, cause: c, ..
                })) = slot.get(&key).and_then(|&i| out.get_mut(i))
                {
                    a.clone_from(after);
                    *c = *cause;
                } else {
                    slot.insert(key, out.len());
                    out.push(Some(e.clone()));
                }
            }
            other => out.push(Some(other.clone())),
        }
    }
    out.into_iter()
        .flatten()
        .filter(|e| !matches!(e, Event::Changed { before, after, .. } if before == after))
        .collect()
}
