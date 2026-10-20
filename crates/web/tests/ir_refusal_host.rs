//! 旧いエンジンが新しい教材（IR）を理由つきで断り、履歴を書き換えないことの native 試験（VER-13。TASK-181 2/2）。
//!
//! ブラウザで動く今のエンジンを、より新しい教材に対する旧いエンジンとみなす（実物の旧版のビルドは repo に無い。
//! 1 件目の `crates/pack/tests/ir_old_engine.rs` と同じ立場）。ここでは、未来の IR を読めなかったホストが
//! 通知（コードだけ）を描き、保存先を開かず何も書かないこと、起動後の読み込みの拒否が実行中の状態を変えないことを固定する。
//! 変異 `VER13-3`〜`VER13-5` の拠り所になる。試験では `unwrap` を使ってよい。
#![cfg(feature = "test-hooks")]

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::storage::DeliveryScope;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, IrLoadError, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::ir_notice::ir_notice_text;

const SENTINEL: &str = "FUTURE-SENTINEL-V3-X";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn min3() -> Vec<u8> {
    std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json")).unwrap()
}

/// 既定の教材から、旧いエンジンが読めない IR を作る。`format` は版も 3 にする。
fn future_ir(format: bool, pack_only: bool) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&min3()).unwrap();
    let obj = v.as_object_mut().unwrap();
    if format {
        if !pack_only {
            obj.insert("format_version".to_owned(), 3.into());
        }
        obj["pack"]
            .as_object_mut()
            .unwrap()
            .insert("ir_format".to_owned(), 3.into());
    }
    obj.insert("future_v3_x".to_owned(), SENTINEL.into());
    serde_json::to_vec(&v).unwrap()
}

fn refusal(bytes: &[u8]) -> IrLoadError {
    load_ir(bytes).unwrap_err()
}

fn default_host() -> EngineHost {
    let ir = load_ir(&min3()).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn snapshot(h: &EngineHost) -> serde_json::Value {
    serde_json::from_str(&h.engine_snapshot_json()).unwrap()
}

#[test]
fn future_format_ir_is_refused_as_format_newer() {
    let e = refusal(&future_ir(true, false));
    assert_eq!(e, IrLoadError::FormatVersion(3));
    assert_eq!(e.reason_code(), "format_newer");
    let e = refusal(&future_ir(true, true));
    assert_eq!(e.reason_code(), "format_newer");
}

#[test]
fn future_field_ir_is_refused_as_malformed() {
    let e = refusal(&future_ir(false, false));
    assert!(matches!(e, IrLoadError::Json(_)));
    assert_eq!(e.reason_code(), "malformed");
}

#[test]
fn refused_host_shows_the_notice_with_the_code_only() {
    let h = EngineHost::refused(&refusal(&future_ir(true, false)));
    assert_eq!(h.ir_refusal(), Some("format_newer"));
    let html = render_component_html(&h);
    assert!(html.contains("data-testid=\"ir-notice\""), "{html}");
    assert!(html.contains("role=\"status\""), "{html}");
    assert!(html.contains(&ir_notice_text("format_newer")), "{html}");
    assert_eq!(
        ir_notice_text("format_newer"),
        "教材を読み込めませんでした。（format_newer）"
    );
    assert!(!html.contains(SENTINEL));
    assert!(!html.contains("draft-input"));
    assert!(!html.contains("raw-trigger"));
    // 入力欄が無く Esc の宛先が無いので、root に Esc の属性は付けない。
    assert!(!html.contains("data-action-keydown"), "{html}");
    assert_eq!(snapshot(&h)["irRefusal"], "format_newer");
}

#[test]
fn refused_host_has_no_history_key_and_saves_nothing() {
    let mut h = EngineHost::refused(&refusal(&future_ir(true, false)));
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
    assert!(h.history_to_save().is_none());
    // 保存済みの履歴があっても開かない（読みも書きもしない）。
    assert_eq!(h.boot(Some(b"{}")), BootKind::NoSession);
    assert!(h.history_to_save().is_none());
}

#[test]
fn refused_ir_at_runtime_keeps_the_session() {
    let mut h = default_host();
    let before = snapshot(&h);
    assert_eq!(before["irRefusal"], serde_json::Value::Null);
    let html_before = render_component_html(&h);
    let mut payload = String::from("load-ir:");
    payload.push_str(std::str::from_utf8(&future_ir(true, false)).unwrap());
    h.update(EngineHost::decode_action("raw", &payload).unwrap());
    let after = snapshot(&h);
    assert_eq!(after["irRefusal"], "format_newer");
    for key in ["stateWithoutLog", "logLen", "installCount", "irLoaded"] {
        assert_eq!(before[key], after[key], "{key}");
    }
    assert_eq!(html_before, render_component_html(&h));
    assert!(matches!(
        EngineHost::decode_action("raw", &payload),
        Some(HostAction::LoadIr(_))
    ));
}

#[test]
fn readable_ir_has_no_notice() {
    let h = default_host();
    assert_eq!(h.ir_refusal(), None);
    let html = render_component_html(&h);
    assert!(!html.contains("ir-notice"));
    assert!(html.contains("draft-input"));
    assert_eq!(snapshot(&h)["irRefusal"], serde_json::Value::Null);
}
