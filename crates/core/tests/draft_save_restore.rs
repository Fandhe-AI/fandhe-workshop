//! 下書きの保存の口 `RecordedSession::set_draft` と写しへの保持（SAVE-23。TASK-328 1/3。#966）の試験。
//!
//! 版 1 の写しは `drafts`（根 → 文字列）に現在の根の下書きも入れ、`draft` の鍵を持たない。spec の `snapshot.draft` は
//! `current_draft()`、`snapshot.drafts` の他の根は `drafts` から現在の根を除いたものと読む。
//! 本ファイルは、連続する置き換えが最後の 1 つに畳まれること、2 つの根の下書きが写しに残ること、改行・Tab・空白などを含む
//! 下書きが保存（包み → バイト列）→ 復元（`load_history`・`boot_session`）でバイト単位に一致することを固定する。
//! 下書きの値は利用者の入力なので、失敗メッセージには値を出さず、比較だけにとどめる。

mod common;

use common::contract_sample::testdata_dir;
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_history;
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::Session;
use proptest::collection::vec;
use proptest::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

const PREFIX: &str = "/";
const STEPS: [&str; 2] = ["m@r1/s1", "m@r2/s1"];

/// 下書きの種類の表（SAVE-23 の文字の種類）。
const KINDS: [&str; 10] = [
    "\n先頭",
    "末尾\n",
    "a\n\nb",
    "\t",
    "a\tb",
    "   ",
    "\u{3000}",
    "1行目\n2行目\n3行目\n",
    "a\r\nb",
    "",
];

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

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn rec_at(step: &str) -> RecordedSession {
    RecordedSession::new(Session::entered_at(ir(), step), "s-1")
}

fn goto(rs: &mut RecordedSession, step: &str) {
    rs.apply(Op::Control(Control::GoTo(StepUid(step.to_string()))));
}

