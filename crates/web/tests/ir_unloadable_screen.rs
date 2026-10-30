//! 教材（IR）を読めないときの画面が、枠の通知欄の `pack-unloadable` 項目で出ることの native 試験
//! （SHELL-31。TASK-545）。
//!
//! 判定そのものは core の `load_ir` と `start_select::open_bundled_ir` が持つ。ここでは、断った結果を受けたホストが
//! 教材を描かず、契約どおりの通知 1 件だけを描くこと、保存しないことを固定する。変異 `SHELL31-2`・`SHELL31-3` の拠り所になる。
//!
//! 読み替え: 巡回する IR は読み込みの時点では断らない（`instance_graph` を `load_ir` に結線しない。`core/src/instance.rs`）。
//! 巡回は実行時の遷移の上限（SAFE-23・34・35）が止めるので、ここでは「断らずに開く」ことを固定する。
//! `check_complete_v2` は適用しない（パネルが無い画面のため）。試験では `unwrap` を使ってよい。
#![cfg(feature = "test-hooks")]

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::limits::MAX_IR_BYTES;
use fandhe_workshop_contract::storage::DeliveryScope;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_web::host::{BootKind, EngineHost};
use fandhe_workshop_web::ir_notice::ir_notice_text;
use fandhe_workshop_web::start_select::{
    open_bundled_ir, open_fetched_ir, BundledIrRefusal, FetchedIr, StartRefusal,
};

const SENTINEL: &str = "UNLOADABLE-SENTINEL-X";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn min3() -> Vec<u8> {
    std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json")).unwrap()
}

fn cycle(name: &str) -> Vec<u8> {
    std::fs::read(testdata().join(format!(
        "untrusted-input-safety/test-data-addendum-3/cycle-ir/{name}.json"
    )))
    .unwrap()
}

fn host_for(bytes: &[u8]) -> (EngineHost, &'static str) {
    let err = load_ir(bytes).unwrap_err();
    (EngineHost::refused(&err), err.reason_code())
}

/// 断った画面の共通の期待。教材を描かず、`pack-unloadable` の項目がちょうど 1 件で、操作が 0 件。
fn assert_unloadable_screen(h: &EngineHost, code: &str) {
    let html = render_component_html(h);
    assert_eq!(
        html.matches("data-notice-kind=\"pack-unloadable\"").count(),
        1,
        "{code}: {html}"
    );
    assert!(
        html.contains(&format!("data-notice-reason=\"{code}\"")),
        "{code}: {html}"
    );
    assert!(
        html.contains("data-notice-message=\"msg-pack-unloadable\""),
        "{html}"
    );
    assert!(
        html.contains(&ir_notice_text(h.ir_refusal().unwrap())),
        "{html}"
    );
    assert_eq!(html.matches("role=\"status\"").count(), 1, "{code}: {html}");
    assert_eq!(
        html.matches("data-testid=\"shell-notice\"").count(),
        1,
        "{html}"
    );
    assert_eq!(
        html.matches("data-testid=\"shell-mock\"").count(),
        1,
        "{html}"
    );
    for absent in [
        "shell-left",
        "shell-right",
        "pane-tabs",
        "draft-input",
        "step-title",
        "raw-trigger",
        "data-action-keydown",
        "<button",
        "shell-dismiss",
        SENTINEL,
    ] {
        assert!(!html.contains(absent), "{code}: {absent}: {html}");
    }
}

fn nested(depth: usize) -> Vec<u8> {
    let mut s = "[".repeat(depth);
    s.push_str(&"]".repeat(depth));
    s.into_bytes()
}

fn duplicated_uid() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&min3()).unwrap();
    let nodes = v["nodes"].as_array_mut().unwrap();
    let first = nodes[0].clone();
    nodes.push(first);
    v["sentinel_x"] = SENTINEL.into();
    serde_json::to_vec(&v).unwrap()
}

fn future_format() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&min3()).unwrap();
    v["format_version"] = 3.into();
    v["pack"]["ir_format"] = 3.into();
    serde_json::to_vec(&v).unwrap()
}

