//! 到達性（`E-UNREACHABLE`）と行き止まり（`E-DEAD-END`）の検査（TASK-55 1/2）。
//!
//! `validate::validate` から呼ばれ、メモリ上の `LoadedPack` だけを見る純関数である。
//!
//! 到達性は、条件の充足可能性を調べない静的な辺の探索とする。条件つきの辺（`when` あり）も辺として数え、
//! 過大に近似する（誤報を出さない側）。名前空間は参照の検査（`check_refs`）と同じで、章は全章で 1 つ
//! （`next` は章をまたぐ。根は全章の `starts[].node`）、モジュールはモジュールごとに閉じる（根は最初の Step と
//! `starts[].node`）。`use` を持つ Step の到達性も通常の `next` の辺で決め、モジュールの Step は呼び出し元の
//! 到達性に関係なくモジュールの中だけで判定する（呼ばれないモジュールを報告する診断は一覧に無いため）。
//!
//! 次の名前空間では到達性を判定しない。根の原因の診断が別にあり、ここで報告すると連鎖して誤報になるためである。
//! - 根が 0 件（章の開始点が無い場合は `E-STRUCTURE`）
//! - 名前空間に無い Step を指す辺・開始点を含む（`E-REF-STEP`）。モジュールの引数（`${`）を含む ID も同様に扱う
//!
//! 範囲外: モジュールの引数と比べる条件（`condition.param`）の実体ごとの畳み込みは、条件の充足可能性の解析
//! （TASK-58）の持ち分である。
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
