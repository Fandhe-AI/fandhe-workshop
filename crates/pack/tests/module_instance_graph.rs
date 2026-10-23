//! 実体数・Step の実体数・開始数が教材の定義と一致することの試験（MOD-1、TASK-233 2/2）。
//!
//! 期待値は IR からでなく、章・モジュールの定義ファイル（`source::load`）から導く。章の Step・開始は `main` に、
//! `use` の Step ごとに呼ばれたモジュールの Step・開始が実体の分だけ増える。これを compile した IR の数と、
//! core の `instance_graph` が返す根ごとの数に突き合わせる。`testdata/` は読むだけで書き換えない。

mod common;

use std::collections::BTreeMap;

use common::testdata;
use fandhe_workshop_core::addr::Root;
use fandhe_workshop_core::instance::{instance_graph, InstanceGraph};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::{compile_dir, compile_dir_with, CompileOptions};
use fandhe_workshop_pack::source::{load, LoadedPack, ScopeKind};

const PACK: &str = "module-reuse-rounds/test-data/pack";
/// 入れ子の上限（pack の展開と同じ 8 段）。
const MAX_DEPTH: usize = 8;

/// 定義から導いた数。鍵は実体の根。
#[derive(Default, Debug, PartialEq, Eq)]
struct Expected {
    steps: BTreeMap<String, usize>,
    starts: BTreeMap<String, usize>,
    /// (呼び出し元の実体, 呼ばれた実体)
    edges: Vec<(String, String)>,
}

impl Expected {
    fn total(m: &BTreeMap<String, usize>) -> usize {
        m.values().sum()
    }
}

/// モジュール `module` を実体 `inst` として展開した数を足す。
fn add_module(pack: &LoadedPack, module: &str, inst: &str, depth: usize, out: &mut Expected) {
    assert!(depth <= MAX_DEPTH, "nesting too deep in test data");
    let scope = pack
        .scopes
        .iter()
        .find(|s| s.kind == ScopeKind::Module && s.src.id == module)
        .unwrap_or_else(|| panic!("module {module} not defined"));
    add_scope(
        pack,
        &scope.src.steps,
        scope.src.starts.len(),
        inst,
        depth,
        out,
    );
}

fn add_scope(
    pack: &LoadedPack,
    steps: &[fandhe_workshop_contract::ir::Step],
    starts: usize,
    inst: &str,
    depth: usize,
    out: &mut Expected,
) {
    *out.steps.entry(inst.to_string()).or_default() += steps.len();
    *out.starts.entry(inst.to_string()).or_default() += starts;
    for step in steps {
        let Some(u) = &step.use_module else { continue };
        let module = u["module"].as_str().expect("module is a string");
        let instance = u["instance"].as_str().expect("instance is a string");
        assert!(
            !module.contains("${") && !instance.contains("${"),
            "argument-derived use is not covered by this test"
        );
        let child = if inst == "main" {
            format!("{module}@{instance}")
        } else {
            format!("{inst}/{module}@{instance}")
        };
        out.edges.push((inst.to_string(), child.clone()));
        add_module(pack, module, &child, depth + 1, out);
    }
}

fn expected(pack: &LoadedPack, chapters: Option<&[&str]>) -> Expected {
    let mut out = Expected::default();
    out.steps.insert("main".into(), 0);
    out.starts.insert("main".into(), 0);
    for scope in pack.scopes.iter().filter(|s| s.kind == ScopeKind::Chapter) {
        if chapters.is_some_and(|c| !c.contains(&scope.src.id.as_str())) {
            continue;
        }
        add_scope(
            pack,
            &scope.src.steps,
            scope.src.starts.len(),
            "main",
            0,
            &mut out,
        );
    }
    out
}

fn check(bytes: &[u8], want: &Expected) -> InstanceGraph {
    let ir = load_ir(bytes).expect("load_ir");
    let g = instance_graph(&ir).expect("instance_graph");
    assert_eq!(ir.nodes.len(), Expected::total(&want.steps), "nodes");
    assert_eq!(ir.starts.len(), Expected::total(&want.starts), "starts");
    assert_eq!(ir.instances.len(), want.steps.len(), "instances");
    assert_eq!(g.instance_count(), want.steps.len());
    assert_eq!(g.step_count(), Expected::total(&want.steps));
    assert_eq!(g.start_count(), Expected::total(&want.starts));
    let per = |m: &BTreeMap<Root, usize>| -> BTreeMap<String, usize> {
        m.iter().map(|(r, n)| (r.0.clone(), *n)).collect()
    };
    assert_eq!(per(&g.steps), want.steps);
    assert_eq!(per(&g.starts), want.starts);
    let mut got: Vec<(String, String)> = g
        .edges
        .iter()
        .map(|(a, b)| (a.0.clone(), b.0.clone()))
        .collect();
    let mut exp = want.edges.clone();
    got.sort();
    exp.sort();
    assert_eq!(got, exp, "edges");
    g
}

#[test]
fn counts_match_the_definition() {
    let dir = testdata(PACK);
    let pack = load(&dir).expect("load");
    let want = expected(&pack, None);
    // 仕様の MOD-1 の基点（Step の実体 36・実体 5・開始 19）と、定義から導いた値が同じであること。
    assert_eq!(Expected::total(&want.steps), 36);
    assert_eq!(want.steps.len(), 5);
    assert_eq!(Expected::total(&want.starts), 19);

    let g = check(&compile_dir(&dir).expect("compile"), &want);
    assert_eq!(g.order.first(), Some(&Root("main".into())));
    assert_eq!(g.edges.len(), 4);
    assert!(g.edges.iter().all(|(from, _)| from.0 == "main"));
}

#[test]
fn same_module_instances_have_same_step_count() {
    let dir = testdata(PACK);
    let ir = load_ir(&compile_dir(&dir).expect("compile")).expect("load_ir");
    let g = instance_graph(&ir).expect("instance_graph");
    let n = |r: &str| g.steps.get(&Root(r.into())).copied();
    assert_eq!(n("idea-cycle@round1"), n("idea-cycle@round2"));
    assert_eq!(n("issue-to-merge@round1"), n("issue-to-merge@round2"));
    assert_eq!(n("idea-cycle@round1"), Some(8));
}

#[test]
fn narrowed_chapters_keep_the_invariants() {
    let dir = testdata(PACK);
    let pack = load(&dir).expect("load");
    for chapter in ["ch-round1", "ch-round2"] {
        let want = expected(&pack, Some(&[chapter]));
        let opts = CompileOptions {
            chapters: Some(vec![chapter.to_string()]),
        };
        let bytes = compile_dir_with(&dir, &opts).expect("compile");
        let ir = load_ir(&bytes).expect("load_ir");
        let g = instance_graph(&ir).expect("instance_graph");
        assert_eq!(g.instance_count(), want.steps.len(), "{chapter} instances");
        assert_eq!(
            g.step_count(),
            Expected::total(&want.steps),
            "{chapter} steps"
        );
    }
}
