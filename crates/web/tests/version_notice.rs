//! 変換して開いたときの版の違いの通知（TASK-535 1/2。SHELL-20）の試験。
//!
//! 履歴ファイルの読み込み（`open_history`）と起動時の復元（`boot`）が、版の違いを変換して開いたときだけ枠の通知欄
//! （`shell-notice`）へ通知を 1 件出し、同じ版・開かない場合では出さないこと、閉じる操作が表示だけを変えることを固定する。
//! 失敗の文には ID・真偽・固定の名前だけを出す（履歴の中身を出さない）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::shell::{self, NoticeKind};
use fandhe_workshop_web::shell_notice::VERSION_CONVERTED_NOTICE;
use fandhe_workshop_web::{a11y, shell_notice};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の教材の `pack.version` を書き換えた IR の JSON。語彙は版の違う履歴の試験と同じ 1.2 にそろえる。
fn ir_json(version: &str, name: &str) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    if !name.is_empty() {
        v["pack"]["name"] = serde_json::json!(name);
    }
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
    let ir = load_ir(&ir_json(version, "")).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

/// `version` の教材で書いた履歴（`ops` は空でない）。
fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version, "")).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn children(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    for c in children(n) {
        find(c, testid, out);
    }
}

fn all<'a>(n: &'a Node, testid: &str) -> Vec<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    v
}

/// 文字ノードを連結する。
fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(text_of).collect(),
        _ => String::new(),
    }
}

fn items(h: &EngineHost) -> usize {
    all(&h.view(), shell::NOTICE_ITEM_TESTID).len()
}

fn log_len(h: &EngineHost) -> usize {
    h.model().session().unwrap().log().len()
}

#[test]
fn same_version_opens_without_a_notice() {
    let mut h = host("1.0.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    let view = h.view();
    assert_eq!(all(&view, shell::NOTICE_AREA_TESTID).len(), 1);
    assert_eq!(
        attr(all(&view, shell::NOTICE_AREA_TESTID)[0], "role"),
        Some("status")
    );
    assert_eq!(items(&h), 0);
}

#[test]
fn converted_open_shows_one_notice() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    assert!(h.dirty_fields().contains(&"shell_notices"));
    let view = h.view();
    let item = all(&view, shell::NOTICE_ITEM_TESTID);
    assert_eq!(item.len(), 1);
    assert_eq!(
        attr(item[0], shell::NOTICE_KIND_ATTR),
        Some("version-converted")
    );
    assert_eq!(
        attr(item[0], shell::NOTICE_MESSAGE_ATTR),
        Some("msg-version-converted")
    );
    assert_eq!(attr(item[0], shell::NOTICE_REASON_ATTR), None);
    let t = all(&view, shell::NOTICE_TEXT_TESTID);
    assert_eq!(t.len(), 1);
    assert_eq!(text_of(t[0]), VERSION_CONVERTED_NOTICE);
    let d = all(&view, "shell-dismiss");
    assert_eq!(d.len(), 1);
    assert_eq!(attr(d[0], "data-action"), Some("dismiss-notice"));
    assert_eq!(attr(d[0], "data-payload"), Some("version-converted"));
    assert!(shell::check_complete(&view).is_ok());
    assert!(shell::check_no_undefined(&view).is_ok());
    assert!(a11y::check_no_undefined(&view).is_ok());
}

#[test]
fn a_same_version_open_after_a_conversion_clears_the_notice() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    assert_eq!(items(&h), 1);
    assert_eq!(h.open_history(&history("1.1.0")), ImportOutcome::Open);
    assert_eq!(items(&h), 0);
}

#[test]
fn a_same_version_open_marks_the_notice_list_dirty_so_the_dom_drops_it() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    // 描いていない種類の閉じる操作は表示を変えず、dirty だけを空にする（前の dirty を持ち越さないため）。
    h.update(EngineHost::decode_action("dismiss-notice", "save-failed").unwrap());
    assert!(!h.dirty_fields().contains(&"shell_notices"));
    assert_eq!(items(&h), 1);
    assert_eq!(h.open_history(&history("1.1.0")), ImportOutcome::Open);
    assert_eq!(items(&h), 0);
    assert!(h.dirty_fields().contains(&"shell_notices"));
}

#[test]
fn a_closed_open_does_not_touch_the_notice() {
    let mut h = host("1.0.0");
    let before = log_len(&h);
    assert!(matches!(
        h.open_history(&history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    assert_eq!(items(&h), 0);
    // 表示だけの理由の欄（VER-8）だけが動く。枠の通知・ログ・状態は変えない（SAVE-19）。
    assert_eq!(h.dirty_fields(), vec!["view_only_notice"], "SAVE-19");
    assert_eq!(log_len(&h), before);

    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    // 前に変換して開いた実行環境は残っているので、通知も残る。
    let bytes = history("3.0.0");
    assert!(matches!(h.open_history(&bytes), ImportOutcome::ViewOnly(_)));
    assert_eq!(items(&h), 1);

    // 開かない履歴は理由の通知を足す。変換して開いた通知は残る（TASK-176 2/2）。
    let mut bad: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    bad["versions"]["ir_format"] = serde_json::json!(99);
    assert!(matches!(
        h.open_history(&serde_json::to_vec(&bad).unwrap()),
        ImportOutcome::Refused(_)
    ));
    assert_eq!(items(&h), 2);
}

#[test]
fn boot_restores_with_a_notice_only_when_converted() {
    let mut h = host("1.1.0");
    assert_eq!(h.boot(Some(&history("1.0.0"))), BootKind::Restored);
    assert_eq!(items(&h), 1);

    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(&history("1.0.0"))), BootKind::Restored);
    assert_eq!(items(&h), 0);

    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(&history("2.0.0"))), BootKind::NotOpened);
    assert_eq!(items(&h), 0);

    let mut h = host("1.0.0");
    assert_eq!(h.boot(None), BootKind::Fresh);
    assert_eq!(items(&h), 0);
}

