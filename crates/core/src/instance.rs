//! 学習モジュールの実体（根）ごとの状態の読み出しと、状態を書き読みする根の決め方の一本化（MOD-1・MOD-3、TASK-233・TASK-235 1/2）。
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
//! - `Projection` は IR を持たないので、復帰の復習の根は [`root_of`] で引く（呼び出しの Step で保存したときだけ core の持ち主と食い違う）
//! - 根の定義（最後の `/` か最初の `/` か。#1413）。変えると `CONTRACT_VERSION` の版上げになる
//!
//! # 根の 2 つの概念（MOD-3、TASK-235 1/2）
//!
//! - [`root_of`]: 実体 ID（scope）の根。フラグ・効果の範囲・条件の評価はこの根の切り身に持つ。
//! - [`owner_of`]: Step の状態（`status`）と選択（`choices`）の持ち主の根。呼び出しの Step（ある実体の `caller`）は、
//!   呼ばれた実体の根が持ち主になる。つまり周回ごとの呼び出しの Step の状態は、その周回の切り身に入る。
//!
//! 状態・選択を書く箇所と読む箇所は、すべて同じ [`owner_of`] を通す。片方だけ変えると食い違う。
//!
//! # 静的な有向非巡回グラフの確認（TASK-233 2/2）
//!
//! [`instance_graph`] は IR から実体の呼び出しのグラフ（呼び出し元の実体 → 呼ばれた実体。辺は `Instance::caller` の
//! 節点の `scope` から引く）を組み、有向非巡回で配線が整っていることと、根ごとの Step の実体・開始の数を返す。
//! 有向非巡回なのは実体の水準であり、Step の `next` の後戻り（`st-2-3` から `st-2-2` へ戻る辺など）は含まない。
//! 呼び出し元は pack・core の試験（教材の定義から導いた数との照合）である。`load_ir` には結線しない。手で組んだ IR や
//! 呼び出し元の節点を持たない小さな IR が読めなくなり、`IrLoadError` の契約に波及するため、拒否にするかは別の判断とする。
//!
//! 本モジュールの関数は `CONTRACT_VERSION` の署名の対象に含まない（`judge`・`text_fill` と同じ扱い）。
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 存在しない根・uid・不正な形の uid でも `None`・`NotStarted`・空の列に倒す。

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use fandhe_workshop_contract::ir::{Instance, Ir, Node};

use crate::addr::{Root, StepUid};
use crate::session::{RunState, Slice};
use crate::step_status::StepStatus;

/// 実体 ID（scope）の根。フラグ・効果・条件の評価の切り身を決める。
///
/// Step の実体 ID の最後の `/` より前（[`StepUid::root`]）を返す。Step の状態・選択の持ち主は [`owner_of`] で決める。
pub fn root_of(uid: &StepUid) -> Root {
    uid.root()
}

/// Step の状態（`status`）と選択（`choices`）の持ち主の根（MOD-3）。
///
/// `Instance::caller` がこの Step の `uid` である実体があれば、IR の順で最初のその実体の根を返す。無ければ [`root_of`] と同じ。
/// 1 回の線形探索でたどらないので、巡回・重複を含む IR でも止まり決定的である（全域）。
/// 呼び出し元は `transit`・`accept`・`confirm`・`env_input`・`checkpoint`・`Session` の書き読みで、web の読み取りもこれを通す。
pub fn owner_of(ir: &Ir, uid: &StepUid) -> Root {
    owner_in(&ir.instances, uid)
}

/// [`owner_of`] の実体。IR 全体を持たない呼び出し元（条件の評価）が実体の一覧だけで同じ持ち主を引く。
pub fn owner_in(instances: &[Instance], uid: &StepUid) -> Root {
    instances
        .iter()
        .find(|i| i.caller.as_deref() == Some(uid.0.as_str()))
        .map(|i| Root(i.id.clone()))
        .unwrap_or_else(|| root_of(uid))
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
pub fn status_of(ir: &Ir, run: &RunState, uid: &StepUid) -> StepStatus {
    slice_of(run, &owner_of(ir, uid))
        .and_then(|slice| slice.status.get(uid))
        .copied()
        .unwrap_or_default()
}

/// 根が状態を持つ Step の実体（呼び出しの Step を含む）を IR の順に返す。
pub fn steps_of(ir: &Ir, root: &Root) -> Vec<StepUid> {
    ir.nodes
        .iter()
        .map(|node| StepUid(node.uid.clone()))
        .filter(|uid| &owner_of(ir, uid) == root)
        .collect()
}

/// 根の切り身を書き込み用に引く（無ければ空で作る）。`transit`・`accept` の書き込みの入口。
pub(crate) fn slice_mut(run: &mut RunState, root: Root) -> &mut Slice {
    run.slices.entry(root).or_default()
}

/// 章の実体の ID。`caller` を持たない実体はこれだけである（`StepUid` の検査と同じ特別扱い）。
const MAIN_ROOT: &str = "main";

/// 実体の呼び出しの有向非巡回グラフと、根ごとの数（MOD-1、TASK-233 2/2）。
///
/// 数の照合の呼び出し元（pack・core の試験）が、教材の定義から導いた期待値と突き合わせるために読む。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstanceGraph {
    /// 実体の根のトポロジカル順（呼び出し元が先。同順位は IR の順）。先頭は `main`。
    pub order: Vec<Root>,
    /// 呼び出しの辺（呼び出し元の実体, 呼ばれた実体）。IR の `instances` の順。
    pub edges: Vec<(Root, Root)>,
    /// 根ごとの Step の実体の数（`Node::scope` で数える）。Step の無い根も 0 で持つ。
    pub steps: BTreeMap<Root, usize>,
    /// 根ごとの開始の数（`Start::node` の節点の `scope` で数える）。開始の無い根も 0 で持つ。
    pub starts: BTreeMap<Root, usize>,
}

