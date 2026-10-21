//! 履歴の保存先の選択の規則の試験（TASK-317 1/2。D-52・SAVE-8）。
//!
//! 手元の写しの偽物の backend で `HistoryStorage` を動かす。IndexedDB の偽物は、確定を保留しておき、
//! 試験が成功・失敗を手で起こせる（確定の前に localStorage の写しが残ることを確かめるため）。

use fandhe_workshop_contract::storage::{
    DeliveryScope, HistoryKey, HistoryStore, StorageError, StorageKind,
};
use fandhe_workshop_web::storage_select::{
    initial_kind, should_switch, utf16_len, Done, Fetched, HistoryStorage, IdbBackend, SyncKv,
    LOCAL_STORAGE_MAX_UTF16, SWITCH_THRESHOLD_UTF16,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// 2 周分の履歴の最大（F-5。バイト）。ASCII なら同数の UTF-16 の符号単位になる。
const TWO_ROUNDS_MAX_BYTES: u64 = 471_568;

#[derive(Default)]
struct FakeLocal {
    map: RefCell<BTreeMap<String, String>>,
    /// 全体の上限（鍵と値の UTF-16 の合計）。`None` なら無制限。
    limit: Cell<Option<u64>>,
    /// `true` なら `total_utf16` が失敗する。
    fail_total: Cell<bool>,
}

impl FakeLocal {
    fn total(&self) -> u64 {
        self.map
            .borrow()
            .iter()
            .map(|(k, v)| utf16_len(k) + utf16_len(v))
            .sum()
    }
    fn put_raw(&self, k: &str, v: &str) {
        self.map.borrow_mut().insert(k.to_string(), v.to_string());
    }
    fn has(&self, k: &str) -> bool {
        self.map.borrow().contains_key(k)
    }
}

impl SyncKv for FakeLocal {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        Ok(self.map.borrow().get(key).cloned())
    }
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        let old = self
            .map
            .borrow()
            .get(key)
            .map(|v| utf16_len(key) + utf16_len(v))
            .unwrap_or(0);
        let after = self.total() - old + utf16_len(key) + utf16_len(value);
        if let Some(l) = self.limit.get() {
            if after > l {
                return Err(StorageError::QuotaExceeded);
            }
        }
        self.put_raw(key, value);
        Ok(())
    }
    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.map.borrow_mut().remove(key);
        Ok(())
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        Ok(self.map.borrow().keys().cloned().collect())
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        if self.fail_total.get() {
            return Err(StorageError::Unavailable);
        }
        Ok(self.total())
    }
}

/// 保留中の操作。書き込みなら鍵と値、削除なら `None`。
type Pending = (Option<(String, String)>, Done);

#[derive(Default)]
struct FakeIdb {
    store: RefCell<BTreeMap<String, String>>,
    pending: RefCell<Vec<Pending>>,
    /// 次の `get` を失敗させる種類（1 回で消える）。
    pub get_error: Cell<Option<StorageError>>,
}

impl FakeIdb {
    /// 保留中の通知を、確定（`Ok`）か失敗で起こす。確定したときだけ保存先に入る。
    fn settle(&self, result: Result<(), StorageError>) {
        let items: Vec<_> = self.pending.borrow_mut().drain(..).collect();
        for (put, done) in items {
            if result.is_ok() {
                if let Some((k, v)) = put {
                    self.store.borrow_mut().insert(k, v);
                }
            }
            done(result);
        }
    }
    fn pending_len(&self) -> usize {
        self.pending.borrow().len()
    }
}

impl IdbBackend for FakeIdb {
    fn get(&self, key: &str, done: Fetched) {
        // 確定済みの書き込みだけが見える（保留中の put は store に入っていない）。
        match self.get_error.take() {
            Some(e) => done(Err(e)),
            None => done(Ok(self.store.borrow().get(key).cloned())),
        }
    }
    fn put(&self, key: &str, value: &str, done: Done) {
        self.pending
            .borrow_mut()
            .push((Some((key.to_string(), value.to_string())), done));
    }
    fn delete(&self, key: &str, done: Done) {
        self.store.borrow_mut().remove(key);
        self.pending.borrow_mut().push((None, done));
    }
}

type Storage = HistoryStorage<Rc<FakeLocal>, Rc<FakeIdb>>;

