//! 条件の語彙（状態・世界状態・論理。TASK-42 1/2）の評価。
//!
//! 型と `Value` からの解釈は contract の [`fandhe_workshop_contract::condition`] が持ち、本モジュールは
//! [`RunState`] に対して条件を評価する純関数 [`eval_condition`] と、効果の列の `when` に差し込む評価器
//! [`VocabConditions`]（[`crate::effect::ConditionEval`] の実体）を持つ。呼び出し元は、効果の適用
//! （[`crate::effect::apply_effects`]）と、後続のタスクで遷移・規則・開始条件の分岐（TASK-207・TASK-210・
//! TASK-215・TASK-217）になる。
//!
//! 評価は 3 値である。`Some(true)`・`Some(false)` に加え、評価できない条件（`param`・`clarifying` は
//! TASK-42 2/2 で評価する。解釈に失敗した値もここに入る）は `None` を返す。`all`・`any`・`not` は、評価できない
//! 子を含むと `None` になる（短絡しない）。呼び出し側は `None` を真とみなさず、適用しない側に倒す。
//!
//! 不変条件: 全域（panic しない）で決定的。I/O・時刻・乱数を使わない。再帰の深さは、解釈が断る入れ子の
//! 上限（`MAX_JSON_DEPTH`）に抑えられる。`count` は世界状態の件数に線形である。
//!
//! 経路・属性は世界状態のメモリ上の鍵で、ファイルパス・URL として扱わない。

use fandhe_workshop_contract::condition::{Condition, StepState};
use serde_json::Value;

use crate::addr::{Root, StepUid};
use crate::effect::ConditionEval;
use crate::session::RunState;
use crate::step_status::StepStatus;

/// 契約の状態名を、実行時の `StepStatus` に対応させる。
fn status_of(s: StepState) -> StepStatus {
    match s {
        StepState::NotStarted => StepStatus::NotStarted,
        StepState::Entered => StepStatus::Entered,
        StepState::Accepted => StepStatus::Accepted,
        StepState::Achieved => StepStatus::Achieved,
        StepState::Skipped => StepStatus::Skipped,
    }
}

/// 根の中の Step 名から実体 ID を作る。
fn uid(root: &Root, step: &str) -> StepUid {
    StepUid(format!("{}/{step}", root.0))
}

/// 条件を `run` に対して評価する。`root` は `step`・`flag`・`chose` が見る実体の根。
///
/// `flag` は実体ごとのフラグ（`run.slices[root].flags`）だけを読み、`global_flags` は読まない。
/// `all([])` は真、`any([])` は偽。評価できない語（`param`・`clarifying`）と、それを含む論理は `None`。
pub fn eval_condition(c: &Condition, run: &RunState, root: &Root) -> Option<bool> {
    match c {
        Condition::Step(s) => {
            let now = run
                .slices
                .get(root)
                .and_then(|sl| sl.status.get(&uid(root, &s.step)))
                .copied()
                .unwrap_or(StepStatus::NotStarted);
            Some(now == status_of(s.state))
        }
        Condition::Flag(n) => Some(run.slices.get(root).is_some_and(|sl| sl.flags.contains(n))),
        Condition::WorldHas(p) => Some(run.world.contains_key(p)),
        Condition::WorldAttr(w) => Some(
            run.world
                .get(&w.path)
                .and_then(|el| el.attrs.get(&w.attr))
                .is_some_and(|v| *v == w.eq),
        ),
        Condition::Count(k) => {
            let n = run
                .world
                .range(k.prefix.clone()..)
                .take_while(|(path, _)| path.starts_with(&k.prefix))
                .filter(|(_, el)| {
                    k.where_attrs
                        .iter()
                        .all(|(a, v)| el.attrs.get(a).is_some_and(|x| x == v))
                })
                .count();
            Some(k.op.holds(u64::try_from(n).unwrap_or(u64::MAX), k.n))
        }
        Condition::Chose(ch) => Some(
            run.slices
                .get(root)
                .and_then(|sl| sl.choices.get(&uid(root, &ch.step)))
                .is_some_and(|o| *o == ch.option),
        ),
        Condition::All(list) => fold(list, run, root, true),
        Condition::Any(list) => fold(list, run, root, false),
        Condition::Not(inner) => eval_condition(inner, run, root).map(|b| !b),
        // `param` と `clarifying` の評価は TASK-42 2/2。評価できないものとして安全側に倒す。
        _ => None,
    }
}

/// `all`（`all_of` が真）または `any`（偽）の畳み込み。短絡せず、評価できない子があれば `None`。
fn fold(list: &[Condition], run: &RunState, root: &Root, all_of: bool) -> Option<bool> {
    let mut acc = all_of;
    let mut unknown = false;
    for c in list {
        match eval_condition(c, run, root) {
            Some(b) => {
                if all_of {
                    acc &= b;
                } else {
                    acc |= b;
                }
            }
            None => unknown = true,
        }
    }
    if unknown {
        None
    } else {
        Some(acc)
    }
}

/// 語彙 v1 の条件で `when` を評価する [`ConditionEval`]。
///
/// `Value` を [`Condition::from_value`] で型にし、解釈に失敗したら `None`（評価できない）を返す。
/// 読み込み時の検査（pack）を通った教材では、失敗は起きない。
#[derive(Debug, Clone, Copy, Default)]
pub struct VocabConditions;

impl ConditionEval for VocabConditions {
    fn eval(&self, when: &Value, run: &RunState, root: &Root) -> Option<bool> {
        let c = Condition::from_value(when).ok()?;
        eval_condition(&c, run, root)
    }
}