#[test]
fn dismiss_decodes_strictly_and_changes_only_the_display() {
    let action = EngineHost::decode_action("dismiss-notice", "version-converted");
    assert!(matches!(
        action,
        Some(HostAction::DismissNotice(NoticeKind::VersionConverted))
    ));
    for bad in [
        "",
        "Version-Converted",
        &"x".repeat(33),
        "bad payload",
        "unknown-kind",
    ] {
        assert!(
            EngineHost::decode_action("dismiss-notice", bad).is_none(),
            "payload len {}",
            bad.len()
        );
    }
    for name in ["export-history", "discard-and-start", "import-history"] {
        assert!(EngineHost::decode_action(name, "").is_none(), "{name}");
    }

    let mut h = host("1.1.0");
    h.open_history(&history("1.0.0"));
    let before = log_len(&h);
    h.update(EngineHost::decode_action("dismiss-notice", "version-converted").unwrap());
    assert_eq!(items(&h), 0);
    assert_eq!(h.dirty_fields(), vec!["shell_notices"]);
    assert_eq!(log_len(&h), before);

    // 描いていない種類は何も変えない。
    let mut h = host("1.1.0");
    h.open_history(&history("1.0.0"));
    h.update(EngineHost::decode_action("dismiss-notice", "save-failed").unwrap());
    assert_eq!(items(&h), 1);
}

#[test]
fn the_notice_list_is_a_bind_list_and_binding_points_do_not_change() {
    let mut h = host("1.1.0");
    let marks = |h: &EngineHost| {
        let v = format!("{:?}", h.view());
        v.matches("shell_notices").count()
    };
    let before = marks(&h);
    assert!(before >= 1);
    h.open_history(&history("1.0.0"));
    assert_eq!(marks(&h), before);
    assert_eq!(shell_notice::SHELL_NOTICES_FIELD.name, "shell_notices");
}

#[test]
fn history_derived_strings_are_not_shown_in_the_notice() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    let view = h.view();
    let area = all(&view, shell::NOTICE_AREA_TESTID)[0];
    let s = text_of(area);
    assert_eq!(s.matches("1.0.0").count(), 0);
    assert_eq!(s.matches("1.1.0").count(), 0);
    assert!(s.contains(VERSION_CONVERTED_NOTICE));
}

// ここから TASK-535 2/2（SHELL-20）。変換して開いた後の操作の保存が、開いた側の版で書かれることを固定する。
// 保存の値の `versions` は `build_envelope` がいま載っている IR から求め直すので、履歴を書いた側の版は引き継がれない。

/// `version` の教材から求めた、いま載せるべき 4 軸の版（保存の値の `versions` の期待）。
fn expected_versions(version: &str) -> fandhe_workshop_contract::history_version::HistoryVersions {
    let ir = load_ir(&ir_json(version, "")).unwrap();
    let hash = fandhe_workshop_contract::content_hash::content_hash(&ir).unwrap();
    fandhe_workshop_core::version_record::record_versions(&ir, &hash).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// 秘密情報の検査に当たらない固定の文を送って、状態を変える。
fn operate(h: &mut EngineHost) {
    h.update(act("draft", "/make-note second-note"));
    h.update(act("submit", ""));
}

fn saved_ops(h: &EngineHost) -> usize {
    h.history_to_save()
        .map(|t| {
            parse_history_envelope(parse_history_json(t.as_bytes()).unwrap())
                .unwrap()
                .ops
                .len()
        })
        .unwrap_or(0)
}

fn saved_versions(h: &EngineHost) -> fandhe_workshop_contract::history_version::HistoryVersions {
    let text = h.history_to_save().expect("saved value exists");
    parse_history_envelope(parse_history_json(text.as_bytes()).unwrap())
        .unwrap()
        .versions
}

#[test]
fn a_save_after_a_converted_open_is_written_with_the_opening_versions() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    let opened_ops = saved_ops(&h);
    operate(&mut h);
    let versions = saved_versions(&h);
    assert!(
        versions == expected_versions("1.1.0"),
        "versions follow the opening ir"
    );
    assert!(
        versions != expected_versions("1.0.0"),
        "versions differ from the history's"
    );
    assert!(saved_ops(&h) > opened_ops, "ops are appended");

    // 書かれた値は新しい版では変換なしで開け、古い版では変換になる（版が本当に変わったことの対照）。
    let bytes = h.history_to_save().unwrap().into_bytes();
    let mut again = host("1.1.0");
    assert_eq!(again.open_history(&bytes), ImportOutcome::Open);
    assert_eq!(items(&again), 0);
    let mut older = host("1.0.0");
    assert_eq!(older.open_history(&bytes), ImportOutcome::Converted);
}

#[test]
fn a_save_after_a_boot_restore_conversion_is_written_with_the_opening_versions() {
    let mut h = host("1.1.0");
    assert_eq!(h.boot(Some(&history("1.0.0"))), BootKind::Restored);
    assert_eq!(items(&h), 1);
    operate(&mut h);
    assert!(saved_versions(&h) == expected_versions("1.1.0"));
}

#[test]
fn control_a_same_version_open_keeps_writing_the_same_versions() {
    let mut h = host("1.0.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    operate(&mut h);
    assert!(saved_versions(&h) == expected_versions("1.0.0"));
}