fn versions(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

fn initial() -> SessionState {
    Session::entered_at(ir(), "m@r1/s1").state().clone()
}

fn saved_bytes(rs: &RecordedSession) -> Vec<u8> {
    export_bytes(&rs.envelope_for_save().expect("envelope")).expect("export")
}

fn load_drafts(bytes: &[u8]) -> BTreeMap<Root, String> {
    let ir = ir();
    let Opened::Open { state } = load_history(
        bytes,
        &ir,
        &versions(&ir),
        &OpeningCaps::current(),
        initial(),
    ) else {
        panic!("history must open");
    };
    state.drafts
}

fn boot(bytes: &[u8]) -> RecordedSession {
    let ir = ir();
    let v = versions(&ir);
    let BootOutcome::Restored { session, .. } = boot_session(
        Some(bytes),
        ir,
        &v,
        &OpeningCaps::current(),
        initial(),
        "s-1",
    ) else {
        panic!("must restore");
    };
    session
}

fn draft_ops(rs: &RecordedSession) -> Vec<String> {
    rs.ops()
        .iter()
        .filter_map(|o| match o {
            HistoryOp::Draft(t) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn set_draft_folds_consecutive_replacements_to_last() {
    let mut rs = rec_at("m@r1/s1");
    rs.set_draft("一".to_string());
    rs.set_draft("一二".to_string());
    rs.set_draft("一二三".to_string());
    assert_eq!(draft_ops(&rs), vec!["一二三".to_string()]);
    assert_eq!(rs.current_draft(), Some("一二三"));
    // 対照: 再読み込みを挟むと畳まない。
    rs.reload();
    rs.set_draft("四".to_string());
    assert_eq!(draft_ops(&rs), vec!["一二三".to_string(), "四".to_string()]);
}

#[test]
fn set_draft_holds_current_and_other_root_drafts() {
    let mut rs = rec_at("m@r1/s1");
    rs.set_draft("一".to_string());
    goto(&mut rs, "m@r2/s1");
    rs.set_draft("二".to_string());
    let state = rs.session().state();
    assert_eq!(rs.current_draft(), Some("二"), "snapshot.draft");
    let current = state.draft_root().expect("current root");
    let others: Vec<(&Root, &String)> = state
        .drafts
        .iter()
        .filter(|(r, _)| **r != current)
        .collect();
    assert_eq!(others, vec![(&root("m@r1"), &"一".to_string())]);
}

#[test]
fn exported_snapshot_keeps_drafts_and_has_no_draft_key() {
    let mut rs = rec_at("m@r1/s1");
    rs.set_draft("一\n".to_string());
    goto(&mut rs, "m@r2/s1");
    rs.set_draft("\t二".to_string());
    let v: Value = serde_json::from_slice(&saved_bytes(&rs)).expect("json");
    let snap = &v["snapshot"];
    assert_eq!(snap["drafts"]["m@r1"], json!("一\n"));
    assert_eq!(snap["drafts"]["m@r2"], json!("\t二"));
    assert!(snap.get("draft").is_none(), "no draft key");
    assert!(snap.get("draft_root").is_none(), "no draft_root key");
    let ops = v["ops"].as_array().expect("ops");
    assert_eq!(ops.last().and_then(|o| o["draft"].as_str()), Some("\t二"));
}

#[test]
fn drafts_round_trip_byte_exact_through_load_and_boot() {
    let mut rs = rec_at("m@r1/s1");
    for (i, k) in KINDS.iter().enumerate() {
        // 現在の根では連続の置き換えを含める（最後の 1 つだけが残る）。
        rs.set_draft(format!("途中{i}"));
        rs.set_draft((*k).to_string());
        goto(&mut rs, STEPS[(i + 1) % 2]);
    }
    let before = rs.session().state().drafts.clone();
    assert_eq!(before.len(), 2);
    let bytes = saved_bytes(&rs);
    assert!(load_drafts(&bytes) == before, "load_history drafts differ");
    let mut restored = boot(&bytes);
    assert!(
        restored.session().state().drafts == before,
        "boot drafts differ"
    );
    assert_eq!(restored.current_draft(), rs.current_draft());
    // 復元の後の置き換えは Reload の後に追記され、畳まれない。
    let n = draft_ops(&restored).len();
    restored.set_draft("続き".to_string());
    assert_eq!(draft_ops(&restored).len(), n + 1);
}

#[test]
fn each_kind_round_trips_alone() {
    for k in KINDS {
        let mut rs = rec_at("m@r1/s1");
        rs.set_draft(k.to_string());
        let restored = boot(&saved_bytes(&rs));
        assert!(restored.current_draft() == Some(k), "kind differs");
    }
}

#[test]
fn resubmitted_draft_after_submit_is_restored() {
    let mut rs = rec_at("m@r1/s1");
    rs.set_draft("/nope".to_string());
    rs.submit(PREFIX, "/nope".to_string());
    assert_eq!(rs.current_draft(), None, "submit clears the root");
    rs.set_draft("/nope".to_string());
    let restored = boot(&saved_bytes(&rs));
    assert_eq!(restored.current_draft(), Some("/nope"));
    // 対照: 送信だけでは、その根の欄が無い。
    let mut only = rec_at("m@r1/s1");
    only.set_draft("/nope".to_string());
    only.submit(PREFIX, "/nope".to_string());
    let restored = boot(&saved_bytes(&only));
    assert_eq!(restored.current_draft(), None);
}

#[test]
fn draft_alias_matches_set_draft() {
    let mut a = rec_at("m@r1/s1");
    let mut b = rec_at("m@r1/s1");
    for (i, k) in KINDS.iter().enumerate() {
        a.draft((*k).to_string());
        b.set_draft((*k).to_string());
        goto(&mut a, STEPS[(i + 1) % 2]);
        goto(&mut b, STEPS[(i + 1) % 2]);
    }
    assert_eq!(a.ops(), b.ops());
    assert_eq!(a.session().state(), b.session().state());
}

#[derive(Debug, Clone)]
enum Act {
    Draft(String),
    Go(usize),
    Reload,
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        "[a-z \\t\\n\\r\u{3000}]{0,8}".prop_map(Act::Draft),
        (0..STEPS.len()).prop_map(Act::Go),
        Just(Act::Reload),
    ]
}

#[test]
fn property_set_draft_round_trip() {
    check(
        "set-draft-round-trip",
        DEFAULT_CASES,
        vec(act_strategy(), 1..MAX_OPS),
        |acts| {
            let mut rs = rec_at(STEPS[0]);
            let mut model: BTreeMap<Root, String> = BTreeMap::new();
            for a in acts {
                match a {
                    Act::Draft(t) => {
                        if let Some(r) = rs.session().state().draft_root() {
                            model.insert(r, t.clone());
                        }
                        rs.set_draft(t);
                    }
                    Act::Go(i) => goto(&mut rs, STEPS[i]),
                    Act::Reload => {
                        rs.reload();
                    }
                }
            }
            prop_assert!(
                rs.session().state().drafts == model,
                "drafts differ from model"
            );
            prop_assert!(
                !rs.ops()
                    .windows(2)
                    .any(|w| matches!(w, [HistoryOp::Draft(_), HistoryOp::Draft(_)])),
                "adjacent Draft ops"
            );
            let bytes = saved_bytes(&rs);
            prop_assert!(load_drafts(&bytes) == model, "restored drafts differ");
            Ok(())
        },
    );
}
