//! 開かない履歴の動作（VER-8・SHELL-16。TASK-176 2/2）の試験。
//!
//! 版の違う履歴の判定が「開かない」になったとき、(1) 理由が枠の通知欄（`shell-notice`）に出る、(2) 状態・ログ・下書き・
//! 導出した値・保存の値が変わらない、(3) 表示だけのモードにならず操作は効く、ことを固定する。読み込み（`open_history`）と
//! 起動時の復元（`boot`）の両方を覆う。起動時は版の判定の段の拒否だけに通知を出し、壊れた履歴（他の段）は TASK-533 の範囲で出さない。
//! 失敗の文には件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::shell;
use fandhe_workshop_web::{a11y, shell_notice};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir_json(version: &str) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
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
    let ir = load_ir(&ir_json(version)).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

/// 履歴の `versions.ir_format` を開く側より新しくした履歴（`format_newer`。版の判定の段の拒否）。
fn format_newer() -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    let n = v["versions"]["ir_format"].as_u64().unwrap();
    v["versions"]["ir_format"] = serde_json::json!(n + 1);
    serde_json::to_vec(&v).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn operate(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
}

fn snapshot(h: &EngineHost) -> (String, usize, String) {
    let s = h.model().session().unwrap();
    (
        serde_json::to_string(s.state()).unwrap(),
        s.log().len(),
        h.draft().to_owned(),
    )
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

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    if let Node::Element { children, .. } = n {
        for c in children {
            find(c, testid, out);
        }
    }
}

fn all<'a>(n: &'a Node, testid: &str) -> Vec<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    v
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(text_of).collect(),
        _ => String::new(),
    }
}

/// 通知欄の項目のうち、種類が `kind` のものの数。
fn items_of(h: &EngineHost, kind: &str) -> usize {
    let view = h.view();
    all(&view, shell::NOTICE_ITEM_TESTID)
        .into_iter()
        .filter(|n| attr(n, "data-notice-kind") == Some(kind))
        .count()
}

fn dismiss_buttons(h: &EngineHost) -> usize {
    all(&h.view(), "shell-dismiss").len()
}

#[test]
fn 読み込みで開かないと理由が出て_状態は変わらない() {
    let mut h = host("1.0.0");
    operate(&mut h);
    let before = snapshot(&h);
    let saved = h.history_to_save();
    let projection = h.model().projection().clone();
    let world = h.model().world().clone();

    let outcome = h.open_history(&format_newer());
    assert_eq!(outcome, ImportOutcome::Refused(LoadReason::FormatNewer));
    assert_eq!(snapshot(&h), before);
    assert_eq!(h.model().projection(), &projection);
    assert_eq!(h.model().world(), &world);
    assert_eq!(h.history_to_save(), saved, "保存の値は変わらない");
    assert_eq!(h.model().view_only_reason(), None);

    let view = h.view();
    let item = all(&view, shell::NOTICE_ITEM_TESTID);
    assert_eq!(item.len(), 1);
    assert_eq!(attr(item[0], "data-notice-kind"), Some("import-refused"));
    assert_eq!(attr(item[0], "data-notice-reason"), Some("format_newer"));
    assert!(text_of(item[0]).contains("format_newer"));
    // 次の行動の文（期待値は実装から引かず直書きする。TASK-334。割り当ては案）。
    assert!(text_of(item[0]).contains("この履歴を書き出したときの版の教材で開いてください。"));
    assert_eq!(dismiss_buttons(&h), 1);
    assert!(shell::check_complete(&view).is_ok());
    assert!(a11y::check_no_undefined(&view).is_ok());
}

#[test]
fn 壊れたファイルの拒否には別のファイルを選ぶ行動が出て_保存は変わらない() {
    let mut h = host("1.0.0");
    operate(&mut h);
    let saved = h.history_to_save();
    let before = snapshot(&h);
    let outcome = h.open_history(b"{ not json");
    assert!(matches!(outcome, ImportOutcome::Refused(_)), "{outcome:?}");
    assert_eq!(snapshot(&h), before);
    assert_eq!(h.history_to_save(), saved);
    let view = h.view();
    let item = all(&view, shell::NOTICE_ITEM_TESTID);
    assert_eq!(item.len(), 1);
    assert!(text_of(item[0]).contains("別の履歴ファイルを選んで読み込んでください。"));
}

#[test]
fn 読み込みで開かないと積む_field_は通知欄だけ() {
    let mut h = host("1.0.0");
    h.open_history(&format_newer());
    assert_eq!(h.dirty_fields(), vec!["shell_notices"]);
}

