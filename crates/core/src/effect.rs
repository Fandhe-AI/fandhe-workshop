//! 状態変化の語彙（要素・属性・フラグ）の適用（TASK-43 1/2）。
//!
//! 型は contract の [`fandhe_workshop_contract::effect`] が持ち、本モジュールは [`RunState`] を変える
//! 純関数だけを持つ。呼び出し元は、後続のタスクで `Session::apply` の受付・達成・途中開始の分岐
//! （TASK-207・TASK-215・TASK-217）になる。[`apply_effect`] は 1 つの語を適用する。[`apply_effects`] は
//! 項目の列を順に適用し、`when` を [`ConditionEval`] で評価して飛ばす（TASK-43 2/2）。`when` を無視して
//! 列を流す関数は、条件つきの項目を無条件に適用してしまうため作らない。条件の語彙の評価は
//! [`crate::condition`]（TASK-42）が持ち、ここでは評価器を注入する。[`apply_tick`] は進行 1 つを適用する。
//!
//! 操作 1 件の `Changed` の集約（経路ごとに 1 件・変更前は操作前の値）は、[`crate::session::apply`] が
//! [`crate::changed`] で行う。本モジュールの `Changed` は語 1 つごとで、集約の際の宛先の出所になる。
//!
//! 不変条件: 全域（panic しない）で決定的。I/O・時刻・乱数を使わない。`Changed` は 1 経路につき 1 件（EVT-13）。
//! 不在の対象への `remove`・`set_attr` は状態を変えず [`Event::Warning`] に倒す。`Warning` の文面には経路・
//! 属性名を入れてよいが、属性値は入れない。
//!
//! `path` は世界状態の鍵で、ファイルパス・URL ではない。値は教材由来で、HTML への埋め込みの
//! エスケープは web 層の責務である。

use fandhe_workshop_contract::effect::{Effect, EffectItem, ProcessingTick, PutEffect};
use serde_json::Value;

use crate::addr::{Root, Stamp};
use crate::event::{Cause, Event};
use crate::session::RunState;
use crate::world::Element;

/// 教材全体のフラグの範囲を示す `Event::Flag::scope`。空文字列で、根の ID（`main`・`<モジュール>@<実体>`）と
/// 衝突しない。
pub const GLOBAL_FLAG_SCOPE: &str = "";

fn changed(
    at: &Stamp,
    path: &str,
    before: Option<Element>,
    after: Option<Element>,
    cause: Cause,
) -> Event {
    Event::Changed {
        at: at.clone(),
        path: path.to_string(),
        before,
        after,
        cause,
    }
}

fn put(run: &mut RunState, p: &PutEffect, at: &Stamp, cause: Cause) -> Vec<Event> {
    let new = Element {
        fixture: p.fixture.clone(),
        attrs: p.attrs.clone(),
    };
    let before = run.world.insert(p.path.clone(), new.clone());
    vec![changed(at, &p.path, before, Some(new), cause)]
}

/// 1 つの語の効果を `run` に適用し、出たイベントを返す。
///
/// `root` はフラグを持つ根（`set_flag`・`clear_flag` の範囲）。`set_flag_global` は `root` に依らない。
/// `replace` は `put` と同じ挙動で、対象が無ければ置く。
pub fn apply_effect(
    run: &mut RunState,
    root: &Root,
    effect: &Effect,
    at: &Stamp,
    cause: Cause,
) -> Vec<Event> {
    let flag = |scope: &str, name: &str, on: bool| Event::Flag {
        at: at.clone(),
        scope: scope.to_string(),
        name: name.to_string(),
        on,
    };
    match effect {
        Effect::Put(p) | Effect::Replace(p) => put(run, p, at, cause),
        Effect::Remove(path) => match run.world.remove(path) {
            Some(old) => vec![changed(at, path, Some(old), None, cause)],
            None => vec![Event::Warning {
                at: at.clone(),
                message: format!("remove: no element at path `{path}`"),
            }],
        },
        Effect::SetAttr(s) => match run.world.get_mut(&s.path) {
            Some(el) => {
                let before = el.clone();
                el.attrs.insert(s.attr.clone(), s.value.clone());
                let after = el.clone();
                vec![changed(at, &s.path, Some(before), Some(after), cause)]
            }
            None => vec![Event::Warning {
                at: at.clone(),
                message: format!("set_attr: no element at path `{}`", s.path),
            }],
        },
        Effect::SetFlag(name) => {
            run.slices
                .entry(root.clone())
                .or_default()
                .flags
                .insert(name.clone());
            vec![flag(&root.0, name, true)]
        }
        Effect::ClearFlag(name) => {
            if let Some(slice) = run.slices.get_mut(root) {
                slice.flags.remove(name);
            }
            vec![flag(&root.0, name, false)]
        }
        Effect::SetFlagGlobal(name) => {
            run.global_flags.insert(name.clone());
            vec![flag(GLOBAL_FLAG_SCOPE, name, true)]
        }
        // `Effect` は非網羅の列挙。未知の語は状態を変えず診断に倒す。
        _ => vec![Event::Warning {
            at: at.clone(),
            message: "unsupported effect word".to_string(),
        }],
    }
}

