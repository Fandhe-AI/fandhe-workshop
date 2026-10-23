//! 学習モジュールの実体（根）ごとの状態の読み出しと、状態を書き読みする根の決め方の一本化（MOD-1、TASK-233 1/2）。
//!
//! 同じ定義の Step も、実体ごとに別の Step の実体（`<実体 ID>/<Step ID>`）になる。状態は実体の根（[`Root`]）ごとの
//! 切り身（[`Slice`]）に持つので、片方の実体の Step の状態・フラグはもう片方に漏れない。IR は展開済みの節点
//! （`Node::uid`・`Node::scope`）と実体の表（`Ir::instances`）を持つため、本モジュールはそれを引くだけで、
//! 展開（`use` を実体の節点にする処理）は pack が担う（TASK-50）。
//!
//! 呼び出し元は [`crate::session::Session`] の読み取り用メソッドと、書き込み側の `transit`・`accept`、
//! 実体の分離を確かめる pack・web の試験である。
//!
//! # 扱わないこと
//!
//! - 呼び出しの Step（`Instance::caller`）の持ち主の規則（TASK-235・TASK-241）。規則を入れるときは、書き込みと読み出しが
//!   一緒に変わるよう [`root_of`] の中身だけを差し替える。片方だけ変えると食い違うので、根の決め方はここに寄せる
//! - 根の定義（最後の `/` か最初の `/` か。#1413）。変えると `CONTRACT_VERSION` の版上げになる
//! - IR の整合（実体数・Step の実体数・開始数の照合。TASK-233 2/2）
//!
//! 本モジュールの関数は `CONTRACT_VERSION` の署名の対象に含まない（`judge`・`text_fill` と同じ扱い）。
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 存在しない根・uid・不正な形の uid でも `None`・`NotStarted`・空の列に倒す。

use fandhe_workshop_contract::ir::Ir;

use crate::addr::{Root, StepUid};
use crate::session::{RunState, Slice};
use crate::step_status::StepStatus;

/// Step の状態を書き読みする根を決める唯一の関数。
///
/// 現在は Step の実体 ID の最後の `/` より前（[`StepUid::root`]）を返す。TASK-235・TASK-241 で呼び出しの Step の
/// 持ち主の規則（`Instance::caller`）を入れるときは、ここだけを差し替える。
pub fn root_of(uid: &StepUid) -> Root {
    uid.root()
}

/// IR の実体の根を IR の順に並べて返す（重複は除く。章の実体 `main` を含む）。
pub fn instance_roots(ir: &Ir) -> Vec<Root> {
    let mut roots: Vec<Root> = Vec::new();
    for instance in &ir.instances {
        let root = Root(instance.id.clone());
        if !roots.contains(&root) {
            roots.push(root);
        }
    }
    roots
}

/// 根の切り身。まだ何も書かれていない根は `None`（空の切り身を前もって作らない）。
pub fn slice_of<'a>(run: &'a RunState, root: &Root) -> Option<&'a Slice> {
    run.slices.get(root)
}

/// Step の実体の状態。未登録は `NotStarted`。
pub fn status_of(run: &RunState, uid: &StepUid) -> StepStatus {
    slice_of(run, &root_of(uid))
        .and_then(|slice| slice.status.get(uid))
        .copied()
        .unwrap_or_default()
}

/// 根に属する Step の実体を IR の順に返す。
pub fn steps_of(ir: &Ir, root: &Root) -> Vec<StepUid> {
    ir.nodes
        .iter()
        .map(|node| StepUid(node.uid.clone()))
        .filter(|uid| &root_of(uid) == root)
        .collect()
}

/// 根の切り身を書き込み用に引く（無ければ空で作る）。`transit`・`accept` の書き込みの入口。
pub(crate) fn slice_mut(run: &mut RunState, root: Root) -> &mut Slice {
    run.slices.entry(root).or_default()
}