struct Fixture {
    local: Rc<FakeLocal>,
    idb: Rc<FakeIdb>,
    storage: Storage,
}

fn key(pack: &str) -> HistoryKey {
    HistoryKey::for_pack(DeliveryScope::named("repo"), pack, "s-0-1")
}

fn fixture_with(idb_initial: &[(&HistoryKey, &str)], idb_available: bool) -> Fixture {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let initial = idb_initial
        .iter()
        .map(|(k, v)| (k.render(), (*v).to_string()))
        .collect();
    let storage = HistoryStorage::new(
        Rc::clone(&local),
        idb_available.then(|| Rc::clone(&idb)),
        initial,
    );
    Fixture {
        local,
        idb,
        storage,
    }
}

fn fixture() -> Fixture {
    fixture_with(&[], true)
}

/// 閾値の手前までほかの鍵で詰める。残りが `room` の符号単位になる。
fn fill_to_threshold(f: &Fixture, room: u64) {
    let pad =
        "x".repeat(usize::try_from(SWITCH_THRESHOLD_UTF16 - room - f.local.total() - 1).unwrap());
    f.local.put_raw("a", &pad);
    assert_eq!(f.local.total(), SWITCH_THRESHOLD_UTF16 - room);
}

#[test]
fn 既定は_localstorage_で書き込みもそこに入る() {
    let f = fixture();
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
    let k = key("p");
    f.storage.write(&k, "v").unwrap();
    assert!(f.local.has(&k.render()));
    assert_eq!(f.idb.pending_len(), 0);
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn utf16_の数え方はバイト数でも文字数でもない() {
    assert_eq!(utf16_len("abc"), 3);
    assert_eq!(utf16_len("あい"), 2);
    assert_eq!("あい".len(), 6);
    assert_eq!(utf16_len("😀"), 2);
    assert_eq!("😀".chars().count(), 1);
    assert_eq!("😀".len(), 4);
}

#[test]
fn 二周分の最大は閾値に対して十分な余裕がある() {
    const {
        assert!(TWO_ROUNDS_MAX_BYTES * 8 < SWITCH_THRESHOLD_UTF16);
        assert!(SWITCH_THRESHOLD_UTF16 < LOCAL_STORAGE_MAX_UTF16);
    }
    let f = fixture();
    let v = "x".repeat(usize::try_from(TWO_ROUNDS_MAX_BYTES).unwrap());
    f.storage.write(&key("p"), &v).unwrap();
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn 閾値ちょうどは切り替えず一つ超えると切り替える() {
    assert!(!should_switch(SWITCH_THRESHOLD_UTF16));
    assert!(should_switch(SWITCH_THRESHOLD_UTF16 + 1));

    let k = key("p");
    let klen = utf16_len(&k.render());
    // 残りがちょうど「鍵 + 値」になる大きさの値は、入る。
    let f = fixture();
    fill_to_threshold(&f, klen + 3);
    f.storage.write(&k, "abc").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
    // 1 つ大きいと切り替わる。
    let f = fixture();
    fill_to_threshold(&f, klen + 3);
    f.storage.write(&k, "abcd").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
}

#[test]
fn 見込みは同じ鍵の旧い値を二重に数えない() {
    let f = fixture();
    let k = key("p");
    let big = "x".repeat(usize::try_from(SWITCH_THRESHOLD_UTF16 / 2 + 10).unwrap());
    f.storage.write(&k, &big).unwrap();
    // 旧い値を足したままなら閾値を超えるが、置き換えなので超えない。
    f.storage.write(&k, &big).unwrap();
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn ほかの鍵の量も見込みに含める() {
    let f = fixture();
    fill_to_threshold(&f, 10);
    f.storage.write(&key("p"), "小さな履歴です").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
}

#[test]
fn 切り替えは一方向で小さな値でも戻らない() {
    let f = fixture();
    // 想定外の上限超過で切り替える。切り替えの後は使用量が小さくなる（局所の写しが無い）。
    f.local.limit.set(Some(50));
    f.storage.write(&key("p"), &"x".repeat(200)).unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    f.local.limit.set(None);
    f.storage.write(&key("q"), "s").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    assert!(!f.local.has(&key("q").render()));
    assert_eq!(f.idb.pending_len(), 2);
}

#[test]
fn 確定の前は_localstorage_の写しが残り確定の後に消える() {
    let f = fixture();
    let k = key("p");
    // 使用量の見積もりは最初の書き込みで決まるため、詰め物を先に入れてから旧い値を書く。
    fill_to_threshold(&f, 5 + utf16_len(&k.render()) + 3);
    f.storage.write(&k, "old").unwrap();
    f.storage.write(&k, "0123456789").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    // 確定前: 旧い写しが残り、読みは手元の写しの新しい値を返す。
    assert!(f.local.has(&k.render()));
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("0123456789"));
    f.idb.settle(Ok(()));
    assert!(!f.local.has(&k.render()));
    assert_eq!(
        f.idb.store.borrow().get(&k.render()).map(String::as_str),
        Some("0123456789")
    );
    assert_eq!(f.storage.last_async_error(), None);
}

#[test]
fn put_の失敗では_localstorage_の写しが残り種類だけが記録される() {
    let f = fixture();
    let k = key("p");
    // 使用量の見積もりは最初の書き込みで決まるため、詰め物を先に入れてから旧い値を書く。
    fill_to_threshold(&f, 5 + utf16_len(&k.render()) + 3);
    f.storage.write(&k, "old").unwrap();
    f.storage.write(&k, "0123456789").unwrap();
    f.idb.settle(Err(StorageError::QuotaExceeded));
    assert!(f.local.has(&k.render()));
    assert_eq!(
        f.storage.last_async_error(),
        Some(StorageError::QuotaExceeded)
    );
}

#[test]
fn 起動時は片方だけならその側_両方なら_indexeddb_を正とする() {
    let k = key("p");
    // どちらにも無い: 既定。
    let f = fixture();
    assert_eq!(f.storage.kind_for(&k), StorageKind::LocalStorage);
    // localStorage だけ。
    let f = fixture();
    f.local.put_raw(&k.render(), "L");
    assert_eq!(f.storage.kind_for(&k), StorageKind::LocalStorage);
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("L"));
    // IndexedDB だけ。
    let f = fixture_with(&[(&k, "I")], true);
    assert_eq!(f.storage.kind_for(&k), StorageKind::IndexedDb);
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("I"));
    // 両方: IndexedDB が正。
    let f = fixture_with(&[(&k, "I")], true);
    f.local.put_raw(&k.render(), "L");
    assert_eq!(f.storage.kind_for(&k), StorageKind::IndexedDb);
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("I"));
    assert_eq!(initial_kind(true), StorageKind::IndexedDb);
    assert_eq!(initial_kind(false), StorageKind::LocalStorage);
}

