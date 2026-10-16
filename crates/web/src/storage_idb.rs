//! IndexedDB の backend と、非同期に開く入口（TASK-317 1/2）。
//!
//! `storage_select::HistoryStorage` が [`IdbBackend`] として使う薄い包み。DB 名 `fandhe-workshop`・版 1・
//! object store `history`（鍵は `HistoryKey::render()` の文字列）。選択の規則は `storage_select` が持つ。
//! [`open_history_storage`] は、開いて全件を手元の写しへ読み込んだ後に `on_ready` を呼ぶ。これが準備完了の合図で、
//! 復元の経路（TASK-319・TASK-331）と離れる直前の書き込み（TASK-317 2/2）はこの合図の後に使う。
//! 開けない（`indexedDB` が無い・エラー・他のタブに塞がれた）ときは、IndexedDB 無しで `on_ready` を呼ぶ（localStorage だけで動く）。
//!
//! 完了・失敗の通知は JS のイベントで届く。ハンドラは `Closure::once_into_js`（1 回呼ばれると自分で解放される形）で作る。
//! 同じ通知を複数のイベントに掛けるため、呼ばれなかった側は要求の寿命の間だけ残る（小さく、要求とともに GC される）。
//! 例外の文は読まない・記録しない。DOM の値は読まない（`DomException::name` と要求の結果だけを使う）。

use crate::storage_local::LocalStore;
use crate::storage_select::{Done, HistoryStorage, IdbBackend};
use fandhe_workshop_contract::storage::{HistoryKey, StorageError};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::js_sys::{Array, Function};
use web_sys::{IdbDatabase, IdbRequest, IdbTransaction, IdbTransactionMode};

const DB_NAME: &str = "fandhe-workshop";
const DB_VERSION: u32 = 1;
const STORE: &str = "history";

/// 開いた IndexedDB の包み。
pub struct IdbStore {
    db: IdbDatabase,
}

/// 1 回だけ呼ぶ通知を、複数のイベントから共有するための入れ物。
type OnceCell = Rc<RefCell<Option<Done>>>;

fn fire(cell: &OnceCell, r: Result<(), StorageError>) {
    let taken = cell.borrow_mut().take();
    if let Some(done) = taken {
        done(r);
    }
}

fn handler(f: impl FnOnce() + 'static) -> JsValue {
    Closure::once_into_js(f)
}

fn as_fn(v: &JsValue) -> &Function {
    v.unchecked_ref()
}

/// トランザクションの完了・失敗・中断を `done` へ流す。中断は例外の `name` で上限超過かを見分ける。
fn watch(tx: &IdbTransaction, done: Done) {
    let cell: OnceCell = Rc::new(RefCell::new(Some(done)));
    let c = Rc::clone(&cell);
    tx.set_oncomplete(Some(as_fn(&handler(move || fire(&c, Ok(()))))));
    let c = Rc::clone(&cell);
    tx.set_onerror(Some(as_fn(&handler(move || {
        fire(&c, Err(StorageError::Failed))
    }))));
    let c = Rc::clone(&cell);
    let tx2 = tx.clone();
    tx.set_onabort(Some(as_fn(&handler(move || {
        let quota = tx2
            .error()
            .map(|e| e.name() == "QuotaExceededError")
            .unwrap_or(false);
        fire(
            &c,
            Err(if quota {
                StorageError::QuotaExceeded
            } else {
                StorageError::Failed
            }),
        )
    }))));
}

impl IdbStore {
    fn start(&self, done: Done) -> Option<(IdbTransaction, Done)> {
        match self
            .db
            .transaction_with_str_and_mode(STORE, IdbTransactionMode::Readwrite)
        {
            Ok(tx) => Some((tx, done)),
            Err(_) => {
                done(Err(StorageError::Failed));
                None
            }
        }
    }
}

impl IdbBackend for IdbStore {
    fn put(&self, key: &str, value: &str, done: Done) {
        let Some((tx, done)) = self.start(done) else {
            return;
        };
        let issued = tx
            .object_store(STORE)
            .and_then(|s| s.put_with_key(&JsValue::from_str(value), &JsValue::from_str(key)));
        match issued {
            Ok(_) => watch(&tx, done),
            Err(_) => done(Err(StorageError::Failed)),
        }
    }

