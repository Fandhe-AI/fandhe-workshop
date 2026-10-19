//! 版の違う履歴の読み込みで使う、開く側の条件の上書きと観察の口の試験（VER-2、TASK-175 2/2）。
//!
//! 開く側のエンジンの版・語彙の上限を試験の設定で上書きできること、上書きが無ければ配信と同じ判定になること、
//! 開かない履歴では実行環境を作り直さない（差し替えの回数が増えない）ことを確かめる。15 件の全体の照合は
//! ブラウザ試験（`tools/browser/version-histories.test.mjs`）が行い、ここは口そのものの native の固定である。
//! 試験の設定（feature `test-hooks`）のときだけ流れる。試験では `unwrap` を使ってよい。
#![cfg(feature = "test-hooks")]

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 語彙 1.2 の教材（V-iv-2 の `base` と同じ版の欄）。
fn ir_json() -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!("1.0.0");
    v["pack"]["vocab"] = serde_json::json!("1.2");
    serde_json::to_vec(&v).unwrap()
}

fn ir() -> Ir {
    load_ir(&ir_json()).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn host() -> EngineHost {
    EngineHost::with_session(Session::from_state(ir(), initial()), standard_registry()).unwrap()
}

/// 開ける履歴を、履歴の側のエンジンの版を書き換えて返す（`None` なら書き換えない）。
fn history(engine: Option<&str>) -> Vec<u8> {
    let ir = ir();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&export_bytes(&built).unwrap()).unwrap();
    if let Some(e) = engine {
        v["versions"]["engine"] = serde_json::json!(e);
    }
    serde_json::to_vec(&v).unwrap()
}

fn send(h: &mut EngineHost, payload: &str) {
    if let Some(action) = EngineHost::decode_action("raw", payload) {
        h.update(action);
    }
}

fn engine_json(h: &EngineHost) -> serde_json::Value {
    serde_json::from_str(&h.engine_snapshot_json()).unwrap()
}

fn open(h: &mut EngineHost, bytes: &[u8]) {
    let body = String::from_utf8(bytes.to_vec()).unwrap();
    send(h, &format!("open-history:{body}"));
}

#[test]
fn without_an_override_the_judgment_is_the_delivery_one() {
    // 履歴の側のエンジンが古くても、上書きが無ければ開く側は現行の版のまま。理由は空である。
    let mut h = host();
    open(&mut h, &history(Some("0.0.0")));
    assert!(matches!(
        h.last_import(),
        Some(ImportOutcome::Open | ImportOutcome::Converted)
    ));
    assert_eq!(
        engine_json(&h)["lastImport"]["reason"],
        serde_json::Value::Null
    );
}

#[test]
fn an_engine_override_turns_the_judgment_into_a_conversion_with_the_engine_axis() {
    let mut h = host();
    send(
        &mut h,
        r#"opening-override:{"engine":"0.1.0","vocab_max":null}"#,
    );
    open(&mut h, &history(Some("0.0.0")));
    assert_eq!(h.last_import(), Some(ImportOutcome::Converted));
    let e = engine_json(&h);
    assert_eq!(e["lastImport"]["axes"], serde_json::json!(["engine"]));
    assert_eq!(e["installCount"], 1);
    assert!(h.last_import_state_json().is_some());
}

#[test]
fn a_major_engine_override_is_view_only_and_does_not_replace_the_session() {
    let mut h = host();
    send(
        &mut h,
        r#"opening-override:{"engine":"1.0.0","vocab_max":null}"#,
    );
    open(&mut h, &history(Some("0.0.0")));
    assert_eq!(
        h.last_import(),
        Some(ImportOutcome::ViewOnly(LoadReason::EngineMajor))
    );
    let e = engine_json(&h);
    assert_eq!(e["lastImport"]["reason"], "engine_major");
    assert_eq!(e["lastImport"]["axes"], serde_json::json!([]));
    assert_eq!(e["installCount"], 0, "a closed result builds no session");
    assert!(h.last_import_state_json().is_none());
}

#[test]
fn a_low_vocab_ceiling_refuses_a_newer_vocabulary() {
    let mut h = host();
    send(
        &mut h,
        r#"opening-override:{"engine":null,"vocab_max":"1.0"}"#,
    );
    open(&mut h, &history(None));
    assert_eq!(
        h.last_import(),
        Some(ImportOutcome::Refused(LoadReason::VocabNewer))
    );
    assert_eq!(engine_json(&h)["installCount"], 0);
}

#[test]
fn clearing_the_override_restores_the_delivery_judgment() {
    let mut h = host();
    send(
        &mut h,
        r#"opening-override:{"engine":null,"vocab_max":"1.0"}"#,
    );
    send(
        &mut h,
        r#"opening-override:{"engine":null,"vocab_max":null}"#,
    );
    open(&mut h, &history(None));
    assert_eq!(h.last_import(), Some(ImportOutcome::Open));
}

#[test]
fn a_malformed_override_changes_nothing() {
    for p in [
        r#"opening-override:{"engine":"x.y","vocab_max":null}"#,
        r#"opening-override:{"engine":"0.1.0","extra":1}"#,
        r#"opening-override:{"engine":1}"#,
        "opening-override:not json",
        "opening-override:[]",
    ] {
        assert!(
            !matches!(
                EngineHost::decode_action("raw", p),
                Some(HostAction::OpeningOverride(_))
            ),
            "{p}"
        );
    }
}

#[test]
fn load_ir_replaces_the_pack_and_a_broken_body_changes_nothing() {
    let mut h = host();
    let before = engine_json(&h)["installCount"].clone();
    send(&mut h, "load-ir:not json");
    send(&mut h, r#"load-ir:{"nodes":[]}"#);
    assert_eq!(engine_json(&h)["installCount"], before);
    let body = String::from_utf8(ir_json()).unwrap();
    send(&mut h, &format!("load-ir:{body}"));
    assert_eq!(engine_json(&h)["installCount"], 1);
}