#[test]
fn 想定外の上限超過は_indexeddb_が使えれば切り替えて書く() {
    let f = fixture();
    f.local.limit.set(Some(50));
    let k = key("p");
    f.storage.write(&k, &"x".repeat(200)).unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    assert!(!f.local.has(&k.render()));
    f.idb.settle(Ok(()));
    assert_eq!(f.idb.store.borrow().len(), 1);
}

#[test]
fn 想定外の上限超過は_indexeddb_が無ければそのまま返す() {
    let f = fixture_with(&[], false);
    f.local.limit.set(Some(50));
    assert_eq!(
        f.storage.write(&key("p"), &"x".repeat(200)),
        Err(StorageError::QuotaExceeded)
    );
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
}

#[test]
fn indexeddb_が使えないとき閾値を超える見込みでも_localstorage_へ書く() {
    let f = fixture_with(&[], false);
    assert!(!f.storage.idb_available());
    fill_to_threshold(&f, 5);
    let k = key("p");
    f.storage.write(&k, "0123456789").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::LocalStorage);
    assert!(f.local.has(&k.render()));
}

#[test]
fn keys_は和集合で重複せず履歴の鍵だけを返す() {
    let (a, b, c) = (key("a"), key("b"), key("c"));
    let f = fixture_with(&[(&b, "1"), (&c, "2")], true);
    f.local.put_raw(&a.render(), "1");
    f.local.put_raw(&b.render(), "old");
    f.local.put_raw("unrelated", "x");
    f.local.put_raw("fandhe-workshop/meta/x", "x");
    let mut got: Vec<String> = f
        .storage
        .keys()
        .unwrap()
        .iter()
        .map(HistoryKey::render)
        .collect();
    got.sort();
    let mut want = vec![a.render(), b.render(), c.render()];
    want.sort();
    assert_eq!(got, want);
}

