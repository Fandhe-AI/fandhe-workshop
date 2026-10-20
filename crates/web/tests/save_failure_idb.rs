//! IndexedDB への保存の失敗の表示の試験（SAVE-11・SHELL-19・TASK-323 2/2）。
//!
//! IndexedDB へ切り替えた後の書き込み（put）は確定が非同期で届く。(1) 失敗すると次の `update` で理由が `save-notice` に出る、
//! (2) メモリ上の状態は進み、IndexedDB の既存の値と localStorage の写しは変わらない、(3) 後の put の成功で表示が消える、
//! (4) 開けたが全件を読めなかった保存先は最初から理由を出し、保存も試さず既存の値を変えない、ことを、
//! ホストの probe と保存先の偽物を通して確かめる。試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore, StorageError};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::save_pending::PendingSave;
use fandhe_workshop_web::storage_select::HistoryStorage;
use storage_fakes::{fixture, FakeIdb, FakeLocal, Fixture};

const QUOTA_TEXT: &str = "保存できませんでした（QuotaExceededError）";
const FAILED_TEXT: &str = "保存できませんでした（書き込みに失敗しました）";
const UNAVAILABLE_TEXT: &str = "保存できませんでした（保存領域を使えません）";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

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

/// IndexedDB を使える保存先と、saver・probe つきのホスト。probe は `persist::async_save_failure` の native 版。
fn setup() -> (EngineHost, Rc<Fixture>, HistoryKey) {
    let f = Rc::new(fixture(true));
    let mut h = page();
    let key = h.history_key(DeliveryScope::named("repo")).unwrap();
    let pending = Rc::new(PendingSave::new());
    let (f2, k2) = (Rc::clone(&f), key.clone());
    h.set_history_saver(Box::new(move |v| {
        pending.stage(k2.clone(), v.to_owned());
        match pending.flush(&f2.storage).first_error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }));
    let f3 = Rc::clone(&f);
    h.set_save_failure_probe(Box::new(move || f3.storage.last_async_error()));
    (h, f, key)
}

fn step(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
}

/// 打鍵だけの `update`（保存は試さない）。表示の求め直しだけが走る。
fn keystroke(h: &mut EngineHost) {
    h.update(act("draft", "x"));
}

fn notice(h: &EngineHost) -> String {
    match h.registry().bound_value("save_notice", h.model()) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("save_notice の束縛が無い: {other:?}"),
    }
}

fn log_len(h: &EngineHost) -> usize {
    h.model().session().unwrap().log().len()
}

/// 最初の保存を IndexedDB へ向け、確定させる。以後の保存は IndexedDB へ行く。確定した値を返す。
fn switch_to_idb(h: &mut EngineHost, f: &Fixture, key: &HistoryKey) -> String {
    f.local.limit.set(Some(0));
    step(h);
    f.idb.settle(Ok(()));
    let saved = f.idb.store.borrow().get(&key.render()).cloned().unwrap();
    f.local.limit.set(None);
    saved
}

#[test]
fn 確定の失敗は次の_update_で理由が出て状態は進み既存の保存は変わらない() {
    let (mut h, f, key) = setup();
    let saved = switch_to_idb(&mut h, &f, &key);
    assert_eq!(notice(&h), "");
    let before = log_len(&h);

    step(&mut h);
    f.idb.settle(Err(StorageError::QuotaExceeded));
    keystroke(&mut h);
    assert_eq!(h.model().save_failure(), Some(StorageError::QuotaExceeded));
    assert_eq!(notice(&h), QUOTA_TEXT);
    assert!(h.dirty_fields().contains(&"save_notice"));
    assert!(log_len(&h) > before, "操作は適用され状態は進む");
    assert_eq!(
        f.idb.store.borrow().get(&key.render()),
        Some(&saved),
        "IndexedDB の既存の値は変わらない"
    );

    // 続く間は値が変わらないので dirty に積まない。
    keystroke(&mut h);
    assert!(!h.dirty_fields().contains(&"save_notice"));
}

#[test]
fn 理由の種類ごとの文になる() {
    let (mut h, f, key) = setup();
    switch_to_idb(&mut h, &f, &key);
    step(&mut h);
    f.idb.settle(Err(StorageError::Failed));
    keystroke(&mut h);
    assert_eq!(notice(&h), FAILED_TEXT);
}

#[test]
fn 後の書き込みの確定で表示が消える() {
    let (mut h, f, key) = setup();
    switch_to_idb(&mut h, &f, &key);
    step(&mut h);
    f.idb.settle(Err(StorageError::QuotaExceeded));
    keystroke(&mut h);
    assert_eq!(notice(&h), QUOTA_TEXT);

    step(&mut h);
    let want = h.history_to_save();
    f.idb.settle(Ok(()));
    assert_eq!(f.storage.last_async_error(), None);
    keystroke(&mut h);
    assert_eq!(h.model().save_failure(), None);
    assert_eq!(notice(&h), "");
    assert!(h.dirty_fields().contains(&"save_notice"));
    assert_eq!(f.idb.store.borrow().get(&key.render()).cloned(), want);
}

#[test]
fn 失敗しても_localstorage_の写しは消えない() {
    let (mut h, f, key) = setup();
    // 1 手目は localStorage に書く。2 手目だけ IndexedDB へ向け、確定を失敗させる。
    step(&mut h);
    let local_before = f.local.raw(&key.render()).unwrap();
    f.local.limit.set(Some(f.local.total()));
    step(&mut h);
    f.local.limit.set(None);
    f.idb.settle(Err(StorageError::Failed));
    assert_eq!(
        f.local.raw(&key.render()),
        Some(local_before),
        "確定の失敗では localStorage の写しを消さない"
    );
    keystroke(&mut h);
    assert_eq!(notice(&h), FAILED_TEXT);
}

#[test]
fn 全件を読めなかった保存先は最初から理由を出し保存を試さず既存の値を変えない() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let mut h = page();
    let key = h.history_key(DeliveryScope::named("repo")).unwrap();
    local
        .map
        .borrow_mut()
        .insert(key.render(), "old".to_owned());
    let storage: HistoryStorage<Rc<FakeLocal>, Rc<FakeIdb>> =
        HistoryStorage::unreadable(Rc::clone(&local));
    assert!(storage.idb_unreadable());
    assert_eq!(storage.read(&key), Err(StorageError::Unavailable));
    assert_eq!(storage.write(&key, "new"), Err(StorageError::Unavailable));
    assert_eq!(local.raw(&key.render()), Some("old".to_owned()));
    assert_eq!(idb.pending_len(), 0);

    let storage = Rc::new(storage);
    let s2 = Rc::clone(&storage);
    h.set_save_failure_probe(Box::new(move || {
        s2.idb_unreadable().then_some(StorageError::Unavailable)
    }));
    assert_eq!(h.model().save_failure(), Some(StorageError::Unavailable));
    assert_eq!(notice(&h), UNAVAILABLE_TEXT);
}
