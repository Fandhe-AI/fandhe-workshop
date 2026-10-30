//! 表示だけで開いた履歴の動作（VER-8。TASK-176 1/2）の試験。
//!
//! 版の違う履歴の判定が「表示だけ」になったとき、(1) 理由が通知欄（`view-only-notice`）に出る、(2) 入力・クリック・保存・停止を
//! 送っても状態とログが変わらない、(3) 保存の値が出ず、書き出しは読み込んだバイト列そのものを返す、ことを固定する。
//! 読み込み（`open_history`）と起動時の復元（`boot`）の両方を覆う。開かない場合（#733）は `refused_history.rs` が扱い、ここには壊れた履歴が起動時に表示だけにならない回帰だけを置く。
//! 失敗の文には件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。試験では `unwrap` を使ってよい。

#[path = "common/secret_values.rs"]
mod secret_values;

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の教材の `pack.version` を書き換えた IR の JSON。語彙は版の違う履歴の試験と同じ 1.2 にそろえる。
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

/// `version` の教材で書いた履歴（`ops` は空でない）。
fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn notice(h: &EngineHost) -> String {
    match h
        .registry()
        .bound_value("view_only_notice", h.model())
        .unwrap()
    {
        BoundValue::Text(t) => t,
        other => panic!("view_only_notice の束縛が文字列でない: {other:?}"),
    }
}

/// 状態の全体（JSON）・ログの長さ・下書きの 3 つ組。操作の前後で比べる。
fn snapshot(h: &EngineHost) -> (String, usize, String) {
    let s = h.model().session().unwrap();
    (
        serde_json::to_string(s.state()).unwrap(),
        s.log().len(),
        h.draft().to_owned(),
    )
}

/// 状態・ログを変えうる操作の一式。表示だけの間は、どれも何も変えない。
fn mutating_actions() -> Vec<(&'static str, HostAction)> {
    vec![
        ("draft", act("draft", "/make-note first-note")),
        ("submit", act("submit", "")),
        ("esc", act("esc", "Escape")),
        ("stop-button", act("stop", "")),
        ("complete", act("complete", "")),
        ("raw-save", HostAction::Engine(Op::Control(Control::Save))),
        ("raw-stop", HostAction::Engine(Op::Control(Control::Stop))),
    ]
}

fn assert_frozen(h: &mut EngineHost) {
    let before = snapshot(h);
    for (name, a) in mutating_actions() {
        h.update(a);
        assert_eq!(snapshot(h), before, "{name} が状態・ログ・下書きを変えた");
        assert!(h.history_to_save().is_none(), "{name} の後に保存の値が出た");
        assert!(h.next_tick().is_none(), "{name} の後に進行の宛先が出た");
    }
}

#[test]
fn 読み込みで表示だけになると理由が出て_操作は何も変えない() {
    let mut h = host("1.0.0");
    let bytes = history("2.0.0");
    let installs_before = h.model().session().unwrap().log().len();
    let outcome = h.open_history(&bytes);
    assert_eq!(outcome, ImportOutcome::ViewOnly(LoadReason::ContentMajor));
    assert_eq!(h.model().view_only_reason(), Some(LoadReason::ContentMajor));
    assert!(h.dirty_fields().contains(&"view_only_notice"));
    let text = notice(&h);
    assert!(!text.is_empty());
    assert!(text.contains("content_major"));
    assert_eq!(h.model().session().unwrap().log().len(), installs_before);
    assert_frozen(&mut h);
}

