//! ヒントの使用回数を Step の実体ごとに分ける（GUIDE-7、TASK-297 1/2）の試験。
//!
//! 3 つの実体（`m@r1`・`m@r2`・境界の対照の `m@r10`）× Step 2 つの IR で、開いた段数の鍵が完全な uid であり、
//! 別の根へ移っても混ざらないこと、根ごとの合計（`hints_opened_in`）が根の完全一致で数えられることを確かめる。
//! 実際の 2 周の教材での通しは pack の `round_hints` が持つ。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::property::{check, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{apply, Session, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::{json, Value};

const STEPS: [&str; 6] = [
    "m@r1/s1", "m@r1/s2", "m@r2/s1", "m@r2/s2", "m@r10/s1", "m@r10/s2",
];

fn ir() -> Ir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    // 水準 A（既定）で 2 本見えるヒントを持つ Step を、実体ごとに複製する。
    v["nodes"][0]["step"]["hints"] = json!(["h-all", "h-a"]);
    let base = v["nodes"][0].clone();
    let mut nodes = Vec::new();
    for scope in ["m@r1", "m@r2", "m@r10"] {
        for sid in ["s1", "s2"] {
            let mut n = base.clone();
            n["uid"] = json!(format!("{scope}/{sid}"));
            n["scope"] = json!(scope);
            n["step"]["id"] = json!(sid);
            nodes.push(n);
        }
    }
    v["nodes"] = Value::Array(nodes);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/u1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/u2"},
        {"id": "m@r10", "module": "m", "args": {}, "caller": "main/u3"},
    ]);
    v["starts"] = json!([]);
    load_ir(&serde_json::to_vec(&v).unwrap()).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn goto(ir: &Ir, st: SessionState, to: &str) -> SessionState {
    let (st, _) = apply(ir, st, Op::Control(Control::GoTo(uid(to))));
    assert_eq!(st.run.current, Some(uid(to)), "moved");
    st
}

fn open(ir: &Ir, st: SessionState) -> (SessionState, Vec<Event>) {
    apply(ir, st, Op::Control(Control::OpenHint))
}

fn start() -> (Ir, SessionState) {
    let ir = ir();
    let st = Session::entered_at(ir.clone(), "m@r1/s1").state().clone();
    (ir, st)
}

#[test]
fn opening_in_one_root_does_not_touch_another() {
    let (ir, st) = start();
    let (st, _) = open(&ir, st);
    let (st, _) = open(&ir, st);
    assert_eq!(st.hints_opened_at(&uid("m@r1/s1")), 2);
    assert_eq!(st.hints_opened_in(&root("m@r1")), 2);

    // 別の実体へ移った直後は 0。開いても元の実体の値は変わらない。
    let st = goto(&ir, st, "m@r2/s1");
    assert_eq!(st.hints_opened_at(&uid("m@r2/s1")), 0);
    assert_eq!(st.hints_opened_in(&root("m@r2")), 0);
    let (st, _) = open(&ir, st);
    assert_eq!(st.hints_opened_at(&uid("m@r2/s1")), 1);
    assert_eq!(st.hints_opened_in(&root("m@r2")), 1);
    assert_eq!(st.hints_opened_at(&uid("m@r1/s1")), 2);
    assert_eq!(st.hints_opened_in(&root("m@r1")), 2);

    // 戻ると元の値が見える。
    let st = goto(&ir, st, "m@r1/s1");
    assert_eq!(st.hints_opened_at(&uid("m@r1/s1")), 2);
}

#[test]
fn a_root_total_does_not_include_a_longer_root_name() {
    let (ir, st) = start();
    let (st, _) = open(&ir, st);
    let st = goto(&ir, st, "m@r10/s1");
    let (st, _) = open(&ir, st);
    let (st, _) = open(&ir, st);
    assert_eq!(st.hints_opened_in(&root("m@r1")), 1);
    assert_eq!(st.hints_opened_in(&root("m@r10")), 2);
}

#[test]
fn a_root_total_sums_the_steps_of_that_root() {
    let (ir, st) = start();
    let (st, _) = open(&ir, st);
    let st = goto(&ir, st, "m@r1/s2");
    let (st, _) = open(&ir, st);
    let (st, _) = open(&ir, st);
    assert_eq!(st.hints_opened_in(&root("m@r1")), 3);
}

#[test]
fn missing_entries_count_as_zero() {
    let st = SessionState::default();
    assert_eq!(st.hints_opened_at(&uid("m@r1/s1")), 0);
    assert_eq!(st.hints_opened_in(&root("m@r1")), 0);
}

#[test]
fn the_total_saturates_instead_of_overflowing() {
    let mut st = SessionState::default();
    st.hints_opened.insert(uid("m@r1/s1"), u32::MAX);
    st.hints_opened.insert(uid("m@r1/s2"), 5);
    assert_eq!(st.hints_opened_in(&root("m@r1")), u32::MAX);
}

#[test]
fn the_event_stamp_carries_the_root_that_was_opened_in() {
    let (ir, st) = start();
    let st = goto(&ir, st, "m@r2/s2");
    let (_, events) = open(&ir, st);
    match events.as_slice() {
        [Event::HintOpened { at, .. }] => assert_eq!(at.step.root(), root("m@r2")),
        other => panic!("unexpected events: {other:?}"),
    }
}

#[derive(Debug, Clone)]
enum Act {
    Open,
    Go(usize),
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![Just(Act::Open), (0..STEPS.len()).prop_map(Act::Go)]
}

#[test]
fn property_counts_follow_the_step_instance() {
    let ir = ir();
    check(
        "hint-instance-key",
        1_000,
        vec(act_strategy(), 0..MAX_OPS),
        |acts| {
            let mut st = Session::entered_at(ir.clone(), "m@r1/s1").state().clone();
            let mut model: BTreeMap<StepUid, u32> = BTreeMap::new();
            for a in acts {
                let before = st.hints_opened.clone();
                let (next, events) = match a {
                    Act::Open => open(&ir, st),
                    Act::Go(i) => apply(&ir, st, Op::Control(Control::GoTo(uid(STEPS[i])))),
                };
                st = next;
                for e in &events {
                    if let Event::HintOpened { at, opened, .. } = e {
                        let slot = model.entry(at.step.clone()).or_insert(0);
                        *slot = (*slot).max(*opened);
                        // 開いた Step の項目以外は操作の前後で変わらない。
                        for (k, v) in &before {
                            if *k != at.step {
                                prop_assert_eq!(st.hints_opened.get(k), Some(v));
                            }
                        }
                    }
                }
                prop_assert_eq!(&st.hints_opened, &model);
                for r in ["m@r1", "m@r2", "m@r10"] {
                    let sum: u32 = model
                        .iter()
                        .filter(|(k, _)| k.root() == root(r))
                        .map(|(_, v)| *v)
                        .sum();
                    prop_assert_eq!(st.hints_opened_in(&root(r)), sum);
                }
            }
            Ok(())
        },
    );
}
