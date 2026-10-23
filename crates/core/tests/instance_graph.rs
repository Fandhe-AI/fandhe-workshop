//! 実体の呼び出しの有向非巡回グラフと数の照合（MOD-1、TASK-233 2/2）の試験。
//!
//! `min-3step.json` の節点を雛形に、実体と節点を手で組んだ小さな IR で、`instance_graph` が受理する形（入れ子を含む）と、
//! 拒否する形（巡回・解決できない呼び出し元・`scope` の食い違い・`main` の欠落・配線の誤り）を確かめる。
//! 教材 1 本分の数の照合（定義から導いた期待値との一致）は pack の `module_instance_graph` が持つ。
//! IR は `load_ir` を通してから渡し、読み込みが拒否する形（宛先の無い `next` など）は作らない。

mod common;

use std::collections::BTreeMap;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::Root;
use fandhe_workshop_core::instance::{instance_graph, InstanceGraphError as E};
use fandhe_workshop_core::load_ir;
use serde_json::{json, Value};

/// 節点の指定（uid・scope・enter・exit_to）。
struct N(
    &'static str,
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
);

/// 実体の指定（id・module・caller）。
type I = (&'static str, Option<&'static str>, Option<&'static str>);

fn base() -> Value {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    serde_json::from_slice(&bytes).expect("json")
}

fn node(base: &Value, n: &N) -> Value {
    let mut out = base["nodes"][0].clone();
    let sid = n.0.rsplit_once('/').map_or(n.0, |(_, s)| s);
    out["uid"] = json!(n.0);
    out["scope"] = json!(n.1);
    out["step"]["id"] = json!(sid);
    out["step"]["next"] = json!([]);
    if let Some(e) = n.2 {
        out["enter"] = json!(e);
    }
    if let Some(x) = n.3 {
        out["exit_to"] = json!(x);
    }
    out
}

/// `starts` は開始の宛先の節点。
fn ir(instances: &[I], nodes: &[N], starts: &[&str]) -> Ir {
    let mut v = base();
    v["nodes"] = Value::Array(nodes.iter().map(|n| node(&v, n)).collect());
    v["instances"] = Value::Array(
        instances
            .iter()
            .map(|(id, module, caller)| {
                json!({"id": id, "module": module, "args": {}, "caller": caller})
            })
            .collect(),
    );
    v["starts"] = Value::Array(
        starts
            .iter()
            .enumerate()
            .map(|(i, n)| json!({"id": format!("s{i}"), "node": n, "effects": []}))
            .collect(),
    );
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

/// main(2 Step。u1・u2 が m を呼ぶ) → m@r1(2)・m@r2(2)、m@r1 の n1 が n@x を呼ぶ。
fn nested() -> Ir {
    ir(
        &[
            ("main", None, None),
            ("m@r1", Some("m"), Some("main/u1")),
            ("m@r2", Some("m"), Some("main/u2")),
            ("m@r1/n@x", Some("n"), Some("m@r1/n1")),
        ],
        &[
            N("main/u1", "main", Some("m@r1/n1"), None),
            N("main/u2", "main", Some("m@r2/n1"), None),
            N("m@r1/n1", "m@r1", Some("m@r1/n@x/k1"), None),
            N("m@r1/n2", "m@r1", None, Some("main/u1")),
            N("m@r2/n1", "m@r2", None, None),
            N("m@r2/n2", "m@r2", None, Some("main/u2")),
            N("m@r1/n@x/k1", "m@r1/n@x", None, Some("m@r1/n1")),
        ],
        &["main/u1", "m@r1/n1", "m@r2/n1", "m@r1/n@x/k1"],
    )
}

#[test]
fn nested_instances_are_accepted_with_counts() {
    let g = instance_graph(&nested()).expect("accepted");
    assert_eq!(g.order.first(), Some(&root("main")));
    assert_eq!(
        g.order,
        vec![root("main"), root("m@r1"), root("m@r2"), root("m@r1/n@x")]
    );
    assert_eq!(
        g.edges,
        vec![
            (root("main"), root("m@r1")),
            (root("main"), root("m@r2")),
            (root("m@r1"), root("m@r1/n@x")),
        ]
    );
    assert_eq!(g.instance_count(), 4);
    assert_eq!(g.step_count(), 7);
    assert_eq!(g.start_count(), 4);
    let steps: BTreeMap<Root, usize> = [
        (root("main"), 2),
        (root("m@r1"), 2),
        (root("m@r2"), 2),
        (root("m@r1/n@x"), 1),
    ]
    .into();
    assert_eq!(g.steps, steps);
    assert_eq!(g.starts.get(&root("main")), Some(&1));
    assert_eq!(g.starts.get(&root("m@r1/n@x")), Some(&1));
}

#[test]
fn parents_precede_children_in_order() {
    let g = instance_graph(&nested()).expect("accepted");
    let pos = |r: &Root| g.order.iter().position(|x| x == r).expect("in order");
    for (parent, child) in &g.edges {
        assert!(pos(parent) < pos(child));
    }
}

#[test]
fn instance_without_steps_keeps_a_zero_entry() {
    let g = instance_graph(&ir(&[("main", None, None)], &[], &[])).expect("accepted");
    assert_eq!(g.steps.get(&root("main")), Some(&0));
    assert_eq!(g.starts.get(&root("main")), Some(&0));
    assert_eq!(
        (g.instance_count(), g.step_count(), g.start_count()),
        (1, 0, 0)
    );
}

#[test]
fn same_ir_gives_same_result() {
    assert_eq!(instance_graph(&nested()), instance_graph(&nested()));
}

#[test]
fn mutual_call_is_a_cycle() {
    let ir = ir(
        &[
            ("main", None, None),
            ("a@x", Some("a"), Some("b@x/b1")),
            ("b@x", Some("b"), Some("a@x/a1")),
        ],
        &[
            N("main/u1", "main", None, None),
            N("a@x/a1", "a@x", None, None),
            N("b@x/b1", "b@x", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::Cycle {
            instance: "a@x".into()
        })
    );
}

#[test]
fn self_call_is_a_cycle() {
    let ir = ir(
        &[("main", None, None), ("a@x", Some("a"), Some("a@x/a1"))],
        &[
            N("main/u1", "main", None, None),
            N("a@x/a1", "a@x", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::Cycle {
            instance: "a@x".into()
        })
    );
}

#[test]
fn cycle_reports_a_member_not_a_downstream_instance() {
    // IR の順は main, c, a, b。辺は a→b・b→a・a→c。c は巡回の下流で構成員ではない。
    let ir = ir(
        &[
            ("main", None, None),
            ("c@x", Some("c"), Some("a@x/a1")),
            ("a@x", Some("a"), Some("b@x/b1")),
            ("b@x", Some("b"), Some("a@x/a1")),
        ],
        &[
            N("main/u1", "main", None, None),
            N("c@x/c1", "c@x", None, None),
            N("a@x/a1", "a@x", None, None),
            N("b@x/b1", "b@x", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::Cycle {
            instance: "a@x".into()
        })
    );
}

#[test]
fn three_instance_ring_is_a_cycle() {
    let ir = ir(
        &[
            ("main", None, None),
            ("a@x", Some("a"), Some("c@x/c1")),
            ("b@x", Some("b"), Some("a@x/a1")),
            ("c@x", Some("c"), Some("b@x/b1")),
        ],
        &[
            N("main/u1", "main", None, None),
            N("a@x/a1", "a@x", None, None),
            N("b@x/b1", "b@x", None, None),
            N("c@x/c1", "c@x", None, None),
        ],
        &[],
    );
    assert!(matches!(instance_graph(&ir), Err(E::Cycle { .. })));
}

#[test]
fn caller_that_is_not_a_node_is_unresolved() {
    // 1/2 の試験の IR（caller が節点にならない）もこの形に当たる。
    let ir = ir(
        &[("main", None, None), ("m@r1", Some("m"), Some("main/none"))],
        &[
            N("main/u1", "main", None, None),
            N("m@r1/s1", "m@r1", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::UnresolvedCaller {
            instance: "m@r1".into(),
            caller: "main/none".into()
        })
    );
}

#[test]
fn unknown_scope_is_rejected() {
    let ir = ir(
        &[("main", None, None)],
        &[
            N("main/u1", "main", None, None),
            N("zz/s1", "zz", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::UnknownScope {
            uid: "zz/s1".into(),
            scope: "zz".into()
        })
    );
}

#[test]
fn scope_that_differs_from_uid_root_is_rejected() {
    let ir = ir(
        &[("main", None, None), ("m@r1", Some("m"), Some("main/u1"))],
        &[
            N("main/u1", "main", None, None),
            N("m@r1/s1", "main", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::ScopeMismatch {
            uid: "m@r1/s1".into(),
            scope: "main".into()
        })
    );
}

#[test]
fn root_must_be_exactly_one_main() {
    let two = ir(
        &[("main", None, None), ("other", None, None)],
        &[N("main/u1", "main", None, None)],
        &[],
    );
    assert_eq!(instance_graph(&two), Err(E::RootCount(2)));
    let none = ir(&[], &[], &[]);
    assert_eq!(instance_graph(&none), Err(E::RootCount(0)));
    let not_main = ir(&[("other", None, None)], &[], &[]);
    assert_eq!(
        instance_graph(&not_main),
        Err(E::RootNotMain("other".into()))
    );
}

#[test]
fn duplicate_instance_id_is_rejected() {
    let ir = ir(
        &[
            ("main", None, None),
            ("m@r1", Some("m"), Some("main/u1")),
            ("m@r1", Some("m"), Some("main/u1")),
        ],
        &[N("main/u1", "main", None, None)],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::DuplicateInstance("m@r1".into()))
    );
}

#[test]
fn enter_into_another_instance_is_rejected() {
    // u1 は m@r1 を呼ぶが、enter が m@r2 の節点を指す。
    let ir = ir(
        &[
            ("main", None, None),
            ("m@r1", Some("m"), Some("main/u1")),
            ("m@r2", Some("m"), Some("main/u2")),
        ],
        &[
            N("main/u1", "main", Some("m@r2/n1"), None),
            N("main/u2", "main", Some("m@r2/n1"), None),
            N("m@r1/n1", "m@r1", None, None),
            N("m@r2/n1", "m@r2", None, None),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::BadEnter {
            uid: "main/u1".into(),
            to: "m@r2/n1".into()
        })
    );
}

#[test]
fn exit_to_a_node_that_is_not_the_caller_is_rejected() {
    let ir = ir(
        &[("main", None, None), ("m@r1", Some("m"), Some("main/u1"))],
        &[
            N("main/u1", "main", Some("m@r1/n1"), None),
            N("main/u2", "main", None, None),
            N("m@r1/n1", "m@r1", None, Some("main/u2")),
        ],
        &[],
    );
    assert_eq!(
        instance_graph(&ir),
        Err(E::BadExit {
            uid: "m@r1/n1".into(),
            to: "main/u2".into()
        })
    );
}
