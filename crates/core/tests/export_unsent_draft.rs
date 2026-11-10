//! 書き出しに未送信の下書きが入るか（SAVE-18・D-52・D-111。TASK-329）の判定の試験。
//!
//! 確かめるのは、(1) 現在の根の下書き・別の根に残った下書き・現在の Step が無い間の下書きで真、(2) 同じ値の送信の後・
//! 空文字列で偽、(3) 包み・バイト列の版が同じ答えを返し、読めない形は真（fail-closed）、(4) 書き出したファイルに下書きが
//! 実際に入ること、(5) 契約の試料で一致すること、(6) 打った後に消した・書き換えた下書きが `Engine`・`Reload` を挟んで
//! `ops` に残ると、入力欄が空でも真（D-111）で、挟んでも同じ値を送った下書きは偽、(7) 乱数の列で、spec の SAVE-18 の
//! 読み（直後の文字列の操作が同じ値の `Submit` でない空でない `Draft`）をそのまま書いた模型と一致すること、の 7 点である。
//! 下書きは利用者の入力なので、失敗の文には値を出さない。

mod common;

use std::collections::BTreeMap;

use common::contract_sample::testdata_dir;
use common::history_sample::{load, SAMPLES};
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::history::{HistoryOp, EXPORT_INCLUDES_DRAFT};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, StepUid, TickAddr};
use fandhe_workshop_core::history_export::{
    bytes_include_unsent_draft, envelope_includes_unsent_draft, export_bytes, includes_unsent_draft,
};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::Session;
use proptest::prelude::*;
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
fn submit_of_the_same_value_clears_it() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("/make-note abc".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert!(!verdict(&rs));
}

/// 記録の口は送信の本文を下書きと別に受け取る。違う本文を送ったときの直前の下書きは送っていない値として数える
/// （spec の SAVE-18 の【推論】。値で比べる）。
#[test]
fn submit_of_another_value_leaves_the_draft_unsent() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert!(
        rs.current_draft().is_none_or(str::is_empty),
        "control: current draft is empty"
    );
    assert!(verdict(&rs));
}

#[test]
fn empty_draft_is_false_and_blank_draft_is_true() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft(String::new());
    assert!(!verdict(&rs));
    rs.draft("   ".to_string());
    assert!(verdict(&rs), "blank is written into the file");
}

/// 写しの規則 (1) だけが効く形にする。r1 の下書きの直後の文字列の操作は同じ値の送信（r2 で送る）で、`ops` の規則では偽。
/// 送信は送った時点の根（r2）の下書きだけを写しから除くため、r1 の値は写しに残る。
#[test]
fn draft_left_in_another_root_counts_while_current_draft_is_empty() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("/make-note abc".to_string());
    goto(&mut rs, "m@r2/s1");
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert_eq!(current_root(&rs), Some(root("main")));
    assert!(
        rs.current_draft().is_none_or(str::is_empty),
        "control: current draft is empty"
    );
    assert!(
        !includes_unsent_draft(rs.ops(), &BTreeMap::new()),
        "control: the ops rule alone is false"
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

fn stale_tick() -> Op {
    Op::Tick(TickAddr {
        generation: Generation(99),
        step: uid("m@r1/s1"),
        index: 0,
    })
}

/// 下書きと下書きの間に挟むエンジンの操作 3 種（進行・ポインタの操作・チェックポイントの保存）。
fn engine_ops_between() -> [Op; 3] {
    [
        stale_tick(),
        Op::Input(Input::Ui {
            view: "main".to_string(),
            path: "notes".to_string(),
        }),
        Op::Control(Control::Save),
    ]
}

/// 写しに空でない下書きが無いことと、`ops` に値が残っていることの対照。
fn assert_only_ops_hold(rs: &RecordedSession, value: &str, label: &str) {
    assert!(
        rs.session().state().drafts.values().all(String::is_empty),
        "{label}: control: no non-empty draft in the snapshot"
    );
    assert!(
        rs.current_draft().is_none_or(str::is_empty),
        "{label}: control: current draft is empty"
    );
    assert!(
        rs.ops()
            .iter()
            .any(|op| matches!(op, HistoryOp::Draft(t) if t == value)),
        "{label}: control: the value stays in ops"
    );
}

#[test]
fn erased_draft_left_in_ops_by_an_engine_op_is_true() {
    for (i, op) in engine_ops_between().into_iter().enumerate() {
        let label = format!("engine op #{i}");
        let mut rs = rec_at("m@r1/s1");
        rs.draft("memo draft text".to_string());
        rs.apply(op);
        rs.draft(String::new());
        assert_only_ops_hold(&rs, "memo draft text", &label);
        assert!(verdict(&rs), "{label}: erased draft stays in ops");
    }
}

#[test]
fn erased_draft_left_in_ops_by_a_reload_is_true() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    rs.reload();
    rs.draft(String::new());
    assert_only_ops_hold(&rs, "memo draft text", "reload");
    assert!(verdict(&rs));
}

