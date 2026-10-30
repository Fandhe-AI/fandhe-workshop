//! 直接リンク `?start=` の開始点の断りが、通知（固定のコードだけ）を描き、保存先に触れないことの native 試験（TASK-109 1/2。DLV-9・SAFE-33）。
//!
//! 開始点の照合の規則は `src/start_select.rs` の単体試験、教材の選択と鍵は `src/test_fixture.rs` の単体試験が持つ。
//! ここでは公開 API（`EngineHost::refused_start`・`with_session_at`）の層で、断ったホストが入力値を持たず
//! 保存済みの履歴を開かないこと、選んだ開始点が鍵・起動の分岐に通ることを固定する。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost};
use fandhe_workshop_web::ir_notice::ir_notice_text;
use fandhe_workshop_web::start_select::{select_start, StartRefusal};

const SENTINEL: &str = "START-SENTINEL-X";

fn data(rel: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

/// 開始点を 2 件持つ IR。
fn two_starts() -> Vec<u8> {
    let mut v: serde_json::Value =
        serde_json::from_slice(&data("tests/data/reload_points_ch2/ir.json")).unwrap();
    v["starts"] = serde_json::json!([
        { "id": "s-1", "node": "main/st-1", "effects": [] },
        { "id": "s-4", "node": "main/st-4", "effects": [] },
    ]);
    serde_json::to_vec(&v).unwrap()
}

fn host_at(start: Option<&str>) -> EngineHost {
    let ir = load_ir(&two_starts()).unwrap();
    let id = select_start(&ir, start).unwrap().id.clone();
    let node = ir.starts.iter().find(|s| s.id == id).unwrap().node.clone();
    let sample: serde_json::Value =
        serde_json::from_slice(&data("../../testdata/contract/event-sequence.json")).unwrap();
    let text = sample["initial"].to_string().replace("main/st-1", &node);
    let state: SessionState = serde_json::from_str(&text).unwrap();
    EngineHost::with_session_at(Session::from_state(ir, state), standard_registry(), id).unwrap()
}

fn key(h: &EngineHost) -> String {
    h.history_key(DeliveryScope::named("repo"))
        .unwrap()
        .render()
}

#[test]
fn refused_start_shows_the_notice_with_the_code_only() {
    let ir = load_ir(&two_starts()).unwrap();
    let refusal = select_start(&ir, Some(SENTINEL)).unwrap_err();
    assert_eq!(refusal, StartRefusal::Unknown);
    let h = EngineHost::refused_start(refusal);
    assert_eq!(h.ir_refusal(), Some("start_unknown"));
    let html = render_component_html(&h);
    assert!(
        html.contains("data-notice-kind=\"pack-unloadable\""),
        "{html}"
    );
    assert!(html.contains(&ir_notice_text("start_unknown")), "{html}");
    assert!(!html.contains(SENTINEL));
    assert!(!html.contains("draft-input"));
    assert!(!html.contains("raw-trigger"));
}

#[test]
fn refused_start_opens_no_history_and_saves_nothing() {
    let mut h = EngineHost::refused_start(StartRefusal::Unknown);
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
    assert!(h.history_to_save().is_none());
    // 保存済みの履歴があっても開かない（読みも書きもしない）。
    assert_eq!(h.boot(Some(b"{}")), BootKind::NoSession);
    assert!(h.history_to_save().is_none());
}

#[test]
fn selected_start_goes_into_the_key() {
    let first = host_at(None);
    let s1 = host_at(Some("s-1"));
    let s4 = host_at(Some("s-4"));
    let ir = load_ir(&two_starts()).unwrap();
    // 指定なしの鍵は、これまでの先頭の開始点の鍵と同じ文字列で、既存の保存済みの履歴は開き続ける。
    assert_eq!(
        key(&first),
        HistoryKey::from_ir(DeliveryScope::named("repo"), &ir)
            .unwrap()
            .render()
    );
    assert_eq!(key(&first), key(&s1));
    assert_ne!(key(&s1), key(&s4));
}
