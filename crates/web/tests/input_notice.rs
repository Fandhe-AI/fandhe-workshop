//! 上限の案内文の表示と貼り付けの扱いの native 試験（TASK-442。TERM-15・TERM-19）。
//!
//! 切り捨ての関数そのものは `input_limit.rs` が確かめる。ここでは、公開の `EngineHost` を操作して、
//! 案内欄が画面の契約どおりに置かれること、超過で案内が束縛の値と描画に出ること、1 文字消すと消えること、
//! 貼り付けの試料を下書きに入れても送信が起きないことを確かめる。変異 `TERM15-1`・`TERM15-2` の拠り所になる。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::limits::{INPUT_TOO_LONG_MESSAGE, MAX_INPUT_CODE_POINTS};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::terminal_area::is_defined_testid;

const FIELD: &str = "input_notice";
const SAMPLES: &str = "input-field-ime/test-data/paste-samples";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn sample(name: &str) -> String {
    std::fs::read_to_string(testdata().join(SAMPLES).join(name)).unwrap()
}

/// 開始直後の状態から始まるホスト（ログの長さを見る試験用）。
fn host() -> EngineHost {
    let ir = load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn set_draft(h: &mut EngineHost, v: &str) {
    h.update(act("draft", v));
}

fn notice(h: &EngineHost) -> String {
    match h.bound_value(FIELD) {
        Some(BoundValue::Text(s)) => s,
        other => panic!("案内の束縛が文字でない: {other:?}"),
    }
}

fn log_len(h: &EngineHost) -> usize {
    h.model().session().unwrap().log().len()
}

fn cp(s: &str) -> usize {
    s.chars().count()
}

fn over() -> String {
    "あ".repeat(MAX_INPUT_CODE_POINTS + 1)
}

/// 描画した HTML の案内欄の要素（開きタグから閉じタグまで）。
fn notice_element(html: &str) -> &str {
    let at = html.find("data-testid=\"input-notice\"").unwrap();
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find("</p>").unwrap() + "</p>".len();
    &html[start..end]
}

#[test]
fn 案内欄は一つだけ_p_要素の_status_で_入力欄より後ろに置かれる() {
    let h = EngineHost::new();
    let html = render_component_html(&h);
    assert_eq!(html.matches("data-testid=\"input-notice\"").count(), 1);
    let el = notice_element(&html);
    assert!(el.starts_with("<p "), "{el}");
    assert!(el.contains("role=\"status\""), "{el}");
    // 子は文字だけ（入れ子の要素を持たない）。
    let inner = &el[el.find('>').unwrap() + 1..el.len() - "</p>".len()];
    assert!(!inner.contains('<'), "{el}");
    assert!(
        html.find("data-testid=\"draft-input\"").unwrap()
            < html.find("data-testid=\"input-notice\"").unwrap()
    );
    assert!(is_defined_testid("input-notice"));
}

#[test]
fn 上限までは案内が空で_超過すると固定の案内文が束縛と描画に出る() {
    let mut h = EngineHost::new();
    assert_eq!(notice(&h), "");
    set_draft(&mut h, &"あ".repeat(MAX_INPUT_CODE_POINTS));
    assert_eq!(notice(&h), "");
    assert!(!notice_element(&render_component_html(&h)).contains(INPUT_TOO_LONG_MESSAGE));

    set_draft(&mut h, &over());
    assert_eq!(notice(&h), INPUT_TOO_LONG_MESSAGE);
    assert!(h.dirty_fields().contains(&FIELD));
    assert_eq!(cp(h.draft()), MAX_INPUT_CODE_POINTS);
    let html = render_component_html(&h);
    let el = notice_element(&html);
    assert!(el.contains(INPUT_TOO_LONG_MESSAGE), "{el}");
}

#[test]
fn 一文字消すと案内も消える() {
    let mut h = EngineHost::new();
    set_draft(&mut h, &over());
    assert_eq!(notice(&h), INPUT_TOO_LONG_MESSAGE);
    let _ = h.dirty_fields();
    set_draft(&mut h, &"あ".repeat(MAX_INPUT_CODE_POINTS - 1));
    assert_eq!(notice(&h), "");
    assert!(h.dirty_fields().contains(&FIELD));
    assert!(!notice_element(&render_component_html(&h)).contains(INPUT_TOO_LONG_MESSAGE));
}

#[test]
fn 上限ちょうどと_utf16_で_20001_単位の試料は受け付け_案内は空() {
    for name in ["paste-20000.txt", "paste-20000-astral.txt"] {
        let v = sample(name);
        assert_eq!(cp(&v), MAX_INPUT_CODE_POINTS, "{name}");
        let mut h = EngineHost::new();
        set_draft(&mut h, &v);
        assert_eq!(h.draft(), v, "{name}");
        assert_eq!(notice(&h), "", "{name}");
    }
    let v = sample("paste-20000-astral.txt");
    assert_eq!(v.encode_utf16().count(), MAX_INPUT_CODE_POINTS + 1);
    let v = sample("paste-20001.txt");
    let mut h = EngineHost::new();
    set_draft(&mut h, &v);
    assert_eq!(cp(h.draft()), MAX_INPUT_CODE_POINTS);
    assert_eq!(notice(&h), INPUT_TOO_LONG_MESSAGE);
}

#[test]
fn 貼り付けの四つの試料は送信されず_改行を保ち_案内は空() {
    for name in [
        "paste-3lines.txt",
        "paste-100lines.txt",
        "paste-crlf-tabs.txt",
        "paste-command-lines.txt",
    ] {
        let v = sample(name);
        let mut h = host();
        let before = log_len(&h);
        set_draft(&mut h, &v);
        assert_eq!(h.draft(), v, "{name}");
        assert_eq!(h.draft().matches('\n').count(), v.matches('\n').count());
        assert!(v.contains('\n'), "{name}");
        assert_eq!(notice(&h), "", "{name}");
        assert_eq!(log_len(&h), before, "{name}: 貼り付けだけで送信された");
    }
}

#[test]
fn 超過で切っても右のログは増えない() {
    let mut h = host();
    let before = log_len(&h);
    set_draft(&mut h, &over());
    assert_eq!(notice(&h), INPUT_TOO_LONG_MESSAGE);
    assert_eq!(log_len(&h), before);
}
