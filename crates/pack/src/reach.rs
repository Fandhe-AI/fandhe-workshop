//! 到達性（`E-UNREACHABLE`）と行き止まり（`E-DEAD-END`）の検査（TASK-55 1/2）。
//!
//! `validate::validate` から呼ばれ、メモリ上の `LoadedPack` だけを見る純関数である。
//!
//! 到達性は、条件の充足可能性を調べない静的な辺の探索とする。条件つきの辺（`when` あり）も辺として数え、
//! 過大に近似する（誤報を出さない側）。名前空間は参照の検査（`check_refs`）と同じで、章は全章で 1 つ
//! （章の `next` は同じ章の中に限られ、他の章への辺は `E-REF-STEP`（TASK-67・PACK-43）。根は全章の `starts[].node`）、モジュールはモジュールごとに閉じる（根は最初の Step と
//! `starts[].node`）。`use` を持つ Step の到達性も通常の `next` の辺で決め、モジュールの Step は呼び出し元の
//! 到達性に関係なくモジュールの中だけで判定する（呼ばれないモジュールを報告する診断は一覧に無いため）。
//!
//! 次の名前空間では到達性を判定しない。根の原因の診断が別にあり、ここで報告すると連鎖して誤報になるためである。
//! - 根が 0 件（章の開始点が無い場合は `E-STRUCTURE`）
//! - 名前空間に無い Step を指す辺・開始点を含む（`E-REF-STEP`）。モジュールの引数（`${`）を含む ID も同様に扱う
//!
//! 範囲外: モジュールの引数と比べる条件（`condition.param`）の実体ごとの畳み込みを含む、条件の充足可能性と
//! Step の巡回は、検査に足さない（PACK-30・D-43）。教材テストで補う。検出しないことは
//! `tests/static_limits.rs` が限界パックで固定している。
//!
//! 確認を省く `wait` の Step の輪（`E-STRUCTURE`。#2209）: `wait` の Step（`input.kind: wait` で `processing` を持つ）に
//! `confirm: none` を付けると、入ると処理中になり、最後の進行で受付・達成になって最初の無条件の `next` へ移る（エンジンの
//! `transit::after_achieved` と同じ辺）。そのような Step だけをたどって同じ Step に戻る輪は、受講者の入力なしに進行が続き
//! 止まらない（エンジンの遷移の上限は 1 操作の中だけを数える）。輪に入る Step ごとに 1 件出す。
//!
//! 探索は明示的なスタックによる反復で、教材データが深い鎖でもスタックあふれを起こさない。診断に写す ID は
//! `safe_ident` を通ったものだけである。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::ir::Step;
use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{LoadedPack, LoadedScope, ScopeKind};
use crate::validate::{diag, shown};

/// 到達性と行き止まりを検査する。順序は `pack.scopes` の順、ファイルの中では Step の順で、
/// 1 つの Step では `E-UNREACHABLE`、`E-DEAD-END` の順。
pub(crate) fn check_reach(pack: &LoadedPack) -> Vec<Diagnostic> {
    let chapters: Vec<&LoadedScope> = pack
        .scopes
        .iter()
        .filter(|s| s.kind == ScopeKind::Chapter)
        .collect();
    let main_reach = reachable_set(&chapters, |s| {
        s.src.starts.iter().map(|st| st.node.as_str()).collect()
    });

    let mut out = Vec::new();
    // 名前空間（章はまとめて 1 つ、モジュールは 1 つずつ）ごとに、確認を省く `wait` の Step の輪を探す。
    let mut cyclic: BTreeSet<(usize, &str)> = BTreeSet::new();
    let module_spaces: Vec<Vec<usize>> = pack
        .scopes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == ScopeKind::Module)
        .map(|(i, _)| vec![i])
        .collect();
    let chapter_space: Vec<usize> = pack
        .scopes
        .iter()
        .enumerate()
        .filter(|(_, s)| s.kind == ScopeKind::Chapter)
        .map(|(i, _)| i)
        .collect();
    for space in std::iter::once(chapter_space).chain(module_spaces) {
        cyclic.extend(auto_cycles(pack, &space));
    }
    for (i, scope) in pack.scopes.iter().enumerate() {
        for step in &scope.src.steps {
            if cyclic.contains(&(i, step.id.as_str())) {
                out.push(diag(
                    &scope.file,
                    &format!("step:{}", step.id),
                    DiagCode::Structure,
                    format!(
                        "wait steps with confirm none form a cycle without learner input: {}",
                        shown(&step.id)
                    ),
                ));
            }
        }
    }
    for scope in &pack.scopes {
        let module_reach;
        let reach: Option<&BTreeSet<&str>> = match scope.kind {
            ScopeKind::Chapter => main_reach.as_ref(),
            ScopeKind::Module => {
                module_reach = reachable_set(&[scope], |s| {
                    let mut roots: Vec<&str> = s
                        .src
                        .steps
                        .first()
                        .map(|st| st.id.as_str())
                        .into_iter()
                        .collect();
                    roots.extend(s.src.starts.iter().map(|st| st.node.as_str()));
                    roots
                });
                module_reach.as_ref()
            }
        };
        for step in &scope.src.steps {
            let item = format!("step:{}", step.id);
            if let Some(set) = reach {
                if !set.contains(step.id.as_str()) {
                    out.push(diag(
                        &scope.file,
                        &item,
                        DiagCode::Unreachable,
                        format!("step is not reachable from any start: {}", shown(&step.id)),
                    ));
                }
            }
            if is_dead_end(step) {
                out.push(diag(
                    &scope.file,
                    &item,
                    DiagCode::DeadEnd,
                    format!(
                        "step is not final and has no unconditional next: {}",
                        shown(&step.id)
                    ),
                ));
            }
        }
    }
    out
}

