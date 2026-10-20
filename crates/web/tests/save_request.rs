//! 変更の後の遅延保存の通知（`EngineHost::set_save_request`）の試験（TASK-318。SAVE-7・SAVE-28）。
//! 通知が「履歴が変わった `update`」でだけ出ることと、変化の無い `update`・結んだだけでは出ないことを native で確かめる。
//! 試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 通知先を結んでいないホスト（開始直後の状態）。
fn bare_host() -> EngineHost {
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

/// 保存の通知を数えるホスト。
fn host() -> (EngineHost, Rc<Cell<u32>>) {
    let mut h = bare_host();
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    h.set_save_request(Box::new(move || c.set(c.get() + 1)));
    (h, n)
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

#[test]
fn 結んだだけでは通知しない() {
    let (_h, n) = host();
    assert_eq!(n.get(), 0);
}

#[test]
fn 下書きの変更ごとに_1_回通知する() {
    let (mut h, n) = host();
    h.update(act("draft", "a"));
    assert_eq!(n.get(), 1);
    h.update(act("draft", "ab"));
    assert_eq!(n.get(), 2);
}

#[test]
fn 下書きが変わらない_update_は通知しない() {
    let (mut h, n) = host();
    h.update(act("draft", "a"));
    let before = n.get();
    h.update(act("draft", "a"));
    h.update(act("draft", "a"));
    assert_eq!(n.get(), before);
}

#[test]
fn 状態を変える送信は即時の保存が書くので遅延保存の通知を出さない() {
    let (mut h, n) = host();
    h.update(act("draft", "x"));
    let before = n.get();
    h.update(act("submit", ""));
    assert_eq!(n.get(), before, "送信は変更のたびの保存（SAVE-10）が書くので通知は要らない");
}

#[test]
fn 通知先が無いホストの_update_は何も起きない() {
    let mut h = bare_host();
    h.update(act("draft", "a"));
    h.update(act("submit", ""));
}
