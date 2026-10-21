//! 推定した OS ごと・受講者が選んだ OS ごとの修飾キーの表示の試験（TASK-285 1/2・2/2、JUDGE-28・D-48）。
//!
//! 修飾キーの語・案内文が OS ごとに合うこと、ホストの入口から描画へ届くこと、dirty を立てないことを確かめる。
//! 画面での確認は `tools/browser/modifier-key-hint.test.mjs`。呼び出し元は CI の `make test` のみである。

use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::learner_os::LearnerOs;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::modifier_key::{
    modifier_label, os_choice_value, parse_os_choice, paste_key_hint,
};
use fandhe_workshop_web::parts::Component;

#[test]
fn 修飾キーの語は_os_ごとに合う() {
    assert_eq!(modifier_label(LearnerOs::Mac), Some("Cmd"));
    assert_eq!(modifier_label(LearnerOs::Windows), Some("Ctrl"));
    assert_eq!(modifier_label(LearnerOs::Unknown), None);
}

#[test]
fn 貼り付けの案内文は_os_ごとに合い_推定できなければ_os_を選ぶよう促す() {
    assert_eq!(paste_key_hint(LearnerOs::Mac), "貼り付け: Cmd+V");
    assert_eq!(paste_key_hint(LearnerOs::Windows), "貼り付け: Ctrl+V");
    let unknown = paste_key_hint(LearnerOs::Unknown);
    assert_eq!(unknown, "OS を選ぶと、貼り付けのキーが表示されます");
    assert!(!unknown.contains("Cmd") && !unknown.contains("Ctrl"));
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
        (
            LearnerOs::Unknown,
            "OS を選ぶと、貼り付けのキーが表示されます",
        ),
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

#[test]
fn 選ぶ欄の値の復号は_mac_と_windows_の完全一致だけを受ける() {
    assert_eq!(parse_os_choice("mac"), Some(LearnerOs::Mac));
    assert_eq!(parse_os_choice("windows"), Some(LearnerOs::Windows));
    for bad in [
        "",
        "Mac",
        "WINDOWS",
        "linux",
        " mac",
        "mac ",
        "mac\n",
        "\u{0}",
        &"w".repeat(10_000),
    ] {
        assert_eq!(parse_os_choice(bad), None, "{bad:?}");
    }
    assert_eq!(os_choice_value(LearnerOs::Mac), Some("mac"));
    assert_eq!(os_choice_value(LearnerOs::Windows), Some("windows"));
    assert_eq!(os_choice_value(LearnerOs::Unknown), None);
}

fn choose(host: &mut EngineHost, value: &str) {
    let action = EngineHost::decode_action("os", value);
    if let Some(a) = action {
        host.update(a);
    }
}

#[test]
fn 選ぶと_key_hint_だけが_dirty_になり_値が切り替わる() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Unknown);
    choose(&mut host, "windows");
    assert_eq!(host.dirty_fields(), ["key_hint"]);
    assert_eq!(hint_of(&host), "貼り付け: Ctrl+V");
    choose(&mut host, "mac");
    assert_eq!(host.dirty_fields(), ["key_hint"]);
    assert_eq!(hint_of(&host), "貼り付け: Cmd+V");
}

#[test]
fn 同じ値の選び直しと不正な値は何も変えない() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Mac);
    choose(&mut host, "mac");
    assert!(host.dirty_fields().is_empty());
    for bad in ["", "Mac", "linux", "windows2"] {
        assert!(EngineHost::decode_action("os", bad).is_none(), "{bad:?}");
    }
    assert_eq!(hint_of(&host), "貼り付け: Cmd+V");
}

#[test]
fn 描画の_selected_は推定と選択に合い_欄は_role_と_aria_を持たない() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Unknown);
    let html = html_of(&host);
    assert!(
        html.contains("<option value=\"\" disabled=\"\" selected=\"\""),
        "{html}"
    );
    assert!(
        !html.contains("<option value=\"mac\" selected=\"\""),
        "{html}"
    );
    host.set_learner_os(LearnerOs::Windows);
    let html = html_of(&host);
    assert!(
        html.contains("<option value=\"windows\" selected=\"\""),
        "{html}"
    );
    assert!(html.contains("data-action-change=\"os\""), "{html}");
    let start = html.find("data-testid=\"os-select\"").expect("os-select");
    let open = html[..start].rfind('<').expect("open tag");
    let close = html[start..].find('>').expect("close tag") + start;
    let tag = &html[open..close];
    assert!(!tag.contains("role=") && !tag.contains("aria-"), "{tag}");
}

#[test]
fn 選んでも履歴と状態は変わらず_保存の経路も動かない() {
    let mut host = EngineHost::new();
    host.set_learner_os(LearnerOs::Unknown);
    let before = (
        host.history_to_save(),
        host.draft().to_owned(),
        host.save_epoch(),
    );
    choose(&mut host, "windows");
    choose(&mut host, "mac");
    let after = (
        host.history_to_save(),
        host.draft().to_owned(),
        host.save_epoch(),
    );
    assert_eq!(before, after);
}