impl InstanceGraph {
    /// 実体の数（`main` を含む）。
    pub fn instance_count(&self) -> usize {
        self.order.len()
    }

    /// Step の実体の総数。
    pub fn step_count(&self) -> usize {
        self.steps.values().sum()
    }

    /// 開始の総数。
    pub fn start_count(&self) -> usize {
        self.starts.values().sum()
    }
}

/// [`instance_graph`] が見つけた IR の不整合。ID だけを持ち、本文・引数の値は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstanceGraphError {
    /// 同じ ID の実体が 2 つ以上ある。
    DuplicateInstance(String),
    /// `caller` を持たない実体がちょうど 1 つでない（持つ数）。
    RootCount(usize),
    /// `caller` を持たない実体が `main` でない。
    RootNotMain(String),
    /// `caller` が存在する節点の `uid` でない、またはその節点の `scope` が実体でない。
    UnresolvedCaller {
        /// 呼ばれた実体。
        instance: String,
        /// 解決できなかった `caller`。
        caller: String,
    },
    /// 節点の `scope` が実体でない。
    UnknownScope {
        /// 節点の `uid`。
        uid: String,
        /// 実体でない `scope`。
        scope: String,
    },
    /// 節点の `scope` が `uid` の根と違う。
    ScopeMismatch {
        /// 節点の `uid`。
        uid: String,
        /// 節点の `scope`。
        scope: String,
    },
    /// `enter` の宛先が、この節点が呼んだ実体の節点でない。
    BadEnter {
        /// `enter` を持つ節点の `uid`。
        uid: String,
        /// `enter` の宛先。
        to: String,
    },
    /// `exit_to` の宛先が、この節点の実体の呼び出し元の節点でない。
    BadExit {
        /// `exit_to` を持つ節点の `uid`。
        uid: String,
        /// `exit_to` の宛先。
        to: String,
    },
    /// 実体の呼び出しが巡回している（自分を呼ぶ場合を含む）。IR の順で最初の巡回に載る実体。
    Cycle {
        /// 巡回に載る実体。
        instance: String,
    },
}