/// 連続する下書きは畳まれ、消した値が `ops` に残らない（負の対照）。
#[test]
fn erased_draft_without_anything_between_is_false() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("memo draft text".to_string());
    rs.draft(String::new());
    assert!(
        !rs.ops()
            .iter()
            .any(|op| matches!(op, HistoryOp::Draft(t) if !t.is_empty())),
        "control: collapsed"
    );
    assert!(!verdict(&rs));
}

/// spec の SAVE-18 の読みの例。書き換える前の `"ab"` も送っていない値として数える（案内が多めに出る側）。
#[test]
fn rewritten_draft_before_the_sent_one_is_true() {
    let mut rs = rec_at("m@r1/s1");
    rs.draft("/make-note ab".to_string());
    rs.apply(stale_tick());
    rs.draft("/make-note abc".to_string());
    rs.submit(PREFIX, "/make-note abc".to_string());
    assert!(verdict(&rs));
}

#[test]
fn submitted_draft_with_an_engine_op_between_is_false() {
    for (i, op) in engine_ops_between().into_iter().enumerate() {
        let label = format!("engine op #{i}");
        let mut rs = rec_at("m@r1/s1");
        rs.draft("/make-note abc".to_string());
        rs.apply(op);
        rs.submit(PREFIX, "/make-note abc".to_string());
        assert!(!verdict(&rs), "{label}: sent before");
        rs.apply(stale_tick());
        rs.draft(String::new());
        assert!(!verdict(&rs), "{label}: empty after the send");
    }
}

#[test]
fn unreadable_or_misshapen_bytes_are_true() {
    for bytes in [
        &b"not json"[..],
        b"{}",
        b"{\"snapshot\":{\"drafts\":[]},\"ops\":[]}",
        b"{\"snapshot\":{\"drafts\":{\"a\":1}},\"ops\":[]}",
        b"{\"snapshot\":{\"drafts\":{}},\"ops\":{}}",
        b"{\"snapshot\":{\"drafts\":{}},\"ops\":[{\"draft\":1}]}",
        b"{\"snapshot\":{\"drafts\":{}},\"ops\":[{\"draft\":\"x\"},{\"submit\":1}]}",
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

/// 乱数の列の 1 件。値は少ない字母から選び、同じ値の送信と違う値の送信がどちらも起きるようにする。
#[derive(Debug, Clone)]
enum Act {
    Draft(String),
    Submit(String),
    Tick,
    Reload,
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        "[ab]{0,2}".prop_map(Act::Draft),
        "[ab]{1,2}".prop_map(Act::Submit),
        Just(Act::Tick),
        Just(Act::Reload),
    ]
}

fn to_ops(acts: Vec<Act>) -> Vec<SessionOp> {
    acts.into_iter()
        .map(|a| match a {
            Act::Draft(t) => HistoryOp::Draft(t),
            Act::Submit(t) => HistoryOp::Submit(t),
            Act::Tick => HistoryOp::Engine(stale_tick()),
            Act::Reload => HistoryOp::Reload,
        })
        .collect()
}

/// spec の SAVE-18 の読みを、空でない `Draft` ごとに直後の文字列の操作を探す形でそのまま書いた模型。
fn model(ops: &[SessionOp]) -> bool {
    ops.iter().enumerate().any(|(i, op)| {
        let HistoryOp::Draft(v) = op else {
            return false;
        };
        if v.is_empty() {
            return false;
        }
        let next = ops[i + 1..]
            .iter()
            .find(|o| matches!(o, HistoryOp::Draft(_) | HistoryOp::Submit(_)));
        !matches!(next, Some(HistoryOp::Submit(s)) if s == v)
    })
}

/// 以前の規則（`ops` の最後の文字列の操作が空でない `Draft`）。新しい規則はこれを特別な場合として含む。
fn last_string_op_is_nonempty_draft(ops: &[SessionOp]) -> bool {
    ops.iter()
        .rev()
        .find_map(|op| match op {
            HistoryOp::Draft(t) => Some(!t.is_empty()),
            HistoryOp::Submit(_) => Some(false),
            HistoryOp::Reload | HistoryOp::Engine(_) => None,
        })
        .unwrap_or(false)
}

#[test]
fn property_ops_rule_matches_the_spec_reading() {
    check(
        "export_unsent_draft_matches_the_spec_reading",
        DEFAULT_CASES,
        proptest::collection::vec(act(), 0..=MAX_OPS),
        |acts| {
            let ops = to_ops(acts);
            let got = includes_unsent_draft(&ops, &BTreeMap::new());
            prop_assert_eq!(got, model(&ops));
            if last_string_op_is_nonempty_draft(&ops) {
                prop_assert!(got, "the old rule is a special case");
            }
            let bytes = serde_json::to_vec(&json!({"snapshot": {"drafts": {}}, "ops": ops}))
                .expect("serialize");
            prop_assert_eq!(bytes_include_unsent_draft(&bytes), got);
            Ok(())
        },
    );
}
