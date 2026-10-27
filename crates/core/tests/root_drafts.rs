//! 根ごとの下書きと、根の切り替えでの入れ替え（MOD-4、TASK-236 1/2）の試験。
//!
//! `min-3step.json` を 2 つの実体（`m@r1`・`m@r2`）× Step 2 つに複製した小さな IR で、現在の根の下書き
//! （`current_draft`）が `Control::GoTo` で根を切り替えるたびに入れ替わることを確かめる。実際の 2 周の教材での
//! 通しは pack の `round_drafts` が持つ。送信で送った根だけが空になることは `submit_drafts.rs`（TASK-236 2/2）が持つ。
//! 下書きの値は利用者の入力なので、失敗メッセージには値を出さず、比較だけにとどめる。

mod common;

use common::contract_sample::testdata_dir;
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::{Session, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const STEPS: [&str; 4] = ["m@r1/s1", "m@r1/s2", "m@r2/s1", "m@r2/s2"];

/// 2 つの実体 × Step 2 つに複製した IR（`module_instances.rs` と同じ作り方）。
fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    let mut nodes = Vec::new();
    for scope in ["m@r1", "m@r2"] {
        for sid in ["s1", "s2"] {
            let mut n = base.clone();
            n["uid"] = json!(format!("{scope}/{sid}"));
            n["scope"] = json!(scope);
            n["step"]["id"] = json!(sid);
            n["step"]["next"] = json!([]);
            nodes.push(n);
        }
    }
    v["nodes"] = Value::Array(nodes);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/u1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/u2"},
    ]);
    v["starts"] = json!([]);
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn root_of_step(step: &str) -> Root {
    root(step.split('/').next().expect("root"))
}

fn rec_at(step: &str) -> RecordedSession {
    RecordedSession::new(Session::entered_at(ir(), step), "s-1")
}

fn goto(rs: &mut RecordedSession, step: &str) {
    rs.apply(Op::Control(Control::GoTo(uid(step))));
    assert_eq!(rs.session().state().run.current, Some(uid(step)), "moved");
}

#[test]
fn before_start_there_is_no_current_draft() {
    let mut rs = RecordedSession::new(Session::new(ir()), "s-1");
    rs.draft("x".to_string());
    assert_eq!(rs.current_draft(), None);
    assert!(rs.session().state().drafts.is_empty());
    assert_eq!(SessionState::default().draft_root(), None);
}

#[test]
fn switching_root_swaps_the_visible_draft() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("一".to_string());
    assert_eq!(rs.current_draft(), Some("一"));

    goto(&mut rs, "m@r2/s1");
    assert_eq!(rs.current_draft(), None, "new root starts empty");
    assert!(!rs.session().state().drafts.contains_key(&root("m@r2")));

    rs.draft("二".to_string());
    assert_eq!(rs.current_draft(), Some("二"));

    // 同じ根の別の Step では同じ下書きが見える（下書きは Step ではなく根の単位）。
    goto(&mut rs, "m@r1/s2");
    assert_eq!(rs.current_draft(), Some("一"));
    goto(&mut rs, "m@r2/s2");
    assert_eq!(rs.current_draft(), Some("二"));

    let d = &rs.session().state().drafts;
    assert_eq!(d.len(), 2);
    assert_eq!(d.get(&root("m@r1")).map(String::as_str), Some("一"));
}

#[test]
fn empty_draft_is_some_empty() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft(String::new());
    assert_eq!(rs.current_draft(), Some(""));
}

#[test]
fn stop_and_save_keep_the_current_draft() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("一".to_string());
    rs.apply(Op::Control(Control::Save));
    goto(&mut rs, "m@r2/s1");
    rs.draft("二".to_string());
    rs.apply(Op::Control(Control::Stop));
    assert_eq!(rs.current_draft(), Some("二"));
    goto(&mut rs, "m@r1/s1");
    assert_eq!(rs.current_draft(), Some("一"));
}

#[derive(Debug, Clone)]
enum Act {
    Draft(String),
    Go(usize),
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        "[a-z ]{0,6}".prop_map(Act::Draft),
        (0..STEPS.len()).prop_map(Act::Go),
    ]
}

#[test]
fn property_current_draft_follows_the_model_across_moves() {
    check(
        "root-drafts-model",
        DEFAULT_CASES,
        vec(act_strategy(), 0..MAX_OPS),
        |acts| {
            let mut rs = rec_at(STEPS[0]);
            let mut model: BTreeMap<Root, String> = BTreeMap::new();
            let mut cur = STEPS[0];
            for a in acts {
                match a {
                    Act::Draft(t) => {
                        model.insert(root_of_step(cur), t.clone());
                        rs.draft(t);
                    }
                    Act::Go(i) => {
                        cur = STEPS[i];
                        rs.apply(Op::Control(Control::GoTo(uid(cur))));
                    }
                }
                prop_assert!(
                    rs.session().state().drafts == model,
                    "drafts differ from model"
                );
                prop_assert!(
                    rs.current_draft() == model.get(&root_of_step(cur)).map(String::as_str),
                    "current draft differs from model"
                );
            }
            Ok(())
        },
    );
}