/// IR から実体の呼び出しのグラフを組み、有向非巡回と配線の整合を確かめる。最初に見つけた違反を 1 件返す。
///
/// 確かめること: 実体 ID が重複しない・`caller` 無しは `main` だけ・`caller` が節点に解決できる・節点の `scope` が
/// 実体で `uid` の根と一致する・`enter` と `exit_to` が呼び出しの対で結ばれている・呼び出しが巡回しない。
/// 巡回は辺の上のトポロジカル順（Kahn 法）が全実体を並べ切れるかで判定するので、巡回する IR でも有限で止まる。
pub fn instance_graph(ir: &Ir) -> Result<InstanceGraph, InstanceGraphError> {
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for instance in &ir.instances {
        if !ids.insert(instance.id.as_str()) {
            return Err(InstanceGraphError::DuplicateInstance(instance.id.clone()));
        }
    }
    let callerless: Vec<&str> = ir
        .instances
        .iter()
        .filter(|i| i.caller.is_none())
        .map(|i| i.id.as_str())
        .collect();
    match callerless.as_slice() {
        [only] if *only == MAIN_ROOT => {}
        [only] => return Err(InstanceGraphError::RootNotMain((*only).to_string())),
        other => return Err(InstanceGraphError::RootCount(other.len())),
    }

    let nodes: BTreeMap<&str, &Node> = ir.nodes.iter().map(|n| (n.uid.as_str(), n)).collect();
    let callers: BTreeMap<&str, Option<&str>> = ir
        .instances
        .iter()
        .map(|i| (i.id.as_str(), i.caller.as_deref()))
        .collect();

    // 辺: 呼び出し元の節点の `scope` の実体 → 呼ばれた実体。
    let mut edges: Vec<(Root, Root)> = Vec::new();
    for instance in &ir.instances {
        let Some(caller) = instance.caller.as_deref() else {
            continue;
        };
        let parent = nodes
            .get(caller)
            .map(|n| n.scope.as_str())
            .filter(|scope| ids.contains(scope));
        let Some(parent) = parent else {
            return Err(InstanceGraphError::UnresolvedCaller {
                instance: instance.id.clone(),
                caller: caller.to_string(),
            });
        };
        edges.push((Root(parent.to_string()), Root(instance.id.clone())));
    }

    for node in &ir.nodes {
        if !ids.contains(node.scope.as_str()) {
            return Err(InstanceGraphError::UnknownScope {
                uid: node.uid.clone(),
                scope: node.scope.clone(),
            });
        }
        if root_of(&StepUid(node.uid.clone())).0 != node.scope {
            return Err(InstanceGraphError::ScopeMismatch {
                uid: node.uid.clone(),
                scope: node.scope.clone(),
            });
        }
        if let Some(to) = node.enter.as_deref() {
            let called = nodes
                .get(to)
                .and_then(|dest| callers.get(dest.scope.as_str()).copied().flatten());
            if called != Some(node.uid.as_str()) {
                return Err(InstanceGraphError::BadEnter {
                    uid: node.uid.clone(),
                    to: to.to_string(),
                });
            }
        }
        if let Some(to) = node.exit_to.as_deref() {
            let back = callers.get(node.scope.as_str()).copied().flatten();
            if !nodes.contains_key(to) || back != Some(to) {
                return Err(InstanceGraphError::BadExit {
                    uid: node.uid.clone(),
                    to: to.to_string(),
                });
            }
        }
    }

    let order = topological_order(ir, &edges)?;

    let mut steps: BTreeMap<Root, usize> = order.iter().map(|r| (r.clone(), 0)).collect();
    let mut starts = steps.clone();
    for node in &ir.nodes {
        if let Some(n) = steps.get_mut(&Root(node.scope.clone())) {
            *n += 1;
        }
    }
    for start in &ir.starts {
        // 宛先の節点が無い開始は `load_ir` が拒否する。ここでは数えずに飛ばす（全域）。
        let scope = nodes.get(start.node.as_str()).map(|n| n.scope.clone());
        if let Some(n) = scope.and_then(|s| starts.get_mut(&Root(s))) {
            *n += 1;
        }
    }
    Ok(InstanceGraph {
        order,
        edges,
        steps,
        starts,
    })
}

/// 辺の上のトポロジカル順（Kahn 法。同順位は IR の順）。並べ切れなければ、IR の順で最初の残りを巡回とする。
fn topological_order(ir: &Ir, edges: &[(Root, Root)]) -> Result<Vec<Root>, InstanceGraphError> {
    let roots = instance_roots(ir);
    let mut indegree: BTreeMap<&Root, usize> = roots.iter().map(|r| (r, 0)).collect();
    for (_, child) in edges {
        if let Some(n) = indegree.get_mut(child) {
            *n += 1;
        }
    }
    let mut queue: VecDeque<&Root> = roots
        .iter()
        .filter(|r| indegree.get(r).copied() == Some(0))
        .collect();
    let mut order: Vec<Root> = Vec::new();
    while let Some(root) = queue.pop_front() {
        order.push(root.clone());
        for (parent, child) in edges {
            if parent != root {
                continue;
            }
            if let Some(n) = indegree.get_mut(child) {
                *n = n.saturating_sub(1);
                if *n == 0 {
                    queue.push_back(child);
                }
            }
        }
    }
    if order.len() == roots.len() {
        return Ok(order);
    }
    // 残りには巡回の下流の実体も含まれる。残りの辺の上で自分へ戻れる実体だけを巡回の構成員とする。
    let left: BTreeSet<&Root> = roots.iter().filter(|r| !order.contains(r)).collect();
    let on_cycle = roots
        .iter()
        .find(|r| left.contains(r) && returns_to_self(r, edges, &left));
    // 並べ切れなかった残りは必ず巡回を含むので `on_cycle` は見つかる。見つからない場合も全域に保つ。
    let picked = on_cycle.or_else(|| left.iter().next().copied());
    Err(InstanceGraphError::Cycle {
        instance: picked.map(|r| r.0.clone()).unwrap_or_default(),
    })
}

/// `start` から `within` の中の辺をたどって `start` に戻れるか（深さ優先。訪問済みを持つので有限で止まる）。
fn returns_to_self(start: &Root, edges: &[(Root, Root)], within: &BTreeSet<&Root>) -> bool {
    let mut seen: BTreeSet<&Root> = BTreeSet::new();
    let mut stack: Vec<&Root> = vec![start];
    while let Some(cur) = stack.pop() {
        for (parent, child) in edges {
            if parent != cur || !within.contains(child) {
                continue;
            }
            if child == start {
                return true;
            }
            if seen.insert(child) {
                stack.push(child);
            }
        }
    }
    false
}