#[test]
fn 読み込みで開かなかった後も操作が効く() {
    let mut h = host("1.0.0");
    assert!(matches!(
        h.open_history(&format_newer()),
        ImportOutcome::Refused(_)
    ));
    let before = snapshot(&h);
    operate(&mut h);
    assert_ne!(snapshot(&h), before);
    assert!(h.history_to_save().is_some());
}

#[test]
fn 閉じる操作で通知だけが消える() {
    let mut h = host("1.0.0");
    h.open_history(&format_newer());
    let before = snapshot(&h);
    h.update(act("dismiss-notice", "import-refused"));
    assert_eq!(items_of(&h, "import-refused"), 0);
    assert_eq!(snapshot(&h), before);
    assert_eq!(h.dirty_fields(), vec!["shell_notices"]);
}

#[test]
fn 開ける履歴や表示だけの履歴を続けて読むと通知が消える() {
    let mut h = host("1.0.0");
    h.open_history(&format_newer());
    assert_eq!(items_of(&h, "import-refused"), 1);
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    assert_eq!(items_of(&h, "import-refused"), 0);

    h.open_history(&format_newer());
    assert!(matches!(
        h.open_history(&history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    assert_eq!(items_of(&h, "import-refused"), 0);
    assert!(h.model().view_only_reason().is_some());
}

#[test]
fn 表示だけの間に開かない履歴を読んでも表示だけのまま() {
    let mut h = host("1.0.0");
    assert!(matches!(
        h.open_history(&history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    let reason = h.model().view_only_reason();
    let before = snapshot(&h);
    assert!(matches!(
        h.open_history(&format_newer()),
        ImportOutcome::Refused(_)
    ));
    assert_eq!(h.model().view_only_reason(), reason);
    assert_eq!(snapshot(&h), before);
    assert_eq!(items_of(&h, "import-refused"), 1);
}

#[test]
fn 変換して開いた後に開かない履歴を読むと通知が二つ並ぶ() {
    let mut h = host("1.1.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Converted);
    assert!(matches!(
        h.open_history(&format_newer()),
        ImportOutcome::Refused(_)
    ));
    assert_eq!(items_of(&h, "import-refused"), 1);
    assert_eq!(items_of(&h, "version-converted"), 1);
    let view = h.view();
    assert!(shell::check_complete(&view).is_ok());
    assert!(a11y::check_no_undefined(&view).is_ok());
}

#[test]
fn 起動時に版の判定の段で開かないと閉じられない通知が出る() {
    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(&format_newer())), BootKind::NotOpened);
    assert_eq!(items_of(&h, "import-refused"), 1);
    assert!(
        format!("{:?}", h.view()).contains("この履歴を書き出したときの版の教材で開いてください。")
    );
    assert_eq!(dismiss_buttons(&h), 0);
    assert_eq!(h.model().view_only_reason(), None);
    operate(&mut h);
    assert!(
        h.history_to_save().is_none(),
        "起動時に開けなかった履歴は保存しない"
    );
    // 生の閉じる操作が届いても消えない。
    h.update(act("dismiss-notice", "import-refused"));
    assert_eq!(items_of(&h, "import-refused"), 1);
}

#[test]
fn 起動時の拒否の後に読み込みで開かなくても閉じられない通知のまま() {
    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(&format_newer())), BootKind::NotOpened);
    assert_eq!(
        h.open_history(&format_newer()),
        ImportOutcome::Refused(LoadReason::FormatNewer)
    );
    assert_eq!(items_of(&h, "import-refused"), 1);
    assert_eq!(
        dismiss_buttons(&h),
        0,
        "保存が止まっている間は閉じる操作を置かない"
    );
    h.update(act("dismiss-notice", "import-refused"));
    assert_eq!(
        items_of(&h, "import-refused"),
        1,
        "閉じても保存停止の案内は残る"
    );
    operate(&mut h);
    assert!(h.history_to_save().is_none());
}

#[test]
fn 起動時の壊れた履歴には通知を出さない() {
    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(b"{broken")), BootKind::NotOpened);
    assert_eq!(items_of(&h, "import-refused"), 0);
}

#[test]
fn 通知に履歴由来の値は出ない() {
    let mut h = host("1.0.0");
    h.open_history(&format_newer());
    let view = h.view();
    let s = text_of(all(&view, shell::NOTICE_AREA_TESTID)[0]);
    assert!(!s.contains("1.0.0"));
    assert!(s.contains(&shell_notice::import_refused_message(
        LoadReason::FormatNewer
    )));
}

#[test]
fn 読み込み先の無いホストでは通知を出さない() {
    let mut h = EngineHost::new();
    assert_eq!(h.open_history(&format_newer()), ImportOutcome::Unavailable);
    assert_eq!(items_of(&h, "import-refused"), 0);
}
