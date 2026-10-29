//! 表示だけの 5 件のうち `content_major` 以外の理由コードでも、ホストの表示だけの経路が同じに働くことの試験（VER-9・VER-10。TASK-178 2/2）。
//!
//! `view_only.rs` は `content_major` だけを見る。ここでは `format_older`（履歴の `ir_format` が古い）と `replay_diverged`
//! （副版で Step を足した教材で開く）について、読み込み（`open_history`）と起動時の復元（`boot`）の両方で、
//! (1) 理由のコードが通知に出る、(2) 操作が状態・ログ・下書き・保存の値を変えない、(3) 書き出しが読み込んだバイト列と一致する、ことを固定する。
//! `engine_major` は開く側のエンジンの版の上書き（test-hooks）が要り、`make test` では流れないため扱わない。
//! core の `version_histories.rs` とブラウザの `view-only-fallback.test.mjs` が覆う。
//! 失敗の文には理由コード・真偽・件数だけを出す。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

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

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の教材の `pack.version` を書き換えた IR の JSON。語彙は版の違う履歴の試験と同じ 1.2 にそろえる。
fn ir_json(version: &str) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    serde_json::to_vec(&v).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

/// `1.2.0` の教材。`st-1` と `st-2` の間に Step を足す（core の `StepAdded120` と同じ編集）。
fn step_added_ir_json() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&ir_json("1.2.0")).unwrap();
    let nodes = v["nodes"].as_array_mut().unwrap();
    let mut added = nodes[1].clone();
    added["uid"] = serde_json::json!("main/st-1b");
    added["step"]["id"] = serde_json::json!("st-1b");
    added["step"]["next"] = serde_json::json!([{ "to": "main/st-2", "when": null }]);
    nodes[0]["step"]["next"] = serde_json::json!([{ "to": "main/st-1b", "when": null }]);
    nodes.insert(1, added);
    serde_json::to_vec(&v).unwrap()
}

/// `ir_format` を 0 に書き換えた履歴（core の V-iv-4 と同じ作り物）。
fn format_older_history() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    v["versions"]["ir_format"] = serde_json::json!(0);
    serde_json::to_vec(&v).unwrap()
}

fn host(version: &str) -> EngineHost {
    let ir = load_ir(&ir_json(version)).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

/// `version` の教材で書いた履歴（`ops` は空でない）。
fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
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

/// 状態の全体（JSON）・ログの長さ・下書きの 3 つ組。操作の前後で比べる。
fn snapshot(h: &EngineHost) -> (String, usize, String) {
    let s = h.model().session().unwrap();
    (
        serde_json::to_string(s.state()).unwrap(),
        s.log().len(),
        h.draft().to_owned(),
    )
}

/// 状態・ログを変えうる操作の一式。表示だけの間は、どれも何も変えない。
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

fn step_added_host() -> EngineHost {
    let ir = load_ir(&step_added_ir_json()).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

/// 理由ごとの (名前, 開く側, 履歴, 理由)。
fn cases() -> Vec<(&'static str, EngineHost, Vec<u8>, LoadReason)> {
    vec![
        (
            "format_older",
            host("1.0.0"),
            format_older_history(),
            LoadReason::FormatOlder,
        ),
        (
            "replay_diverged",
            step_added_host(),
            history("1.0.0"),
            LoadReason::ReplayDiverged,
        ),
    ]
}

#[test]
fn 読み込みで表示だけになると_理由ごとに操作は何も変えない() {
    for (name, mut h, bytes, code) in cases() {
        let log_before = h.model().session().unwrap().log().len();
        assert_eq!(
            h.open_history(&bytes),
            ImportOutcome::ViewOnly(code),
            "{name}"
        );
        assert_eq!(h.model().view_only_reason(), Some(code), "{name}");
        assert!(h.dirty_fields().contains(&"view_only_notice"), "{name}");
        assert!(notice(&h).contains(code.as_str()), "{name}");
        assert_eq!(
            h.model().session().unwrap().log().len(),
            log_before,
            "{name}"
        );
        assert_frozen(&mut h);
        assert_eq!(h.export_live(), ExportGate::Ready(bytes), "{name}");
    }
}

#[test]
fn 起動時の復元で表示だけになると_理由ごとに保存の値は出ない() {
    for (name, mut h, bytes, code) in cases() {
        assert_eq!(h.boot(Some(&bytes)), BootKind::NotOpened, "{name}");
        assert_eq!(h.model().view_only_reason(), Some(code), "{name}");
        assert!(notice(&h).contains(code.as_str()), "{name}");
        assert!(h.history_to_save().is_none(), "{name}");
        assert_frozen(&mut h);
        assert_eq!(h.export_live(), ExportGate::Ready(bytes), "{name}");
    }
}

#[test]
fn 対照_同じ教材の版で書いた履歴では同じ操作が状態を変える() {
    let mut h = host("1.0.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    let before = snapshot(&h);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_ne!(snapshot(&h), before, "対照が変化を起こさない");
    assert!(h.history_to_save().is_some());
}
