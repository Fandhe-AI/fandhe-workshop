//! 送信で、送った時点の根の下書きだけが空になること（MOD-4・S-7。TASK-236 2/2）の試験。
//!
//! 送信が受け付けられ、同じ `apply` の中で別の根へ着地する小さな IR（`m@r1/s1` の達成で `main/u1` へ戻り、
//! `main/u2` を経て `m@r2/s1` に入る）を合成し、除かれるのが着地した根でなく送った根であることを確かめる。
//! 除く処理は `history_envelope::record_submit` が `apply` の前に行う。この順序の不変条件を固定するのが本ファイルの役割である。
//! 拒否の送信でも core の写しからは除かれる（入力欄へ残す下書きは web が記録し直す。`host.rs` の `keeps_draft`）。
//! 下書きの値は利用者の入力なので、失敗メッセージには値を出さず、比較だけにとどめる。

mod common;

use common::contract_sample::testdata_dir;
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::{instance, Session};
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};

const STEPS: [&str; 3] = ["m@r1/s1", "m@r2/s1", "main/u1"];
const PREFIX: &str = "/";

fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    let mk = |uid: &str, scope: &str| {
        let mut n = base.clone();
        n["uid"] = json!(uid);
        n["scope"] = json!(scope);
        n["step"]["id"] = json!(uid.rsplit('/').next().expect("id"));
        n["step"]["next"] = json!([]);
        n
    };
    let mut nodes = Vec::new();
    let mut u1 = mk("main/u1", "main");
    u1["enter"] = json!("m@r1/s1");
    u1["step"]["next"] = json!([{"to": "main/u2", "when": null}]);
    nodes.push(u1);
    let mut u2 = mk("main/u2", "main");
    u2["enter"] = json!("m@r2/s1");
    u2["step"]["final"] = json!(true);
    nodes.push(u2);
    for (r, caller) in [("m@r1", "main/u1"), ("m@r2", "main/u2")] {
        let mut s = mk(&format!("{r}/s1"), r);
        s["step"]["processing"] = json!([]);
        s["step"]["confirm"] = json!({"kind": "none", "reason": "受付と同時に達成にする"});
        s["step"]["final"] = json!(true);
        s["exit_to"] = json!(caller);
        nodes.push(s);
    }
    v["nodes"] = Value::Array(nodes);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@r1", "module": "m", "args": {}, "caller": "main/u1"},
        {"id": "m@r2", "module": "m", "args": {}, "caller": "main/u2"},
    ]);
    v["starts"] = json!([{"id": "s-1", "node": "m@r1/s1", "effects": []}]);
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn rec_at(step: &str) -> RecordedSession {
    RecordedSession::new(Session::entered_at(ir(), step), "s-1")
}

fn goto(rs: &mut RecordedSession, step: &str) {
    rs.apply(Op::Control(Control::GoTo(uid(step))));
    assert_eq!(rs.session().state().run.current, Some(uid(step)), "moved");
}

fn current_root(rs: &RecordedSession) -> Option<Root> {
    rs.session().state().draft_root()
}

#[test]
fn fixture_ir_has_a_valid_instance_graph() {
    assert!(instance::instance_graph(&ir()).is_ok());
}

#[test]
fn submit_that_lands_in_another_root_clears_only_the_sent_root() {
    let mut rs = rec_at("m@r1/s1");
    goto(&mut rs, "m@r2/s1");
    rs.draft("二".to_string());
    goto(&mut rs, "m@r1/s1");
    rs.draft("一".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert_eq!(current_root(&rs), Some(root("m@r2")), "landed in r2");
    let d = &rs.session().state().drafts;
    assert!(!d.contains_key(&root("m@r1")), "sent root is cleared");
    assert_eq!(d.get(&root("m@r2")).map(String::as_str), Some("二"));
    assert_eq!(rs.current_draft(), Some("二"));
}

#[test]
fn submit_that_returns_to_main_keeps_other_roots() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("一".to_string());
    goto(&mut rs, "m@r2/s1");
    rs.draft("二".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert_eq!(current_root(&rs), Some(root("main")), "returned to main");
    let d = &rs.session().state().drafts;
    assert!(!d.contains_key(&root("m@r2")));
    assert_eq!(d.get(&root("m@r1")).map(String::as_str), Some("一"));
    assert_eq!(d.len(), 1);
}

#[test]
fn rejected_submit_still_clears_sent_root_in_core() {
    // 拒否でも core の写しは除く。入力欄へ残す下書きは web が記録し直す（`host.rs` の `keeps_draft`）。
    let mut rs = rec_at("m@r1/s1");
    rs.draft("一".to_string());
    rs.submit(PREFIX, "/nope".to_string());
    assert_eq!(current_root(&rs), Some(root("m@r1")));
    assert!(!rs.session().state().drafts.contains_key(&root("m@r1")));
}

#[test]
fn blank_submit_changes_nothing() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("一".to_string());
    let before = rs.session().state().drafts.clone();
    rs.submit(PREFIX, "   ".to_string());
    assert_eq!(rs.session().state().drafts, before);
}

#[test]
fn replay_after_cross_root_submit_restores_drafts() {
    let mut rs = rec_at("m@r1/s1");
    goto(&mut rs, "m@r2/s1");
    rs.draft("二".to_string());
    goto(&mut rs, "m@r1/s1");
    rs.draft("一".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    let start = Session::entered_at(ir(), "m@r1/s1").state().clone();
    let replayed = replay(&ir(), start, rs.ops()).expect("replay");
    assert_eq!(replayed.drafts, rs.session().state().drafts);
    let restored = Session::from_state(ir(), replayed);
    assert_eq!(restored.current_draft(), rs.current_draft());
}

#[derive(Debug, Clone)]
enum Act {
    Draft(String),
    Go(usize),
    Submit(usize),
}

const TEXTS: [&str; 4] = ["/make-note abc", "/nope", "ただの文", "   "];

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        "[a-z ]{0,6}".prop_map(Act::Draft),
        (0..STEPS.len()).prop_map(Act::Go),
        (0..TEXTS.len()).prop_map(Act::Submit),
    ]
}

#[test]
fn property_s7_submit_clears_only_the_sent_root() {
    let crossed = AtomicUsize::new(0);
    check(
        "s7-submit-clears-sent-root",
        DEFAULT_CASES,
        vec(act_strategy(), 0..MAX_OPS),
        |acts| {
            let mut rs = rec_at(STEPS[0]);
            let mut model: BTreeMap<Root, String> = BTreeMap::new();
            for a in acts {
                match a {
                    Act::Draft(t) => {
                        if let Some(r) = current_root(&rs) {
                            model.insert(r, t.clone());
                        }
                        rs.draft(t);
                    }
                    Act::Go(i) => {
                        rs.apply(Op::Control(Control::GoTo(uid(STEPS[i]))));
                    }
                    Act::Submit(i) => {
                        let before = current_root(&rs);
                        if !TEXTS[i].trim().is_empty() {
                            if let Some(r) = &before {
                                model.remove(r);
                            }
                        }
                        rs.submit(PREFIX, TEXTS[i].to_string());
                        if before != current_root(&rs) {
                            crossed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
                prop_assert!(
                    rs.session().state().drafts == model,
                    "drafts differ from model"
                );
                let cur = current_root(&rs);
                prop_assert!(
                    rs.current_draft() == cur.and_then(|r| model.get(&r)).map(String::as_str),
                    "current draft differs from model"
                );
            }
            Ok(())
        },
    );
    assert!(crossed.load(Ordering::Relaxed) > 0, "no cross-root submit");
}
