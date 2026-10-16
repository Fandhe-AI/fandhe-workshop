//! 離れる直前の書き出し（保留と `flush`）の試験（TASK-317 2/2。SAVE-9）。
//!
//! `pagehide` のリスナー自体はブラウザ試験（`tools/browser/storage-leave.test.mjs`）が確かめる。ここでは、
//! リスナーが呼ぶ `PendingSave::flush` の規則（最後の値だけ・同期・失敗で残す・二重に書かない）を、
//! 手元の写しの偽物の保存先に当てて固定する。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use fandhe_workshop_contract::storage::{HistoryStore, StorageError, StorageKind};
use fandhe_workshop_web::save_pending::PendingSave;
use fandhe_workshop_web::storage_select::SWITCH_THRESHOLD_UTF16;
use storage_fakes::{fixture, key};

#[test]
fn 同じ鍵に_2_回積むと最後の値だけを_1_回書く() {
    let f = fixture(false);
    let p = PendingSave::new();
    let k = key("p");
    p.stage(k.clone(), "first".to_string());
    p.stage(k.clone(), "last".to_string());
    assert_eq!(p.pending_len(), 1);
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 1);
    assert_eq!(out.failed, 0);
    assert_eq!(out.first_error, None);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("last"));
}

#[test]
fn 積むだけでは保存先に書かない() {
    let f = fixture(false);
    let p = PendingSave::new();
    let k = key("p");
    p.stage(k.clone(), "v".to_string());
    assert!(p.has_pending());
    assert_eq!(f.local.raw(&k.render()), None);
}

#[test]
fn 保留が空なら何も書かずに_0_件を返す() {
    let f = fixture(false);
    let p = PendingSave::new();
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 0);
    assert_eq!(out.failed, 0);
    assert!(f.local.map.borrow().is_empty());
}

#[test]
fn flush_の直後に_indexeddb_の確定を待たずに_localstorage_へ入っている() {
    let f = fixture(true);
    let p = PendingSave::new();
    let k = key("p");
    p.stage(k.clone(), "v".to_string());
    p.flush(&f.storage);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("v"));
    assert_eq!(f.idb.pending_len(), 0);
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn 書き込みの失敗では保留に残り再びの_flush_で書ける() {
    let f = fixture(false);
    let k = key("p");
    f.storage.write(&k, "old").unwrap();
    f.local.limit.set(Some(f.local.total()));
    let p = PendingSave::new();
    p.stage(k.clone(), "new-and-longer".to_string());
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 0);
    assert_eq!(out.failed, 1);
    assert_eq!(out.first_error, Some(StorageError::QuotaExceeded));
    assert_eq!(p.pending_len(), 1);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("old"));
    f.local.limit.set(None);
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 1);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("new-and-longer"));
    assert!(!p.has_pending());
}

#[test]
fn 別の鍵は互いに置き換わらない() {
    let f = fixture(false);
    let p = PendingSave::new();
    let (a, b) = (key("a"), key("b"));
    p.stage(a.clone(), "va".to_string());
    p.stage(b.clone(), "vb".to_string());
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 2);
    assert_eq!(f.local.raw(&a.render()).as_deref(), Some("va"));
    assert_eq!(f.local.raw(&b.render()).as_deref(), Some("vb"));
}

#[test]
fn flush_の後に保留は空になり_2_回目は書かない() {
    let f = fixture(false);
    let p = PendingSave::new();
    let k = key("p");
    p.stage(k.clone(), "v".to_string());
    assert_eq!(p.flush(&f.storage).written, 1);
    assert!(!p.has_pending());
    // 保存先の値を外から変えても、2 回目の flush は書き戻さない。
    f.local
        .map
        .borrow_mut()
        .insert(k.render(), "other".to_string());
    assert_eq!(p.flush(&f.storage).written, 0);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("other"));
}

#[test]
fn indexeddb_へ切り替えた後の_flush_は_put_を発行し写しは新しい値を返す() {
    let f = fixture(true);
    let k = key("p");
    // 閾値を超える見込みにして、書き込みを IndexedDB へ切り替えさせる。
    let pad = "x".repeat(usize::try_from(SWITCH_THRESHOLD_UTF16).unwrap());
    f.local.map.borrow_mut().insert("pad".to_string(), pad);
    let p = PendingSave::new();
    p.stage(k.clone(), "v".to_string());
    let out = p.flush(&f.storage);
    assert_eq!(out.written, 1);
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    // 確定は保証しない（限界）。put は発行済みで、確定の前に手元の写しは新しい値を返す。
    assert_eq!(f.idb.pending_len(), 1);
    assert!(f.idb.store.borrow().is_empty());
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("v"));
    f.idb.settle(Ok(()));
    assert_eq!(
        f.idb.store.borrow().get(&k.render()).map(String::as_str),
        Some("v")
    );
}
