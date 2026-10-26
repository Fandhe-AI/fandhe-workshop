//! 操作プロファイルの説明（`op-guide`）への修飾キーの表示の試験（TASK-392 1/2、A11Y-25・D-48・D-79）。
//!
//! 説明の文言が OS ごとに `modifier_label` の語と一致すること、ホストの入口・選ぶ欄から束縛値と描画へ届くこと、
//! 推定できない OS で Cmd・Ctrl を出さないこと、`role`・`aria` を持たないことを確かめる。
//! 画面での確認は `tools/browser/op-guide.test.mjs`。呼び出し元は CI の `make test` のみである。

use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::learner_os::LearnerOs;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::modifier_key::{modifier_label, operation_guide};
use fandhe_workshop_web::parts::Component;

fn guide_of(host: &EngineHost) -> String {
    match host.bound_value("op_guide") {
        Some(BoundValue::Text(t)) => t,
        other => panic!("op_guide is not text: {other:?}"),
    }
}

fn choose(host: &mut EngineHost, value: &str) {
    if let Some(a) = EngineHost::decode_action("os", value) {
        host.update(a);
    }
}

#[test]
fn 説明の文言は_os_ごとの修飾キーの語と一致し_もう一方の語を含まない() {
    for (os, other) in [(LearnerOs::Mac, "Ctrl"), (LearnerOs::Windows, "Cmd")] {
        let g = operation_guide(os);
        let label = modifier_label(os).expect("label");
        assert!(g.contains(label), "{g}");
        assert!(!g.contains(other), "{g}");
        assert!(!g.contains('\u{2318}'), "記号は使わない: {g}");
    }
    let unknown = operation_guide(LearnerOs::Unknown);
    assert!(
        !unknown.contains("Cmd") && !unknown.contains("Ctrl"),
        "{unknown}"
    );
    for os in [LearnerOs::Mac, LearnerOs::Windows, LearnerOs::Unknown] {
        let g = operation_guide(os);
        for word in ["Enter", "Shift+Enter", "Tab", "Esc"] {
            assert!(g.contains(word), "{g}");
        }
    }
}

#[test]
fn ホストへ渡した_os_が束縛値と描画に届き_dirty_は立たない() {
    for os in [LearnerOs::Mac, LearnerOs::Windows, LearnerOs::Unknown] {
        let mut host = EngineHost::new();
        host.set_learner_os(os);
        let want = operation_guide(os);
        assert_eq!(guide_of(&host), want);
        assert!(host.dirty_fields().is_empty(), "{os:?}");
        let html = render_component_html(&host);
        assert!(html.contains("data-testid=\"op-guide\""), "{html}");
        assert!(html.contains("data-bind-text=\"op_guide\""), "{html}");
        assert!(html.contains(want), "{html}");
    }
}

#[test]
fn 選ぶと説明が切り替わり_同じ値と不正な値は何も変えない() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Unknown);
    choose(&mut host, "windows");
    assert_eq!(host.dirty_fields(), ["key_hint", "op_guide"]);
    assert_eq!(guide_of(&host), operation_guide(LearnerOs::Windows));
    choose(&mut host, "mac");
    assert_eq!(host.dirty_fields(), ["key_hint", "op_guide"]);
    assert_eq!(guide_of(&host), operation_guide(LearnerOs::Mac));
    let _ = host.dirty_fields();
    choose(&mut host, "mac");
    choose(&mut host, "linux");
    assert!(host.dirty_fields().is_empty());
    assert_eq!(guide_of(&host), operation_guide(LearnerOs::Mac));
}

#[test]
fn 説明の要素は_role_と_aria_を持たず_選ぶ欄は_label_の中にある() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Mac);
    let html = render_component_html(&host);
    let start = html.find("data-testid=\"op-guide\"").expect("op-guide");
    let open = html[..start].rfind('<').expect("open tag");
    let close = html[start..].find('>').expect("close tag") + start;
    let tag = &html[open..close];
    assert!(!tag.contains("role=") && !tag.contains("aria-"), "{tag}");
    let sel = html.find("data-testid=\"os-select\"").expect("os-select");
    let label_open = html[..sel].rfind("<label").expect("label");
    assert!(!html[label_open..sel].contains("</label>"), "{html}");
    assert!(html[label_open..sel].contains("OS"), "{html}");
}
