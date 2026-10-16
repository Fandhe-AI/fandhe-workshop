//! 合成の保存先 `HistoryStorage` への、口 `HistoryStore` の契約適合試験（TASK-317 2/2。TASK-310 の筋書きを web へ当てる）。
//!
//! 筋書きの出所は `crates/contract/tests/storage_contract.rs` の `exercise` で、二重の定義になる点を承知で写す
//! （contract の試験は integration test のため web から取り込めない。contract 側の構成は変えない）。
//! 対象は 3 つの状態: (a) IndexedDB 無し、(b) IndexedDB あり・localStorage のまま、(c) IndexedDB へ切り替えた後。
//! 同じオリジンの関係ない鍵が、一覧に出ず、教材ごとの削除で消えないことも確かめる。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore, StorageKind};
use fandhe_workshop_web::storage_select::SWITCH_THRESHOLD_UTF16;
use storage_fakes::{fixture, Fixture};

fn repo() -> DeliveryScope {
    DeliveryScope::named("repo")
}

/// `storage_contract.rs` の `exercise` と同じ筋書き。
fn exercise(store: &dyn HistoryStore) {
    let k1 = HistoryKey::for_pack(repo(), "a", "s");
    let k2 = HistoryKey::for_pack(repo(), "b", "s");
    let k3 = HistoryKey::for_pack(DeliveryScope::root(), "a", "s");
    store.write(&k1, "v1").unwrap();
    assert_eq!(store.read(&k1).unwrap().as_deref(), Some("v1"));
    store.write(&k1, "v2").unwrap();
    assert_eq!(store.read(&k1).unwrap().as_deref(), Some("v2"));
    store.write(&k2, "x").unwrap();
    store.write(&k3, "y").unwrap();
    let mut keys = store.keys().unwrap();
    keys.sort();
    let mut want = vec![k1.clone(), k2.clone(), k3.clone()];
    want.sort();
    assert_eq!(keys, want);
    // SAVE-32 相当: 教材 a の削除は、別の教材・別の区間の鍵を残す。
    for k in store.keys().unwrap() {
        if k.belongs_to(&repo(), "a") {
            store.remove(&k).unwrap();
        }
    }
    assert_eq!(store.read(&k1).unwrap(), None);
    assert!(store.read(&k2).unwrap().is_some());
    assert!(store.read(&k3).unwrap().is_some());
    // 無い鍵の削除は成功する。
    store.remove(&k1).unwrap();
}

fn with_unrelated(f: &Fixture) {
    for k in ["unrelated", "fandhe-workshop/meta/x"] {
        f.local
            .map
            .borrow_mut()
            .insert(k.to_string(), "z".to_string());
    }
}

fn assert_unrelated_kept(f: &Fixture) {
    assert_eq!(f.local.raw("unrelated").as_deref(), Some("z"));
    assert_eq!(f.local.raw("fandhe-workshop/meta/x").as_deref(), Some("z"));
}

#[test]
fn indexeddb_無しで契約の筋書きが通る() {
    let f = fixture(false);
    with_unrelated(&f);
    exercise(&f.storage);
    assert_unrelated_kept(&f);
}

#[test]
fn indexeddb_ありで_localstorage_のままでも契約の筋書きが通る() {
    let f = fixture(true);
    with_unrelated(&f);
    exercise(&f.storage);
    assert_unrelated_kept(&f);
    // 書き込みは IndexedDB へ行かない（remove は両方へ delete を発行するため、保留は delete だけである）。
    assert!(f.idb.store.borrow().is_empty());
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn indexeddb_へ切り替えた後でも契約の筋書きが通る() {
    let f = fixture(true);
    with_unrelated(&f);
    // 閾値を超える鍵で切り替えてから、確定を起こす。
    let pad = "x".repeat(usize::try_from(SWITCH_THRESHOLD_UTF16).unwrap());
    let switch = HistoryKey::for_pack(repo(), "switch", "s");
    f.storage.write(&switch, &pad).unwrap();
    f.idb.settle(Ok(()));
    f.storage.remove(&switch).unwrap();
    f.idb.settle(Ok(()));
    exercise(&f.storage);
    f.idb.settle(Ok(()));
    assert_unrelated_kept(&f);
}
