//! 変更のたびの保存の失敗の表示の試験（SAVE-10・SHELL-18・TASK-323 1/2）。
//!
//! localStorage だけの保存先（IndexedDB が無い）で容量が足りないとき、(1) 理由が `save-notice` に出る、
//! (2) メモリ上の状態は進み、保存領域には直前に保存できた値が残る、(3) 容量を空けた後の次の変更で保存に成功し表示が消える、
//! ことを、web の保存の経路（ホストの saver・保留と書き出し・保存先の偽物）を通して確かめる。
//! 下書きだけの変更・未操作・保存停止では保存を試さないことも確かめる。試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, StorageError};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::save_pending::PendingSave;
use storage_fakes::{fixture, Fixture};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// SAVE-10 の文言（定数を参照せず、文言そのものを固定する）。
const QUOTA_TEXT: &str = "保存できませんでした（QuotaExceededError）";

fn page() -> EngineHost {
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

/// 保存先（IndexedDB 無し）と saver つきのホスト。`persist::save_now` の native 版を注入する。
/// 戻りの `Cell` は saver が呼ばれた回数。
fn setup() -> (EngineHost, Rc<Fixture>, HistoryKey, Rc<Cell<u32>>) {
    let f = Rc::new(fixture(false));
    let mut h = page();
    let key = h.history_key(DeliveryScope::named("repo")).unwrap();
    let pending = Rc::new(PendingSave::new());
    let calls = Rc::new(Cell::new(0));
    let (f2, k2, c2) = (Rc::clone(&f), key.clone(), Rc::clone(&calls));
    h.set_history_saver(Box::new(move |v| {
        c2.set(c2.get() + 1);
        pending.stage(k2.clone(), v.to_owned());
        match pending.flush(&f2.storage).first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }));
    (h, f, key, calls)
}

/// 状態を変える操作 1 回（受け付けられる 1 手）。
fn step(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
}

fn log_len(h: &EngineHost) -> usize {
    h.model().session().unwrap().log().len()
}

fn notice(h: &EngineHost) -> String {
    match h.registry().bound_value("save_notice", h.model()) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("save_notice の束縛が無い: {other:?}"),
    }
}

fn stored(f: &Fixture) -> Vec<String> {
    f.local.map.borrow().values().cloned().collect()
}

#[test]
fn 未操作では保存を試さず表示も空() {
    let (mut h, f, _, calls) = setup();
    assert_eq!(notice(&h), "");
    h.update(act("draft", ""));
    assert_eq!(calls.get(), 0);
    assert!(f.local.map.borrow().is_empty());
    assert_eq!(h.model().save_failure(), None);
}

#[test]
fn 状態を変える操作のたびに保存され値は保存する値と等しい() {
    let (mut h, f, key, calls) = setup();
    step(&mut h);
    assert!(calls.get() >= 1);
    assert_eq!(h.model().save_failure(), None);
    assert_eq!(notice(&h), "");
    let want = h.history_to_save().unwrap();
    assert_eq!(f.local.raw(&key.render()), Some(want));
}

#[test]
fn 容量超過で理由が出て状態は進み既存の保存は変わらず_空けた後の次の変更で消える() {
    let (mut h, f, _, _) = setup();
    step(&mut h);
    let saved = stored(&f);
    assert_eq!(saved.len(), 1);
    let before_len = log_len(&h);

    // 今の合計を上限にする。次の保存の値は大きくなるため入らない。
    f.local.limit.set(Some(f.local.total()));
    step(&mut h);
    assert_eq!(h.model().save_failure(), Some(StorageError::QuotaExceeded));
    assert_eq!(notice(&h), QUOTA_TEXT);
    assert!(h.dirty_fields().contains(&"save_notice"));
    assert!(log_len(&h) > before_len, "操作は適用され状態は進む");
    assert_eq!(stored(&f), saved, "保存領域には直前に保存できた値が残る");

    // 失敗が続く間は、値が変わらないので dirty に積まない。
    step(&mut h);
    assert_eq!(notice(&h), QUOTA_TEXT);
    assert!(!h.dirty_fields().contains(&"save_notice"));
    assert_eq!(stored(&f), saved);

    // 容量を空けた後の次の変更で保存に成功し、表示が消える。
    f.local.limit.set(None);
    step(&mut h);
    assert_eq!(h.model().save_failure(), None);
    assert_eq!(notice(&h), "");
    assert!(h.dirty_fields().contains(&"save_notice"));
    assert_eq!(stored(&f), vec![h.history_to_save().unwrap()]);
}

#[test]
fn 下書きだけの変更では保存を試さない() {
    let (mut h, _, _, calls) = setup();
    step(&mut h);
    let n = calls.get();
    h.update(act("draft", "/make"));
    h.update(act("draft", "/make-note"));
    assert_eq!(calls.get(), n);
}

#[test]
fn 保存停止のときは保存を試さず表示も変えない() {
    let (mut h, f, _, calls) = setup();
    h.boot(Some(b"not a history file"));
    step(&mut h);
    assert_eq!(calls.get(), 0);
    assert!(f.local.map.borrow().is_empty());
    assert_eq!(notice(&h), "");
}

#[test]
fn 遅延保存の結果の_action_が通知を出し_成功で消す() {
    let (mut h, _, _, _) = setup();
    h.update(act("save-result", "0 quota"));
    assert_eq!(h.model().save_failure(), Some(StorageError::QuotaExceeded));
    assert_eq!(notice(&h), QUOTA_TEXT);
    assert!(h.dirty_fields().contains(&"save_notice"));
    h.update(act("save-result", "0 ok"));
    assert_eq!(h.model().save_failure(), None);
    assert_eq!(notice(&h), "");
}

#[test]
fn 遅延保存の結果の未知の_payload_は受けない() {
    assert!(EngineHost::decode_action("save-result", "").is_none());
    assert!(EngineHost::decode_action("save-result", "OK").is_none());
    assert!(EngineHost::decode_action("save-result", "ok").is_none());
    assert!(EngineHost::decode_action("save-result", "01 ok").is_none());
    assert!(EngineHost::decode_action("save-result", "-1 ok").is_none());
    assert!(EngineHost::decode_action("save-result", "1 OK").is_none());
    assert!(EngineHost::decode_action("save-result", "99999999999999999999 ok").is_none());
}

#[test]
fn 配送待ちの古い成功は_後の即時保存の失敗の通知を消さない() {
    let (mut h, f, _, _) = setup();
    step(&mut h);
    // 遅延保存がこの世代で成功し、結果の配送はまだ。
    let stale = h.save_epoch();
    f.local.limit.set(Some(f.local.total()));
    step(&mut h);
    assert_eq!(notice(&h), QUOTA_TEXT);
    h.update(act("save-result", &format!("{stale} ok")));
    assert_eq!(h.model().save_failure(), Some(StorageError::QuotaExceeded));
    assert_eq!(notice(&h), QUOTA_TEXT);
}

#[test]
fn 配送待ちの古い失敗は_後の即時保存の成功の後に表示されない() {
    let (mut h, f, _, _) = setup();
    step(&mut h);
    let stale = h.save_epoch();
    f.local.limit.set(Some(f.local.total()));
    step(&mut h);
    f.local.limit.set(None);
    step(&mut h);
    assert_eq!(notice(&h), "");
    h.update(act("save-result", &format!("{stale} quota")));
    assert_eq!(h.model().save_failure(), None);
    assert_eq!(notice(&h), "");
    // 今の世代の結果は反映される。
    h.update(act("save-result", &format!("{} quota", h.save_epoch())));
    assert_eq!(notice(&h), QUOTA_TEXT);
}
