//! 書き出しに未送信の下書きが入るか（SAVE-18・D-52。TASK-329）の判定の試験。
//!
//! 確かめるのは、(1) 現在の根の下書き・別の根に残った下書き・現在の Step が無い間の下書きで真、(2) 送信後・空文字列で偽、
//! (3) 包み・バイト列の版が同じ答えを返し、読めない形は真（fail-closed）、(4) 書き出したファイルに下書きが実際に入ること、
//! (5) 契約の試料で一致すること、の 5 点である。下書きは利用者の入力なので、失敗の文には値を出さない。

mod common;

use common::contract_sample::testdata_dir;
use common::history_sample::{load, SAMPLES};
use fandhe_workshop_contract::history::EXPORT_INCLUDES_DRAFT;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::history_export::{
    bytes_include_unsent_draft, envelope_includes_unsent_draft, export_bytes,
};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use serde_json::{json, Value};

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

/// 3 通りの判定（口・包み・バイト列）が同じ答えであることを確かめて、その答えを返す。
fn verdict(rs: &RecordedSession) -> bool {
    let direct = rs.has_unsent_draft();
    let env = rs.envelope_for_save().expect("envelope");
    assert_eq!(envelope_includes_unsent_draft(&env), direct, "envelope");
    let bytes = export_bytes(&env).expect("export");
    assert_eq!(bytes_include_unsent_draft(&bytes), direct, "bytes");
    direct
}

#[test]
fn decision_is_to_include_drafts() {
    const { assert!(EXPORT_INCLUDES_DRAFT) };
}

#[test]
fn nonempty_draft_in_current_root_is_true_and_lands_in_the_file() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    assert!(verdict(&rs));
    let bytes = export_bytes(&rs.envelope_for_save().expect("env")).expect("export");
    let v: Value = serde_json::from_slice(&bytes).expect("json");
    assert_eq!(
        v["ops"].as_array().and_then(|a| a.last()),
        Some(&json!({"draft": "memo draft text"}))
    );
    assert_eq!(v["snapshot"]["drafts"]["m@r1"], json!("memo draft text"));
}

#[test]
fn nothing_recorded_is_false() {
    assert!(!rec_at("m@r1/s1").has_unsent_draft());
}

#[test]
fn submit_clears_it() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert!(!verdict(&rs));
}

#[test]
fn empty_draft_is_false_and_blank_draft_is_true() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft(String::new());
    assert!(!verdict(&rs));
    rs.draft("   ".to_string());
    assert!(verdict(&rs), "blank is written into the file");
}

#[test]
fn draft_left_in_another_root_counts_while_current_draft_is_empty() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    goto(&mut rs, "m@r2/s1");
    rs.draft("other".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert_eq!(current_root(&rs), Some(root("main")));
    assert!(
        rs.current_draft().is_none_or(str::is_empty),
        "control: current draft is empty"
    );
    assert!(verdict(&rs), "the draft of r1 stays in the snapshot");
}

#[test]
fn draft_before_any_step_counts_through_ops_only() {
    let mut rs = RecordedSession::new(Session::new(ir()), "s-1");
    assert!(
        rs.session().state().draft_root().is_none(),
        "control: no current step"
    );
    rs.draft("memo draft text".to_string());
    assert!(
        rs.session().state().drafts.is_empty(),
        "control: not in the snapshot"
    );
    assert!(verdict(&rs), "ops only");
}

#[test]
fn trailing_reload_does_not_hide_the_last_draft() {
    let mut rs = RecordedSession::new(Session::new(ir()), "s-1");
    rs.draft("memo draft text".to_string());
    rs.reload();
    assert!(verdict(&rs));
}

#[test]
fn unreadable_or_misshapen_bytes_are_true() {
    for bytes in [
        &b"not json"[..],
        b"{}",
        b"{\"snapshot\":{\"drafts\":[]},\"ops\":[]}",
        b"{\"snapshot\":{\"drafts\":{\"a\":1}},\"ops\":[]}",
        b"{\"snapshot\":{\"drafts\":{}},\"ops\":{}}",
    ] {
        assert!(bytes_include_unsent_draft(bytes), "fail-closed");
    }
    assert!(!bytes_include_unsent_draft(
        b"{\"snapshot\":{\"drafts\":{}},\"ops\":[]}"
    ));
}

#[test]
fn contract_samples_agree_between_envelope_and_bytes() {
    for name in SAMPLES {
        let env = load(name);
        let bytes = export_bytes(&env).expect("export");
        assert_eq!(
            envelope_includes_unsent_draft(&env),
            bytes_include_unsent_draft(&bytes),
            "sample {name}"
        );
    }
}