#[test]
fn remove_は両方から消し別の教材と別の配信先の鍵を残す() {
    let (a, b) = (key("a"), key("b"));
    let other_scope = HistoryKey::for_pack(DeliveryScope::named("other"), "a", "s-0-1");
    let f = fixture_with(&[(&a, "I")], true);
    f.local.put_raw(&a.render(), "L");
    f.local.put_raw(&b.render(), "B");
    f.local.put_raw(&other_scope.render(), "O");
    assert_eq!(f.storage.kind_for(&a), StorageKind::IndexedDb);
    f.storage.remove(&a).unwrap();
    assert_eq!(f.storage.read(&a).unwrap(), None);
    assert!(!f.local.has(&a.render()));
    assert!(f.idb.store.borrow().get(&a.render()).is_none());
    assert_eq!(f.storage.read(&b).unwrap().as_deref(), Some("B"));
    assert_eq!(f.storage.read(&other_scope).unwrap().as_deref(), Some("O"));
    // 鍵が無くても Ok。
    f.storage.remove(&key("none")).unwrap();
}

#[test]
fn kind_for_を呼ぶ前でも_indexeddb_にある鍵への書き込みは_indexeddb_へ行く() {
    let k = key("p");
    let f = fixture_with(&[(&k, "old")], true);
    f.storage.write(&k, "new").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    assert!(!f.local.has(&k.render()));
    assert_eq!(f.idb.pending_len(), 1);
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("new"));
}

#[test]
fn 全走査に失敗した見積もりは_cache_せず悲観側で_indexeddb_へ切り替える() {
    let f = fixture();
    f.local.fail_total.set(true);
    let k = key("p");
    f.storage.write(&k, "v").unwrap();
    assert_eq!(f.storage.kind(), StorageKind::IndexedDb);
    assert!(!f.local.has(&k.render()));
}

/// 取り直しの結果を受ける入れ物。
fn reload_result(f: &Fixture, k: &HistoryKey) -> Result<(), StorageError> {
    let out = Rc::new(Cell::new(None));
    let o = Rc::clone(&out);
    f.storage.refetch(k, Box::new(move |r| o.set(Some(r))));
    out.take().expect("done was not called")
}

#[test]
fn 取り直しは別のタブが保存した内容を写しに取り込む() {
    let k = key("p");
    let f = fixture_with(&[(&k, "from-start")], true);
    f.idb
        .store
        .borrow_mut()
        .insert(k.render(), "from-start".to_string());
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("from-start"));
    // 別のタブが保存した（DB の実データだけが変わり、このタブの写しは古いまま）。
    f.idb
        .store
        .borrow_mut()
        .insert(k.render(), "other-tab".to_string());
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("from-start"));
    assert_eq!(reload_result(&f, &k), Ok(()));
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("other-tab"));
}

#[test]
fn 取り直しは失敗した書き込みの未保存の値を写しから消す() {
    let k = key("p");
    let f = fixture_with(&[(&k, "saved")], true);
    f.idb
        .store
        .borrow_mut()
        .insert(k.render(), "saved".to_string());
    f.storage.write(&k, "unsaved").unwrap();
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("unsaved"));
    f.idb.settle(Err(StorageError::Failed));
    // 失敗しても写しは未保存の値を持つ（既存の限界）。取り直しで DB の実データに戻る。
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("unsaved"));
    assert_eq!(reload_result(&f, &k), Ok(()));
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("saved"));
}

#[test]
fn 取り直しの失敗は写しを変えず失敗を返す() {
    let k = key("p");
    let f = fixture_with(&[(&k, "saved")], true);
    f.idb
        .store
        .borrow_mut()
        .insert(k.render(), "db".to_string());
    f.idb.get_error.set(Some(StorageError::Failed));
    assert_eq!(reload_result(&f, &k), Err(StorageError::Failed));
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("saved"));
}

#[test]
fn 取り直しは_indexeddb_が無ければ即座に成功し_localstorage_を変えない() {
    let k = key("p");
    let f = fixture_with(&[], false);
    f.storage.write(&k, "v").unwrap();
    assert_eq!(reload_result(&f, &k), Ok(()));
    assert_eq!(f.storage.read(&k).unwrap().as_deref(), Some("v"));
}
