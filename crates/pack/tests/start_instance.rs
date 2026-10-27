//! 途中開始用の状態から周回の途中の Step で開始する試験（TASK-239 1/2、MOD-6）。
//!
//! `module-reuse-rounds` の 2 周目の実体 `issue-to-merge@round2` の開始 `s-6-1`・`s-7-1` について、次の 2 点を
//! コンパイルした IR と `Control::Begin` で確かめる。
//!
//! 1. 開始する Step が `Entered` になる（呼び出し元の根ではなく実体の根の Step に着地する）。
//! 2. 開始用の効果の `${名前}` が、その実体の引数（`ir.instances[].args`）で置換される。開始 ID は `<実体 ID>#<開始 ID>`。
//!
//! 置換は pack の compile（`module_starts`）が行い、core は置換済みの IR の値をそのまま適用する。期待値は IR の
//! 実体の引数から組み、値を直書きしない。実体 ID だけ spec の例の名前を使う。2 周目の引数が 1 周目と異なることを
//! 前提として確かめ、置換の有無を区別できない条件で試験が空振りしないようにする。
//!
//! 飛ばした Step の `Skipped` 3 件と、前の周回を終えた状態からの開始で前の周回の切り身が変わらないことは
//! TASK-239 2/2 が持つ（一部は `start_skip.rs`）。記録は `docs/testing.md` の 5.328。

mod common;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::{owner_of, root_of, status_of};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;

const R1: &str = "issue-to-merge@round1";
const R2: &str = "issue-to-merge@round2";

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn arg(ir: &Ir, inst: &str, name: &str) -> String {
    ir.instances
        .iter()
        .find(|i| i.id == inst)
        .and_then(|i| i.args.get(name))
        .unwrap_or_else(|| panic!("{inst} arg {name}"))
        .clone()
}

fn begin(ir: &Ir, id: &str) -> (Session, Vec<Event>) {
    let mut s = Session::from_state(ir.clone(), SessionState::default());
    let events = s
        .apply(Op::Control(Control::Begin(id.to_string())))
        .to_vec();
    (s, events)
}

/// 開始の `effects` の `put` の経路を順に集める。
fn put_paths(ir: &Ir, id: &str) -> Vec<String> {
    let st = ir.starts.iter().find(|s| s.id == id).expect(id);
    st.effects
        .iter()
        .map(|v| {
            v["put"]["path"]
                .as_str()
                .unwrap_or_else(|| panic!("put.path: {v}"))
                .to_string()
        })
        .collect()
}

fn doc_path(ir: &Ir, inst: &str) -> String {
    format!("impl/docs/behavior-{}.md", arg(ir, inst, "idea"))
}

#[test]
fn mod6_module_start_effects_are_substituted_per_instance_in_ir() {
    let ir = ir();
    for name in ["idea", "root", "child"] {
        assert_ne!(arg(&ir, R1, name), arg(&ir, R2, name), "{name} must differ");
    }
    assert_eq!(
        put_paths(&ir, &format!("{R2}#s-6-1")),
        vec![doc_path(&ir, R2)]
    );
    assert_eq!(
        put_paths(&ir, &format!("{R2}#s-7-1")),
        vec![
            doc_path(&ir, R2),
            format!("issues/root-{}", arg(&ir, R2, "root")),
            format!("issues/child-{}", arg(&ir, R2, "child")),
        ]
    );
    assert_eq!(
        put_paths(&ir, &format!("{R1}#s-6-1")),
        vec![doc_path(&ir, R1)]
    );
    for st in &ir.starts {
        let text = serde_json::to_string(&(&st.effects, &st.requires)).expect("json");
        assert!(!text.contains("${"), "{}: {text}", st.id);
    }
}

#[test]
fn mod6_round2_start_enters_the_start_step() {
    let ir = ir();
    let id = format!("{R2}#s-6-1");
    let (s, events) = begin(&ir, &id);
    let begun: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Begun { start, .. } => Some(start.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(begun, vec![id.clone()]);
    let uid = StepUid(format!("{R2}/st-6-1"));
    assert_eq!(s.state().run.current.as_ref(), Some(&uid));
    assert_eq!(status_of(&ir, &s.state().run, &uid), StepStatus::Entered);
    assert_eq!(s.step_status(&uid), StepStatus::Entered);
    assert_eq!(root_of(&uid).0, R2);
    assert_eq!(owner_of(&ir, &uid).0, R2);
}

#[test]
fn mod6_round2_start_writes_instance_arg_paths_into_world() {
    let ir = ir();
    let (s, events) = begin(&ir, &format!("{R2}#s-6-1"));
    let path = doc_path(&ir, R2);
    let el = s.state().run.world.get(&path).expect("round2 path");
    assert_eq!(el.fixture.as_deref(), Some("fx-kakuu-handoff-diff"));
    assert!(!s.state().run.world.contains_key(&doc_path(&ir, R1)));
    assert!(s.state().run.world.keys().all(|k| !k.contains("${")));
    let changed: Vec<_> = events
        .iter()
        .filter_map(|e| match e {
            Event::Changed { path: p, cause, .. } if *p == path => Some(*cause),
            _ => None,
        })
        .collect();
    assert_eq!(changed, vec![Cause::Start]);
}

#[test]
fn mod6_round2_s7_start_writes_root_and_child_with_round2_args() {
    let ir = ir();
    let (s, _) = begin(&ir, &format!("{R2}#s-7-1"));
    let uid = StepUid(format!("{R2}/st-7-1"));
    assert_eq!(status_of(&ir, &s.state().run, &uid), StepStatus::Entered);
    let world = &s.state().run.world;
    for (prefix, name) in [("issues/root-", "root"), ("issues/child-", "child")] {
        let el = world
            .get(&format!("{prefix}{}", arg(&ir, R2, name)))
            .expect(name);
        assert_eq!(el.attrs.get("state").map(String::as_str), Some("open"));
        assert!(!world.contains_key(&format!("{prefix}{}", arg(&ir, R1, name))));
    }
}

#[test]
fn mod6_round1_and_round2_starts_differ_only_by_instance_args() {
    let ir = ir();
    let (s1, _) = begin(&ir, &format!("{R1}#s-6-1"));
    let (s2, _) = begin(&ir, &format!("{R2}#s-6-1"));
    assert_eq!(
        s1.state().run.current,
        Some(StepUid(format!("{R1}/st-6-1")))
    );
    assert_eq!(
        s2.state().run.current,
        Some(StepUid(format!("{R2}/st-6-1")))
    );
    let keys = |s: &Session| -> Vec<String> { s.state().run.world.keys().cloned().collect() };
    assert_eq!(keys(&s1), vec![doc_path(&ir, R1)]);
    assert_eq!(keys(&s2), vec![doc_path(&ir, R2)]);
}