/// 入ると処理中になり、受講者の入力なしに達成して次へ移る Step（`wait` の入力・`processing` あり・`confirm: none`）。
fn is_auto(step: &Step) -> bool {
    let kind = |v: &Option<Value>| {
        v.as_ref()
            .and_then(|x| x.get("kind"))
            .and_then(Value::as_str)
            .map(str::to_owned)
    };
    kind(&step.input).as_deref() == Some("wait")
        && !step.processing.is_empty()
        && kind(&step.confirm).as_deref() == Some("none")
}

/// 名前空間 `space`（`pack.scopes` の添字の集まり）の中で、確認を省く `wait` の Step だけをたどって戻る輪に入る Step。
/// 各 Step の次は、最初の無条件の `next`（エンジンが達成の後にたどる辺）だけとする。
fn auto_cycles<'a>(pack: &'a LoadedPack, space: &[usize]) -> BTreeSet<(usize, &'a str)> {
    // ID から（scope の添字, 確認を省く wait か, 次の ID）。ID の重複は最初のものを使う（重複は `E-DUP-ID`）。
    let mut node: BTreeMap<&'a str, (usize, bool, Option<&'a str>)> = BTreeMap::new();
    for &i in space {
        let Some(scope) = pack.scopes.get(i) else {
            continue;
        };
        for step in &scope.src.steps {
            let next = step
                .next
                .iter()
                .find(|t| !is_conditional(&t.when))
                .map(|t| t.to.as_str());
            node.entry(step.id.as_str())
                .or_insert((i, is_auto(step), next));
        }
    }
    let mut out = BTreeSet::new();
    for (&start, &(_, auto, _)) in &node {
        if !auto {
            continue;
        }
        // 確認を省く wait の Step だけをたどり、`node` の数を超えずに `start` へ戻れば輪に入る。
        let mut at = start;
        for _ in 0..node.len() {
            let Some(&(_, true, Some(next))) = node.get(at) else {
                break;
            };
            if next == start {
                if let Some(&(i, _, _)) = node.get(start) {
                    out.insert((i, start));
                }
                break;
            }
            at = next;
        }
    }
    out
}

/// 条件つきの遷移か（`when` が欠けているか `null` なら無条件）。
fn is_conditional(when: &Option<Value>) -> bool {
    matches!(when, Some(v) if !v.is_null())
}

/// `final` でなく、無条件の `next` が 1 本も無い Step。
fn is_dead_end(step: &Step) -> bool {
    !step.is_final && step.next.iter().all(|t| is_conditional(&t.when))
}

/// 名前空間（`scopes` の Step 全体）の中で、根から到達できる Step ID の集合。
/// 判定を飛ばす場合（根が 0 件・参照切れあり）は `None`。ID の重複は辺の和集合にする。
fn reachable_set<'a>(
    scopes: &[&'a LoadedScope],
    roots_of: impl Fn(&'a LoadedScope) -> Vec<&'a str>,
) -> Option<BTreeSet<&'a str>> {
    let mut graph: BTreeMap<&'a str, Vec<&'a str>> = BTreeMap::new();
    let mut roots: Vec<&'a str> = Vec::new();
    for scope in scopes {
        for step in &scope.src.steps {
            graph
                .entry(step.id.as_str())
                .or_default()
                .extend(step.next.iter().map(|t| t.to.as_str()));
        }
        roots.extend(roots_of(scope));
    }
    if roots.is_empty() {
        return None;
    }
    let dangling = roots.iter().any(|r| !graph.contains_key(r))
        || graph.values().flatten().any(|to| !graph.contains_key(to));
    if dangling {
        return None;
    }
    let mut seen: BTreeSet<&'a str> = BTreeSet::new();
    let mut stack = roots;
    while let Some(id) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        if let Some(edges) = graph.get(id) {
            stack.extend(edges.iter().copied().filter(|e| !seen.contains(e)));
        }
    }
    Some(seen)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_when_is_unconditional() {
        assert!(!is_conditional(&None));
        assert!(!is_conditional(&Some(Value::Null)));
        assert!(is_conditional(&Some(serde_json::json!({"flag": "f"}))));
    }
}
