//! IR の要素数・文字列長の上限（SAFE-25。TASK-162）を超えたとき、画面に契約の文面とコードが出て
//! 教材が描かれないことの native 試験。判定は core の `ir_value_limits.rs` が持ち、ここでは通知の描画を固定する。
//! 試験では `unwrap` を使ってよい。
#![cfg(feature = "test-hooks")]

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::limits::{MAX_COLLECTION_ELEMENTS, MAX_STRING_CHARS};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::ir_notice::ir_notice_text;
use serde_json::Value;

fn ir_with(edit: impl FnOnce(&mut Value)) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    edit(&mut v);
    serde_json::to_vec(&v).unwrap()
}

fn lo(n: usize) -> Vec<u8> {
    ir_with(|v| {
        v["nodes"][0]["step"]["lo"] = Value::Array(vec![Value::from("x"); n]);
    })
}

fn title(n: usize) -> Vec<u8> {
    ir_with(|v| v["pack"]["title"] = Value::from("a".repeat(n)))
}

fn shows_notice(bytes: &[u8], code: &'static str, wording: &str) {
    let err = load_ir(bytes).unwrap_err();
    let h = EngineHost::refused(&err);
    let html = render_component_html(&h);
    assert!(
        html.contains("data-notice-kind=\"pack-unloadable\""),
        "{html}"
    );
    assert!(html.contains("role=\"status\""), "{html}");
    assert!(html.contains(wording), "{html}");
    assert!(html.contains(&ir_notice_text(code)), "{html}");
    assert!(!html.contains("draft-input"), "{html}");
    let snap: Value = serde_json::from_str(&h.engine_snapshot_json()).unwrap();
    assert_eq!(snap["irRefusal"], code);
}

#[test]
fn too_many_elements_shows_the_message_and_code() {
    shows_notice(
        &lo(MAX_COLLECTION_ELEMENTS + 1),
        "too_many_elements",
        "要素が上限（100,000 個）",
    );
}

#[test]
fn long_string_shows_the_message_and_code() {
    shows_notice(
        &title(MAX_STRING_CHARS + 1),
        "string_too_long",
        "文字列が上限（100,000 文字）",
    );
}

#[test]
fn exact_limit_loads() {
    assert!(load_ir(&lo(MAX_COLLECTION_ELEMENTS)).is_ok());
    assert!(load_ir(&title(MAX_STRING_CHARS)).is_ok());
}
