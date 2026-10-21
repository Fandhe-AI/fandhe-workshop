//! 推定した OS ごとの修飾キーの表示の試験（TASK-285 1/2、JUDGE-28・D-48）。
//!
//! 修飾キーの語・案内文が OS ごとに合うこと、ホストの入口から描画へ届くこと、dirty を立てないことを確かめる。
//! 画面での確認は `tools/browser/modifier-key-hint.test.mjs`。呼び出し元は CI の `make test` のみである。

use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::learner_os::LearnerOs;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::modifier_key::{modifier_label, paste_key_hint};

#[test]
fn 修飾キーの語は_os_ごとに合う() {
    assert_eq!(modifier_label(LearnerOs::Mac), Some("Cmd"));
    assert_eq!(modifier_label(LearnerOs::Windows), Some("Ctrl"));
    assert_eq!(modifier_label(LearnerOs::Unknown), None);
}

#[test]
fn 貼り付けの案内文は_os_ごとに合い_推定できなければ空() {
    assert_eq!(paste_key_hint(LearnerOs::Mac), "貼り付け: Cmd+V");
    assert_eq!(paste_key_hint(LearnerOs::Windows), "貼り付け: Ctrl+V");
    assert_eq!(paste_key_hint(LearnerOs::Unknown), "");
}

fn hint_of(host: &EngineHost) -> String {
    match host.bound_value("key_hint") {
        Some(BoundValue::Text(t)) => t,
        other => panic!("key_hint is not text: {other:?}"),
    }
}

fn html_of(host: &EngineHost) -> String {
    render_component_html(host)
}

#[test]
fn ホストへ渡した_os_が束縛値と描画に届き_dirty_は立たない() {
    for (os, want) in [
        (LearnerOs::Mac, "貼り付け: Cmd+V"),
        (LearnerOs::Windows, "貼り付け: Ctrl+V"),
        (LearnerOs::Unknown, ""),
    ] {
        let mut host = EngineHost::new();
        host.set_learner_os(os);
        assert_eq!(hint_of(&host), want);
        assert!(host.dirty_fields().is_empty(), "{os:?}");
        let html = html_of(&host);
        assert!(html.contains("data-testid=\"key-hint\""), "{html}");
        assert!(html.contains("data-bind-text=\"key_hint\""), "{html}");
        assert!(html.contains(want), "{html}");
    }
}

#[test]
fn 案内の要素は_role_と_aria_を持たない() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Mac);
    let html = html_of(&host);
    let start = html.find("data-testid=\"key-hint\"").expect("key-hint");
    let open = html[..start].rfind('<').expect("open tag");
    let close = html[start..].find('>').expect("close tag") + start;
    let tag = &html[open..close];
    assert!(!tag.contains("role="), "{tag}");
    assert!(!tag.contains("aria-"), "{tag}");
}