/// `when` の評価器。実体は [`crate::condition::VocabConditions`]（TASK-42）。純関数であること（I/O・時刻・乱数を持たない）。
pub trait ConditionEval {
    /// `Some(true)` で適用、`Some(false)` で飛ばす。`None` は評価できない（未知の語など）。
    fn eval(&self, when: &Value, run: &RunState, root: &Root) -> Option<bool>;
}

/// 条件の語彙が入るまでの既定。常に `None` を返すため、`when` つきの項目は適用されない。
#[derive(Debug, Clone, Copy, Default)]
pub struct NoConditions;

impl ConditionEval for NoConditions {
    fn eval(&self, _when: &Value, _run: &RunState, _root: &Root) -> Option<bool> {
        None
    }
}

/// 効果の列を上から順に適用し、出たイベントを返す。
///
/// `when` が無い項目は無条件に適用する。`when` は、その項目の直前までを適用した後の状態で評価する
/// （同じ列の先の `set_flag` を後の項目の `when` が見られる）。評価できない（`None`）項目は、真とみなさず
/// 適用しない側に倒し、[`Event::Warning`] を 1 件出す。文面は添字だけで、条件の値は入れない。
pub fn apply_effects(
    run: &mut RunState,
    root: &Root,
    items: &[EffectItem],
    at: &Stamp,
    cause: Cause,
    cond: &dyn ConditionEval,
) -> Vec<Event> {
    let mut events = Vec::new();
    for (i, item) in items.iter().enumerate() {
        if let Some(when) = &item.when {
            match cond.eval(when, run, root) {
                Some(true) => {}
                Some(false) => continue,
                None => {
                    events.push(Event::Warning {
                        at: at.clone(),
                        message: format!("effects[{i}]: `when` could not be evaluated; skipped"),
                    });
                    continue;
                }
            }
        }
        events.extend(apply_effect(run, root, &item.effect, at, cause));
    }
    events
}

/// 進行 1 つを適用する。先に [`Event::Progress`] を出し、続けて効果の列（[`apply_effects`]）のイベントを並べる。
///
/// `index` は、いま適用した進行の 0 始まりの番号（`Event::Progress::index` と同じ）。`total` は進行の総数。
/// `run.busy` は変えない。宛先の照合・`busy` の進め方・`Op::Tick` の分岐は `tick` モジュール（TASK-210 2/3）、
/// `wait` の受付は Step に入るときの処理を持つタスクの責務である。`name` は教材由来の表示名で、HTML への埋め込みのエスケープは web 層の責務である。
#[allow(
    clippy::too_many_arguments,
    reason = "apply_effect と同じ並びの引数（状態・根・宛先・原因）に評価器を足した公開 API。束ねる型は TASK-210 で判断する"
)]
pub fn apply_tick(
    run: &mut RunState,
    root: &Root,
    tick: &ProcessingTick,
    index: u32,
    total: u32,
    at: &Stamp,
    cause: Cause,
    cond: &dyn ConditionEval,
) -> Vec<Event> {
    let mut events = vec![Event::Progress {
        at: at.clone(),
        name: tick.name.clone(),
        index,
        total,
    }];
    events.extend(apply_effects(run, root, &tick.effects, at, cause, cond));
    events
}
