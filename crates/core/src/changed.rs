//! 操作 1 件の世界状態の変化を、変更前の値つきの `Changed` として記録する（TASK-215 1/2、EVT-13）。
//!
//! 呼び出し元は [`crate::session::apply`] で、操作の前後の世界状態の差分から経路ごとに 1 件の
//! [`Event::Changed`] を作る。停止・移動・復帰・開始は世界状態を丸ごと戻すため、語を 1 つずつ適用する
//! [`crate::effect`] だけでは変更前の値を持つ `Changed` を作れない。差分から作ることで、どの分岐が
//! 世界状態を変えても、経路ごとに高々 1 件・変更前は操作前の値、という性質を [`crate::session::apply`] が一元的に保つ。
//! [`replay_changes`] は、イベント列だけから世界状態を組み立て直す（性質 P-1b）。復習の前後比較
//! （チェックポイントの写しに依らない導出）の土台で、導出そのものは TASK-215 2/2 が持つ。
//!
//! 本モジュールは [`crate::CONTRACT_VERSION`] の対象に含めない（[`crate::effect`] と同様）。
//!
//! 不変条件: 全域（panic しない）で決定的。I/O・時刻・乱数を使わず、[`Event::Warning`] を出さない。
//! `path` は世界状態の鍵で、ファイルパス・URL ではない。値は教材・利用者由来で、HTML への埋め込みの
//! エスケープは web 層の責務である。

use std::collections::BTreeMap;

use crate::addr::{Stamp, StepUid};
use crate::event::{Cause, Event};
use crate::input::{Control, Op};
use crate::world::{Element, World};

/// 経路・変更前・変更後の組。
pub type PathChange = (String, Option<Element>, Option<Element>);

/// 2 つの世界状態の差分を、経路の辞書順で返す。値が異なる経路だけを含む。
pub fn world_diff(before: &World, after: &World) -> Vec<PathChange> {
    let mut out = Vec::new();
    // BTreeMap は鍵の昇順なので、両方を併合して走査すれば辞書順になる。
    let mut keys: Vec<&String> = before.keys().chain(after.keys()).collect();
    keys.sort();
    keys.dedup();
    for key in keys {
        let b = before.get(key);
        let a = after.get(key);
        if b != a {
            out.push((key.clone(), b.cloned(), a.cloned()));
        }
    }
    out
}

/// 操作から `Changed` の原因を決める。
///
/// `Save`・`OpenHint`・`SetGuidance` は世界状態を変えず差分が空になるため、原因は使われない（`Input` を返す）。
/// 処理中でない停止を取消として扱う場合の原因の細分は、停止の担当（TASK-211）が見直す。
pub fn cause_of(op: &Op) -> Cause {
    match op {
        Op::Input(_) | Op::Tick(_) => Cause::Input,
        Op::Control(Control::Stop) => Cause::Stop,
        Op::Control(Control::GoTo(_)) => Cause::Move,
        Op::Control(Control::Restore(_)) => Cause::Restore,
        Op::Control(Control::Begin(_)) => Cause::Start,
        Op::Control(_) => Cause::Input,
    }
}

fn changed_path(event: &Event) -> Option<(&str, &Stamp)> {
    match event {
        Event::Changed { path, at, .. } => Some((path.as_str(), at)),
        _ => None,
    }
}

/// 操作 1 件のイベント列 `events` の語レベルの `Changed` を、前後の差分から作った `Changed` に置き換える。
///
/// 語レベルの `Changed` は取り除き、経路ごとの宛先（`at`）の出所としてだけ使う（同じ経路は後勝ち）。
/// 差分の経路ごとの `Changed` は、辞書順でイベント列の末尾に足す。宛先が無い経路は `default_at` を使う。
/// 他のイベントの相対順は変えない。宛先の決め方は初版で、各分岐のタスクが見直してよい。
pub fn record_changes(
    events: Vec<Event>,
    before: &World,
    after: &World,
    default_at: &Stamp,
    cause: Cause,
) -> Vec<Event> {
    let mut at_of: BTreeMap<String, Stamp> = BTreeMap::new();
    let mut out = Vec::with_capacity(events.len());
    for event in events {
        if let Some((path, at)) = changed_path(&event) {
            at_of.insert(path.to_string(), at.clone());
        } else {
            out.push(event);
        }
    }
    for (path, b, a) in world_diff(before, after) {
        let at = at_of.get(&path).unwrap_or(default_at).clone();
        out.push(Event::Changed {
            at,
            path,
            before: b,
            after: a,
            cause,
        });
    }
    out
}

/// `Changed` の `after` を順に当てて世界状態を組み立て直す（P-1b）。`None` は削除。
/// `Changed` 以外のイベントは無視する。
pub fn replay_changes(world: &mut World, events: &[Event]) {
    for event in events {
        if let Event::Changed { path, after, .. } = event {
            match after {
                Some(el) => {
                    world.insert(path.clone(), el.clone());
                }
                None => {
                    world.remove(path);
                }
            }
        }
    }
}

/// 宛先の既定値。操作後の世代と現在の Step（無ければ操作前の Step）を使う。
///
/// どちらも無いのは世界状態が変わらない場合だけのはずだが、全域性のため空の Step に倒す
/// （記録を落とさない側に倒し、P-1b を壊さない）。
pub fn default_stamp(
    generation: crate::addr::Generation,
    current_after: Option<&StepUid>,
    current_before: Option<&StepUid>,
) -> Stamp {
    Stamp {
        generation,
        step: current_after
            .or(current_before)
            .cloned()
            .unwrap_or_else(|| StepUid(String::new())),
    }
}
