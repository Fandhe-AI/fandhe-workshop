//! 状態変化の語彙（要素・属性・フラグ）の適用（TASK-43 1/2）。
//!
//! 型は contract の [`fandhe_workshop_contract::effect`] が持ち、本モジュールは [`RunState`] を変える
//! 純関数だけを持つ。呼び出し元は、後続のタスクで `Session::apply` の受付・達成・途中開始の分岐
//! （TASK-207・TASK-215・TASK-217）と、`when` の評価と項目の列の順次適用（TASK-43 2/2）になる。
//! 本件は 1 つの語だけを適用し、`when` は評価しない。`when` を無視して列を流す関数は、条件つきの項目を
//! 無条件に適用してしまうため作らない。
//!
//! 不変条件: 全域（panic しない）で決定的。I/O・時刻・乱数を使わない。`Changed` は 1 経路につき 1 件（EVT-13）。
//! 不在の対象への `remove`・`set_attr` は状態を変えず [`Event::Warning`] に倒す。`Warning` の文面には経路・
//! 属性名を入れてよいが、属性値は入れない。
//!
//! `path` は世界状態の鍵で、ファイルパス・URL ではない。値は教材由来で、HTML への埋め込みの
//! エスケープは web 層の責務である。

use fandhe_workshop_contract::effect::{Effect, PutEffect};

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
