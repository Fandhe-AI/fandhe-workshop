//! IR が大きさの上限を超えたとき、画面に契約の文面とコードが出て教材が描かれないことの native 試験
//! （SAFE-17・SAFE-45。TASK-154）。
//!
//! 判定そのものは core の `ir_size.rs` が持つ。ここでは、判定の結果を受けたホストが通知を描く部分を固定する。
//! 変異 `SAFE17-2` の拠り所になる。試験では `unwrap` を使ってよい。
#![cfg(feature = "test-hooks")]

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::limits::MAX_IR_BYTES;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::ir_notice::ir_notice_text;

fn padded(len: u64) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut bytes = std::fs::read(path).unwrap();
    bytes.resize(usize::try_from(len).unwrap(), b' ');
    bytes
}

fn snapshot(h: &EngineHost) -> serde_json::Value {
    serde_json::from_str(&h.engine_snapshot_json()).unwrap()
}

#[test]
fn over_by_one_ir_shows_the_size_message_and_code() {
    let err = load_ir(&padded(MAX_IR_BYTES + 1)).unwrap_err();
    let h = EngineHost::refused(&err);
    let html = render_component_html(&h);
    assert!(
        html.contains("data-notice-kind=\"pack-unloadable\""),
        "{html}"
    );
    assert!(html.contains("role=\"status\""), "{html}");
    assert!(
        html.contains("ファイルが上限の大きさを超えています"),
        "{html}"
    );
    assert!(html.contains("too_large"), "{html}");
    assert!(html.contains(&ir_notice_text("too_large")), "{html}");
    assert!(!html.contains("draft-input"), "{html}");
    assert_eq!(snapshot(&h)["irRefusal"], "too_large");
}

#[test]
fn exact_limit_ir_loads_and_shows_no_notice() {
    // 上限ちょうどの有効な IR は読め、通知（拒否）の経路に入らない。
    assert!(load_ir(&padded(MAX_IR_BYTES)).is_ok());
}

#[test]
fn notice_text_names_no_measured_value() {
    let text = ir_notice_text("too_large");
    assert!(!text.contains(&(MAX_IR_BYTES + 1).to_string()), "{text}");
    assert!(
        text.contains("ファイルが上限の大きさを超えています"),
        "{text}"
    );
}