#[test]
fn every_load_failure_shows_one_pack_unloadable_item() {
    let mut padded = min3();
    padded.resize(usize::try_from(MAX_IR_BYTES + 1).unwrap(), b' ');
    let cases: [(&str, Vec<u8>); 6] = [
        ("too_large", padded),
        ("too_deep", nested(65)),
        ("not_json", b"{".to_vec()),
        ("not_json", Vec::new()),
        ("malformed", duplicated_uid()),
        ("format_newer", future_format()),
    ];
    for (expected, bytes) in cases {
        let (h, code) = host_for(&bytes);
        assert_eq!(code, expected);
        assert_unloadable_screen(&h, code);
    }
}

#[test]
fn every_start_refusal_shows_one_pack_unloadable_item() {
    for r in [StartRefusal::Unknown, StartRefusal::Missing] {
        let code = r.reason_code();
        assert_unloadable_screen(&EngineHost::refused_start(r), code);
    }
}

#[test]
fn not_found_is_judged_before_body_and_shown() {
    // 404 は本文を見ずに断る（SHELL-32・D-74）。読める IR が付いていても、不正な本文でも結果は同じ。
    for body in [min3(), b"{".to_vec(), Vec::new()] {
        let _ = body;
        assert!(matches!(
            open_fetched_ir(FetchedIr::NotFound, None),
            Err(BundledIrRefusal::NotFound)
        ));
    }
    let h = EngineHost::refused_not_found();
    assert_eq!(h.ir_refusal(), Some("not_found"));
    assert_unloadable_screen(&h, "not_found");
    let html = render_component_html(&h);
    assert!(html.contains("教材が見つかりません"), "{html}");
    assert!(!html.contains("教材を読み込めませんでした"), "{html}");
}

#[test]
fn not_found_screen_saves_nothing() {
    let mut h = EngineHost::refused_not_found();
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
    assert!(h.history_to_save().is_none());
    assert_eq!(h.boot(Some(b"{}")), BootKind::NoSession);
    assert!(h.history_to_save().is_none());
}

#[test]
fn empty_body_stays_not_json() {
    // 200 で空の本文は 404 ではない。not_json のまま（SHELL-32）。
    match open_fetched_ir(FetchedIr::Body(&[]), None) {
        Err(BundledIrRefusal::Load(e)) => {
            assert_eq!(e.reason_code(), "not_json");
            assert_unloadable_screen(&EngineHost::refused(&e), "not_json");
            let html = render_component_html(&EngineHost::refused(&e));
            assert!(!html.contains("教材が見つかりません"), "{html}");
        }
        _ => panic!("empty body must be refused as a load failure"),
    }
}

#[test]
fn unloadable_screen_saves_nothing() {
    let (mut h, _) = host_for(b"{");
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
    assert!(h.history_to_save().is_none());
    assert_eq!(h.boot(Some(b"{}")), BootKind::NoSession);
    assert!(h.history_to_save().is_none());
}

#[test]
fn cyclic_ir_is_not_refused_at_load_time() {
    for (name, start) in [
        ("cycle-enter-self", Some("s-enter-self")),
        ("cycle-exit-2", Some("s-exit-2")),
        ("cycle-enter-self", None),
    ] {
        let bytes = cycle(name);
        assert!(load_ir(&bytes).is_ok(), "{name}");
        assert!(open_bundled_ir(&bytes, start).is_ok(), "{name}");
    }
}

#[test]
fn loadable_ir_has_no_pack_unloadable_item() {
    // 負の対照: 読める IR の画面には項目が出ない（上の検査が空振りしないことの確認）。
    let (session, _) = open_bundled_ir(&min3(), None).unwrap();
    let h = EngineHost::with_session(
        session,
        fandhe_workshop_web::engine_parts::standard_registry(),
    )
    .unwrap();
    let html = render_component_html(&h);
    assert!(!html.contains("pack-unloadable"), "{html}");
    assert!(html.contains("draft-input"), "{html}");
}
