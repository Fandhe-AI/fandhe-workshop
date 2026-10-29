//! ヒントの使用回数が停止・復帰・移動・再読み込み・拒否で減らないことの試験（GUIDE-7、TASK-297 2/2）。
//!
//! `hints_opened` は `run` の外に置かれ、停止・復帰・移動は `run` と入口の写しだけを戻す。この構造による保証を、
//! 操作ごとに「前に空でない → 後で各鍵の値が前以上・鍵が消えない」として固定する。変異は `GUIDE7-3`〜`GUIDE7-5`。
//! 周回の開始と書き出しの往復は pack の `round_hint_counts`・`round_restore_property` が持つ。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::contract_sample::{load, testdata_dir};
use common::property::{check, MAX_OPS};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{apply, reload, Session, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::{json, Value};

type Counts = BTreeMap<StepUid, u32>;

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn sample_ir() -> Ir {
    let s = load();
    load_ir(&std::fs::read(testdata_dir().join(&s.ir)).unwrap()).unwrap()
}

/// ヒントを 2 本持つ Step を 3 つ並べた IR（`hint_instance_key` と同じ元データ）。
fn hints_ir() -> Ir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    v["nodes"][0]["step"]["hints"] = json!(["h-all", "h-a"]);
    let base = v["nodes"][0].clone();
    let mut nodes = Vec::new();
    for sid in ["s1", "s2", "s3"] {
        let mut n = base.clone();
        n["uid"] = json!(format!("main/{sid}"));
        n["step"]["id"] = json!(sid);
        nodes.push(n);
    }
    v["nodes"] = Value::Array(nodes);
    v["starts"] = json!([]);
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

const HSTEPS: [&str; 3] = ["main/s1", "main/s2", "main/s3"];

/// 各鍵が後でも残り、値が前以上であることを確かめる。
fn assert_monotone(before: &Counts, after: &Counts, what: &str) {
    for (k, v) in before {
        let now = after.get(k).copied();
        assert!(
            now.is_some_and(|n| n >= *v),
            "{what}: a hint count decreased or vanished"
        );
    }
}

fn open(ir: &Ir, st: SessionState) -> SessionState {
    apply(ir, st, Op::Control(Control::OpenHint)).0
}

fn goto(ir: &Ir, st: SessionState, to: &str) -> SessionState {
    apply(ir, st, Op::Control(Control::GoTo(uid(to)))).0
}

fn hinted_start() -> (Ir, SessionState) {
    let ir = hints_ir();
    let st = Session::entered_at(ir.clone(), "main/s1").state().clone();
    let st = open(&ir, open(&ir, st));
    assert!(!st.hints_opened.is_empty(), "precondition: hints are open");
    (ir, st)
}

/// 受け付けて 2 回進めた処理中の状態に、使用回数を持たせる。
fn busy_with_hints(ir: &Ir) -> SessionState {
    let first = Op::Input(Input::Command("/make-note first-note".into()));
    let mut st = apply(ir, load().initial, first).0;
    for _ in 0..2 {
        let addr = next_tick(&st).unwrap();
        st = apply(ir, st, Op::Tick(addr)).0;
    }
    assert!(st.run.busy.is_some(), "precondition: busy");
    st.hints_opened.insert(uid("main/st-1"), 2);
    st
}

#[test]
fn stop_while_busy_keeps_hint_counts() {
    let ir = sample_ir();
    let st = busy_with_hints(&ir);
    let before = st.hints_opened.clone();
    let (next, _) = apply(&ir, st, Op::Control(Control::Stop));
    assert!(next.run.busy.is_none(), "the stop took effect");
    assert_monotone(&before, &next.hints_opened, "stop");
}

#[test]
fn cancel_stop_keeps_hint_counts() {
    let ir = sample_ir();
    let mut st = load().initial;
    st.hints_opened.insert(uid("main/st-1"), 2);
    let before = st.hints_opened.clone();
    let (next, _) = apply(&ir, st, Op::Control(Control::Stop));
    assert_monotone(&before, &next.hints_opened, "cancel");
}

#[test]
fn restore_to_checkpoint_saved_before_hints_keeps_counts() {
    let ir = hints_ir();
    let st = Session::entered_at(ir.clone(), "main/s1").state().clone();
    let (st, _) = apply(&ir, st, Op::Control(Control::Save));
    let st = open(&ir, open(&ir, st));
    let before = st.hints_opened.clone();
    assert!(!before.is_empty());
    let (next, _) = apply(&ir, st, Op::Control(Control::Restore(CheckpointId(1))));
    assert_eq!(next.run.generation.0, 2, "the restore took effect");
    assert_monotone(&before, &next.hints_opened, "restore");
}

#[test]
fn goto_other_root_and_back_keeps_counts() {
    let (ir, st) = hinted_start();
    let before = st.hints_opened.clone();
    let st = goto(&ir, st, "main/s2");
    assert_eq!(st.run.current, Some(uid("main/s2")));
    assert_monotone(&before, &st.hints_opened, "goto away");
    let st = open(&ir, st);
    let mid = st.hints_opened.clone();
    let st = goto(&ir, st, "main/s1");
    assert_monotone(&mid, &st.hints_opened, "goto back");
}

#[test]
fn begin_next_round_keeps_previous_round_counts() {
    // 周回の開始の成功は pack の 2 周の教材で固定する（round_hint_counts）。ここでは拒否が減らさないことだけ。
    let (ir, st) = hinted_start();
    let before = st.hints_opened.clone();
    let (next, _) = apply(&ir, st, Op::Control(Control::Begin("no-such-start".into())));
    assert_monotone(&before, &next.hints_opened, "begin refused");
}

#[test]
fn reload_keeps_hint_counts() {
    let (_ir, st) = hinted_start();
    let before = st.hints_opened.clone();
    let (next, _) = reload(st);
    assert_monotone(&before, &next.hints_opened, "reload");
}

#[test]
fn refused_input_keeps_hint_counts() {
    let ir = sample_ir();
    let mut st = load().initial;
    st.hints_opened.insert(uid("main/st-1"), 2);
    let before = st.hints_opened.clone();
    let junk = Op::Input(Input::Command("/no-such-command zz".into()));
    let (next, _) = apply(&ir, st, junk);
    assert_monotone(&before, &next.hints_opened, "refused input");
}

#[derive(Debug, Clone)]
enum Act {
    Open,
    Stop,
    Save,
    Restore(u32),
    Go(usize),
    Reload,
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        3 => Just(Act::Open),
        1 => Just(Act::Stop),
        1 => Just(Act::Save),
        1 => (1u32..5).prop_map(Act::Restore),
        1 => (0..HSTEPS.len()).prop_map(Act::Go),
        1 => Just(Act::Reload),
    ]
}

#[test]
fn property_hint_counts_never_decrease() {
    let ir = hints_ir();
    check(
        "hint-count-monotone",
        1_000,
        vec(act_strategy(), 0..MAX_OPS),
        |acts| {
            let mut st = Session::entered_at(ir.clone(), "main/s1").state().clone();
            for a in acts {
                let before = st.hints_opened.clone();
                st = match a {
                    Act::Open => open(&ir, st),
                    Act::Stop => apply(&ir, st, Op::Control(Control::Stop)).0,
                    Act::Save => apply(&ir, st, Op::Control(Control::Save)).0,
                    Act::Restore(n) => {
                        apply(&ir, st, Op::Control(Control::Restore(CheckpointId(n)))).0
                    }
                    Act::Go(i) => goto(&ir, st, HSTEPS[i]),
                    Act::Reload => reload(st).0,
                };
                for (k, v) in &before {
                    prop_assert!(st.hints_opened.get(k).is_some_and(|n| n >= v));
                }
            }
            Ok(())
        },
    );
}
