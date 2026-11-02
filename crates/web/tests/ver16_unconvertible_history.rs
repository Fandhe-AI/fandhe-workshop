//! 変換できない履歴の理由つき表示（VER-16・TASK-184 2/2）を、`ir_format` 2 の教材で、ホストの層で確かめる試験。
//!
//! 版 1 で書いた履歴を `ir_format` 2 の教材で開くと、エンジンは変換せず理由つきで表示だけにする。
//! core の `history_open.rs`（`ver16_*`）は判定だけを、`view_only_fallback.rs` は `ir_format` 0 の作り物での
//! ホストの動作を固定する。ここでは実在する版の組（1 と 2）で、読み込み（`open_history`）と起動時の復元（`boot`）の
//! 両方について、通知の理由の文・操作の凍結・書き出しの一致と、変換して開く／開かない対照を固定する。
//! pack の凍結物ではなく `min-3step` の変種を使う（web は pack に依存できない）。
//! 失敗の文には理由コード・真偽・件数だけを出す（SAFE-40）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::shell;
use fandhe_workshop_web::view_only_notice::view_only_reason_text;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の教材の版を書き換えた IR の JSON。形式 2 は `format_version` と `pack.ir_format` の両方を 2 にする。
fn ir_json(version: &str, ir_format: u64) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    if ir_format == 2 {
        v["format_version"] = serde_json::json!(2);
        v["pack"]["ir_format"] = serde_json::json!(2);
    }
    serde_json::to_vec(&v).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn host(version: &str, ir_format: u64) -> EngineHost {
    let ir = load_ir(&ir_json(version, ir_format)).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

/// `version`・`ir_format` の教材で書いた履歴（`ops` は空でない）。
fn history(version: &str, ir_format: u64) -> Vec<u8> {
    let ir = load_ir(&ir_json(version, ir_format)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn notice(h: &EngineHost) -> String {
    match h
        .registry()
        .bound_value("view_only_notice", h.model())
        .unwrap()
    {
        BoundValue::Text(t) => t,
        other => panic!("view_only_notice の束縛が文字列でない: {other:?}"),
    }
}

fn snapshot(h: &EngineHost) -> (String, usize, String) {
    let s = h.model().session().unwrap();
    (
        serde_json::to_string(s.state()).unwrap(),
        s.log().len(),
        h.draft().to_owned(),
    )
}

fn mutating_actions() -> Vec<(&'static str, HostAction)> {
    vec![
        ("draft", act("draft", "/make-note first-note")),
        ("submit", act("submit", "")),
        ("esc", act("esc", "Escape")),
        ("stop-button", act("stop", "")),
        ("complete", act("complete", "")),
        ("raw-save", HostAction::Engine(Op::Control(Control::Save))),
        ("raw-stop", HostAction::Engine(Op::Control(Control::Stop))),
    ]
}

fn assert_frozen(h: &mut EngineHost) {
    let before = snapshot(h);
    for (name, a) in mutating_actions() {
        h.update(a);
        assert_eq!(snapshot(h), before, "{name} が状態・ログ・下書きを変えた");
        assert!(h.history_to_save().is_none(), "{name} の後に保存の値が出た");
        assert!(h.next_tick().is_none(), "{name} の後に進行の宛先が出た");
    }
}

fn operate(h: &mut EngineHost) {
    let before = snapshot(h);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_ne!(snapshot(h), before, "対照が変化を起こさない");
    assert!(h.history_to_save().is_some());
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    if let Node::Element { children, .. } = n {
        for c in children {
            find(c, testid, out);
        }
    }
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(text_of).collect(),
        _ => String::new(),
    }
}

fn reason_sentence() -> &'static str {
    let t = view_only_reason_text(LoadReason::FormatOlder).expect("format_older の理由の文");
    assert!(!t.is_empty(), "format_older の理由の文が空");
    t
}

#[test]
fn ver16_v1_history_on_v2_pack_is_view_only_with_reason() {
    let mut h = host("1.0.0", 2);
    let bytes = history("1.0.0", 1);
    let log_before = h.model().session().unwrap().log().len();
    assert_eq!(
        h.open_history(&bytes),
        ImportOutcome::ViewOnly(LoadReason::FormatOlder)
    );
    assert_eq!(h.model().view_only_reason(), Some(LoadReason::FormatOlder));
    assert!(h.dirty_fields().contains(&"view_only_notice"));
    let n = notice(&h);
    assert!(n.contains("format_older"));
    assert!(n.contains(reason_sentence()));
    assert_eq!(h.model().session().unwrap().log().len(), log_before);
    assert_frozen(&mut h);
    assert_eq!(h.export_live(), ExportGate::Ready(bytes));
}

#[test]
fn ver16_boot_with_v1_history_on_v2_pack_is_view_only_with_reason() {
    let mut h = host("1.0.0", 2);
    let bytes = history("1.0.0", 1);
    assert_eq!(h.boot(Some(&bytes)), BootKind::NotOpened);
    assert_eq!(h.model().view_only_reason(), Some(LoadReason::FormatOlder));
    assert!(notice(&h).contains(reason_sentence()));
    assert!(h.history_to_save().is_none());
    assert_frozen(&mut h);
    assert_eq!(h.export_live(), ExportGate::Ready(bytes));
}

#[test]
fn ver16_not_converted_even_when_replay_would_match() {
    let ir2 = load_ir(&ir_json("1.0.0", 2)).unwrap();
    let bytes = history("1.0.0", 1);
    let env = parse_history_envelope(parse_history_json(&bytes).unwrap()).unwrap();
    assert!(!env.ops.is_empty());
    let replayed = replay(&ir2, initial(), &env.ops).unwrap();
    assert_eq!(replayed, env.snapshot, "前提: 変換は自明に可能");
    let mut h = host("1.0.0", 2);
    assert_eq!(
        h.open_history(&bytes),
        ImportOutcome::ViewOnly(LoadReason::FormatOlder)
    );
}

#[test]
fn ver16_contrast_same_format_content_patch_is_converted() {
    for ir_format in [1u64, 2] {
        let mut h = host("1.0.1", ir_format);
        assert_eq!(
            h.open_history(&history("1.0.0", ir_format)),
            ImportOutcome::Converted,
            "ir_format {ir_format}"
        );
        assert_eq!(h.model().view_only_reason(), None);
        assert_eq!(notice(&h), "");
        operate(&mut h);
    }
}

#[test]
fn ver16_contrast_v2_history_on_v1_pack_is_refused() {
    let mut h = host("1.0.0", 1);
    let before = snapshot(&h);
    assert_eq!(
        h.open_history(&history("1.0.0", 2)),
        ImportOutcome::Refused(LoadReason::FormatNewer)
    );
    assert_eq!(h.model().view_only_reason(), None);
    assert_eq!(snapshot(&h), before);
    let view = h.view();
    let mut items = Vec::new();
    find(&view, shell::NOTICE_ITEM_TESTID, &mut items);
    assert_eq!(items.len(), 1);
    assert_eq!(attr(items[0], "data-notice-reason"), Some("format_newer"));
    assert!(!text_of(items[0]).contains(reason_sentence()));
}

#[test]
fn ver16_contrast_same_format_opens_and_operates() {
    let mut h = host("1.0.0", 2);
    assert_eq!(h.open_history(&history("1.0.0", 2)), ImportOutcome::Open);
    assert_eq!(h.model().view_only_reason(), None);
    operate(&mut h);
}