    fn delete(&self, key: &str, done: Done) {
        let Some((tx, done)) = self.start(done) else {
            return;
        };
        let issued = tx
            .object_store(STORE)
            .and_then(|s| s.delete(&JsValue::from_str(key)));
        match issued {
            Ok(_) => watch(&tx, done),
            Err(_) => done(Err(StorageError::Failed)),
        }
    }
}

/// 開いた結果の合成の保存先の型。
pub type OpenedStorage = HistoryStorage<LocalStore, IdbStore>;

type Ready = Rc<RefCell<Option<Box<dyn FnOnce(OpenedStorage)>>>>;

fn finish(ready: &Ready, idb: Option<(IdbStore, BTreeMap<String, String>)>) {
    let taken = ready.borrow_mut().take();
    if let Some(f) = taken {
        let storage = match idb {
            Some((store, initial)) => HistoryStorage::new(LocalStore::new(), Some(store), initial),
            None => HistoryStorage::new(LocalStore::new(), None, BTreeMap::new()),
        };
        f(storage);
    }
}

/// 全件の鍵と値から、履歴の鍵として読める文字列の組だけを手元の写しへ入れる。
fn collect(keys: &IdbRequest, values: &IdbRequest) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let (Ok(k), Ok(v)) = (keys.result(), values.result()) else {
        return out;
    };
    let (k, v): (Array, Array) = (k.unchecked_into(), v.unchecked_into());
    for i in 0..k.length() {
        if let (Some(key), Some(val)) = (k.get(i).as_string(), v.get(i).as_string()) {
            if HistoryKey::parse(&key).is_some() {
                out.insert(key, val);
            }
        }
    }
    out
}

/// 非同期に IndexedDB を開き、全件を手元の写しへ読んでから `on_ready` を呼ぶ（準備完了の合図）。
/// 開けないときは IndexedDB 無しの保存先（localStorage だけ）で呼ぶ。`on_ready` は必ず 1 回だけ呼ばれる。
pub fn open_history_storage(on_ready: impl FnOnce(OpenedStorage) + 'static) {
    let ready: Ready = Rc::new(RefCell::new(Some(Box::new(on_ready))));
    let factory = web_sys::window().and_then(|w| w.indexed_db().ok().flatten());
    let Some(factory) = factory else {
        finish(&ready, None);
        return;
    };
    let Ok(req) = factory.open_with_u32(DB_NAME, DB_VERSION) else {
        finish(&ready, None);
        return;
    };

    let req_up = req.clone();
    req.set_onupgradeneeded(Some(as_fn(&handler(move || {
        if let Ok(db) = req_up.result().and_then(|r| r.dyn_into::<IdbDatabase>()) {
            // 版 1 の作成時だけ呼ばれる。失敗は後続の transaction の失敗として現れる。
            let _ = db.create_object_store(STORE);
        }
    }))));

    let r = Rc::clone(&ready);
    req.set_onerror(Some(as_fn(&handler(move || finish(&r, None)))));
    let r = Rc::clone(&ready);
    req.set_onblocked(Some(as_fn(&handler(move || finish(&r, None)))));

    let r = Rc::clone(&ready);
    let req_ok = req.clone();
    req.set_onsuccess(Some(as_fn(&handler(move || {
        let Ok(db) = req_ok.result().and_then(|v| v.dyn_into::<IdbDatabase>()) else {
            finish(&r, None);
            return;
        };
        let reads = db
            .transaction_with_str(STORE)
            .and_then(|tx| tx.object_store(STORE).map(|s| (tx, s)))
            .and_then(|(tx, s)| Ok((tx, s.get_all_keys()?, s.get_all()?)));
        let Ok((tx, keys, values)) = reads else {
            finish(&r, None);
            return;
        };
        let (r_done, r_fail) = (Rc::clone(&r), Rc::clone(&r));
        let db_done = db.clone();
        tx.set_oncomplete(Some(as_fn(&handler(move || {
            let initial = collect(&keys, &values);
            finish(&r_done, Some((IdbStore { db: db_done }, initial)));
        }))));
        tx.set_onerror(Some(as_fn(&handler(move || finish(&r_fail, None)))));
    }))));
}
