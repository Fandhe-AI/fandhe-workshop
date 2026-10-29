//! 版つき URL で受講を続けられること（TASK-178 1/2。VER-9・VER-10・DLV-26）の native の試験。
//!
//! 版つき URL は「同じ配信の区間の下に置いた、旧版の配信物の凍結した写し」である。ここでは、その前提の 2 点を
//! ホストの層で固定する。(1) 旧版の URL と最新版の URL は履歴の鍵が一致し、別の先頭区間の URL とは一致しない（D-56）。
//! (2) 同じ履歴を、最新版のホストは表示だけ（保存しない）で開き、旧版のホストは変換なしで開いて続きを保存でき、
//! 続けた履歴を最新版が開いても表示だけのまま壊れない。ブラウザでの往復は `tools/browser/versioned-url.test.mjs`。
//! 失敗の文には件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の `pack.version` だけを変えた IR。同じ `pack.id`・開始点なので、履歴の鍵は版に依らない。
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

fn content_version(bytes: &[u8]) -> String {
    let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    v["versions"]["content"].as_str().unwrap().to_owned()
}

fn op_count(bytes: &[u8]) -> usize {
    let v: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    v["ops"].as_array().unwrap().len()
}

fn exported(h: &mut EngineHost) -> Vec<u8> {
    match h.export_live() {
        ExportGate::Ready(b) => b,
        _ => panic!("書き出しの関門が開かない"),
    }
}

#[test]
fn 旧版の_url_は最新版と同じ鍵で_別の先頭区間とは別の鍵() {
    let key = |p: &str| {
        HistoryKey::for_pack(DeliveryScope::from_pathname(p), "min-3step", "s-1").render()
    };
    assert_eq!(key("/repo/"), key("/repo/v1/"), "版つき URL が別の鍵");
    assert_eq!(key("/repo/"), key("/repo/v1/index.html"));
    assert_ne!(
        key("/repo/"),
        key("/repo-v1/"),
        "対照: 別の先頭区間が同じ鍵"
    );
    assert_ne!(key("/repo/"), key("/"), "対照: 根の配信が同じ鍵");
}

#[test]
fn 最新版は表示だけで開き_保存しない() {
    let saved = history("1.0.0");
    let mut latest = host("2.0.0");
    assert_eq!(latest.boot(Some(&saved)), BootKind::NotOpened);
    assert_eq!(
        latest.model().view_only_reason(),
        Some(LoadReason::ContentMajor)
    );
    latest.update(act("draft", "/make-note first-note"));
    latest.update(act("submit", ""));
    assert!(
        latest.history_to_save().is_none(),
        "表示だけで保存の値が出た"
    );
    assert_eq!(
        exported(&mut latest),
        saved,
        "書き出しが読み込んだ履歴と違う"
    );
}

#[test]
fn 旧版は変換なしで開き_続きを旧版のまま追記して保存する() {
    let saved = history("1.0.0");
    let before_ops = op_count(&saved);
    let mut old = host("1.0.0");
    assert_ne!(
        old.boot(Some(&saved)),
        BootKind::NotOpened,
        "旧版が開かない"
    );
    assert_eq!(old.model().view_only_reason(), None);
    old.update(act("draft", "/make-note first-note"));
    old.update(act("submit", ""));
    assert!(old.history_to_save().is_some(), "続きの保存の値が出ない");
    let continued = exported(&mut old);
    assert_eq!(content_version(&continued), "1.0.0", "教材の版が旧版のまま");
    assert!(op_count(&continued) > before_ops, "ops が増えない");

    // 続けた履歴を最新版が開いても表示だけのままで、書き換えない。
    let mut latest = host("2.0.0");
    assert_eq!(latest.boot(Some(&continued)), BootKind::NotOpened);
    assert_eq!(
        latest.model().view_only_reason(),
        Some(LoadReason::ContentMajor)
    );
    assert_eq!(exported(&mut latest), continued);
}