#[test]
fn 対照_表示だけでない読み込みでは同じ操作が状態を変える() {
    let mut h = host("1.0.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    assert_eq!(h.model().view_only_reason(), None);
    assert_eq!(notice(&h), "");
    let before = snapshot(&h);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_ne!(snapshot(&h), before, "対照が変化を起こさない");
    assert!(h.history_to_save().is_some());
}

#[test]
fn 書き出しは読み込んだバイト列そのものを返す() {
    let mut h = host("1.0.0");
    let bytes = history("2.0.0");
    assert!(matches!(h.open_history(&bytes), ImportOutcome::ViewOnly(_)));
    assert_eq!(h.export_live(), ExportGate::Ready(bytes.clone()));
    assert_frozen(&mut h);
    assert_eq!(h.export_live(), ExportGate::Ready(bytes));
}

#[test]
fn 起動時の復元でも表示だけになり_保存の値は出ない() {
    let mut h = host("1.0.0");
    let saved = history("2.0.0");
    assert_eq!(h.boot(Some(&saved)), BootKind::NotOpened);
    assert_eq!(h.model().view_only_reason(), Some(LoadReason::ContentMajor));
    assert!(notice(&h).contains("content_major"));
    assert_eq!(h.export_live(), ExportGate::Ready(saved));
    assert_frozen(&mut h);
}

#[test]
fn 壊れた履歴は起動時に表示だけにならず_従来どおり操作できる() {
    // 壊れた履歴（版の判定の段の外）は `corrupt-history` で案内する（TASK-333 1/2。`boot_corrupt_history.rs`）。ここでは表示だけのモードに入らないことだけを固定する。
    let mut h = host("1.0.0");
    assert_eq!(h.boot(Some(b"{broken")), BootKind::NotOpened);
    assert_eq!(h.model().view_only_reason(), None);
    assert_eq!(notice(&h), "");
    let before = snapshot(&h);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_ne!(snapshot(&h), before);
    assert!(
        h.history_to_save().is_none(),
        "起動時に開けなかった履歴は保存しない"
    );
}

#[test]
fn 開ける履歴を読み込むと表示だけから出る() {
    let mut h = host("1.0.0");
    assert!(matches!(
        h.open_history(&history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    assert_eq!(h.model().view_only_reason(), None);
    assert_eq!(notice(&h), "");
    assert!(h.dirty_fields().contains(&"view_only_notice"));
    let before = snapshot(&h);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_ne!(snapshot(&h), before);
    assert!(h.history_to_save().is_some());
}

#[test]
fn 通知の文に履歴の値は入らない() {
    // 履歴の教材の版（2.0.0）や pack.id は、固定の文言と理由のコードだけの通知に出ない。
    let mut h = host("1.0.0");
    let _ = h.open_history(&history("2.0.0"));
    let t = notice(&h);
    assert!(!t.contains("2.0.0"));
    assert!(!t.contains("min-3step"));
}

/// 秘密情報の警告と取り直しの要求を持ったまま、表示だけの履歴を読み込む。
fn held_then_view_only() -> (EngineHost, Vec<u8>, Rc<RefCell<Option<String>>>) {
    let (_, _, value) = secret_values::shapes().remove(0);
    let mut h = host("1.0.0");
    let store = Rc::new(RefCell::new(None));
    let s2 = Rc::clone(&store);
    h.set_history_saver(Box::new(move |v: &str| {
        *s2.borrow_mut() = Some(v.to_owned());
        Ok(())
    }));
    h.set_saved_refresh(Box::new(|_| {}));
    h.update(act("draft", &value));
    h.update(act("submit", ""));
    assert!(!h.model().secret_warning().is_empty(), "警告が出ない");
    h.update(act("secret-discard", ""));
    let bytes = history("2.0.0");
    assert!(matches!(h.open_history(&bytes), ImportOutcome::ViewOnly(_)));
    (h, bytes, store)
}

#[test]
fn 警告を持ったまま表示だけになると_破棄と保留の操作は何も変えない() {
    let (mut h, bytes, store) = held_then_view_only();
    assert!(h.model().secret_warning().is_empty(), "警告が残った");
    let before = snapshot(&h);
    let saved_before = store.borrow().clone();
    let epoch = h.save_epoch();
    let shape = fandhe_workshop_core::secret_scan::SecretShape::from_id(
        secret_values::shapes().remove(0).1.as_str(),
    )
    .unwrap();
    let hold = fandhe_workshop_web::secret_notice::encode_secret_hold(epoch, &[shape]);
    for (name, a) in [
        ("discard-read", act("secret-discard-read", "ok 0")),
        ("discard-read-1", act("secret-discard-read", "ok 1")),
        ("discard", act("secret-discard", "")),
        ("keep", act("secret-keep", "")),
        ("hold", act("secret-hold", &hold)),
        ("save-result", HostAction::SaveResult(epoch, None)),
    ] {
        h.update(a);
        assert_eq!(snapshot(&h), before, "{name} が状態・ログ・下書きを変えた");
        assert_eq!(h.model().view_only_reason(), Some(LoadReason::ContentMajor));
        assert!(
            h.model().secret_warning().is_empty(),
            "{name} が警告を出した"
        );
        assert_eq!(*store.borrow(), saved_before, "{name} が保存先を変えた");
        assert_eq!(h.export_live(), ExportGate::Ready(bytes.clone()), "{name}");
    }
}
