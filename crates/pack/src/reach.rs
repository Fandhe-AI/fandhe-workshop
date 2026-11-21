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
//! 止まらない（エンジンの遷移の上限は 1 操作の中だけを数える）。輪に入る Step ごとに 1 件出す。モジュールの引数の置換の後で
//! 決まる値もあるため、展開の後の Step（[`check_auto_cycles`]。`validate` だけ）で判定する。章をまたぐ辺は展開の前に
//! `E-REF-STEP` で断られるので、ここには来ない。上の「Step の巡回は検査に足さない」の例外である。
//!
//! 探索は明示的なスタックによる反復で、教材データが深い鎖でもスタックあふれを起こさない。診断に写す ID は
//! `safe_ident` を通ったものだけである。

use std::collections::{BTreeMap, BTreeSet};

use fandhe_workshop_contract::ir::{Node, Step};
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

/// 確認を省く `wait` の Step なら、達成の後に移る先（最初の無条件の `next`）。それ以外は `None`。
fn auto_next(n: &Node) -> Option<&str> {
    if !is_auto(&n.step) {
        return None;
    }
    n.step
        .next
        .iter()
        .find(|t| !is_conditional(&t.when))
        .map(|t| t.to.as_str())
}

/// 展開の後の Step（`nodes`）で、確認を省く `wait` の Step だけを最初の無条件の `next` でたどって戻る輪を探し、輪に入る
/// Step ごとに `E-STRUCTURE` を 1 件出す（#2209）。各 Step の自動の次は高々 1 本なので、たどり済みの印を付けて
/// 1 回ずつ見る（線形）。同じ定義の実体ごとに重なる診断は呼び出し側が 1 件にする。
pub(crate) fn check_auto_cycles(pack: &LoadedPack, nodes: &[Node]) -> Vec<Diagnostic> {
    let index: BTreeMap<&str, &Node> = nodes.iter().map(|n| (n.uid.as_str(), n)).collect();

    // 0: 未着手、1: いまの道の上、2: 済み。
    let mut mark: BTreeMap<&str, u8> = BTreeMap::new();
    let mut cyclic: BTreeSet<&str> = BTreeSet::new();
    for n in nodes {
        let mut path: Vec<&str> = Vec::new();
        let mut at = n.uid.as_str();
        loop {
            match mark.get(at).copied().unwrap_or(0) {
                2 => break,
                1 => {
                    // いまの道で `at` に戻った。`at` から先が輪。
                    if let Some(p) = path.iter().position(|u| *u == at) {
                        cyclic.extend(path.get(p..).unwrap_or_default().iter().copied());
                    }
                    break;
                }
                _ => {}
            }
            mark.insert(at, 1);
            path.push(at);
            match index.get(at).and_then(|node| auto_next(node)) {
                Some(next) if index.contains_key(next) => at = next,
                _ => break,
            }
        }
        for u in path {
            mark.insert(u, 2);
        }
    }
    let file_of = |unit: &str| {
        pack.scopes
            .iter()
            .find(|s| s.src.id == unit)
            .map_or(pack.workshop_file.as_str(), |s| s.file.as_str())
    };
    nodes
        .iter()
        .filter(|n| cyclic.contains(n.uid.as_str()))
        .map(|n| {
            diag(
                file_of(&n.unit),
                &format!("step:{}", n.step.id),
                DiagCode::Structure,
                format!(
                    "wait steps with confirm none form a cycle without learner input: {}",
                    shown(&n.step.id)
                ),
            )
        })
        .collect()
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
