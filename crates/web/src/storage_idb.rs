//! IndexedDB の backend と、非同期に開く入口（TASK-317 1/2）。
//!
//! `storage_select::HistoryStorage` が [`IdbBackend`] として使う薄い包み。DB 名 `fandhe-workshop`・版 1・
//! object store `history`（鍵は `HistoryKey::render()` の文字列）。選択の規則は `storage_select` が持つ。
//! [`open_history_storage`] は、開いて全件を手元の写しへ読み込んだ後に `on_ready` を呼ぶ。これが準備完了の合図で、
//! 復元の経路（TASK-319・TASK-331）と離れる直前の書き込み（TASK-317 2/2。`save_pending`・`leave_flush`）はこの合図の後に使う。
//! 開けない（`indexedDB` が無い・エラー・他のタブに塞がれた）ときは、IndexedDB 無しで `on_ready` を呼ぶ（localStorage だけで動く）。
//!
//! 完了・失敗の通知は JS のイベントで届く。ハンドラは `Closure::new` で作って `Handlers` に持ち、確定のときに
//! 要求・トランザクションから外して Rust 側のクロージャを解放する（`release`）。`Closure::once_into_js` は呼ばれない限り
//! 解放されず、書き込みごとに呼ばれなかった側が残るため使わない。確定しないまま残るのは、他のタブに塞がれたまま
//! 開けない要求のハンドラ（1 セッションに 1 組・定数個）と、保持中の接続の `versionchange` ハンドラ 1 つ（[`IdbStore`] の破棄で外す）だけである。
//! 例外の文は読まない・記録しない。DOM の値は読まない（`DomException::name` と要求の結果だけを使う）。

use crate::storage_local::LocalStore;
use crate::storage_select::{Done, Fetched, HistoryStorage, IdbBackend, Listed};
use fandhe_workshop_contract::storage::{HistoryKey, StorageError};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::js_sys::{Array, Function};
use web_sys::{IdbDatabase, IdbOpenDbRequest, IdbRequest, IdbTransaction, IdbTransactionMode};

const DB_NAME: &str = "fandhe-workshop";
const DB_VERSION: u32 = 1;
const STORE: &str = "history";

/// 開いた IndexedDB の包み。接続はこの包みだけが持ち（`Clone` にしない）、破棄のときに閉じる（[`Drop`]）。
///
/// 別のタブが新しい版で開こうとすると、保持中の接続に `versionchange` が届く。接続を閉じないと相手の要求が
/// `blocked` のまま進まないため、ハンドラの中で同期に閉じる（同期に閉じるので相手には `blocked` も発火しない）。
/// 閉じた後は再度開かない。版を上げたのは新しいコードのタブで、このタブの `DB_VERSION` は DB の版より小さく、
/// 開き直しは `VersionError` になる上に新しい版のスキーマも知らないためである。閉じた後の `put`・`delete` は
/// トランザクションを作らず `StorageError::Unavailable`（このセッションでは IndexedDB へ書けない）で失敗する。
/// 失敗後の扱いは `HistoryStorage` の既存の規則のままで、手元の写しは値を持ち（このセッションの読み取りは書いた値を返す）、
/// localStorage の写しは消さず、`last_async_error` に種類が残る。再読み込みすると保存先に残った旧い値に戻る。
/// 読み取りは手元の写しと localStorage だけを引くため、閉じても影響を受けない。
pub struct IdbStore {
    db: IdbDatabase,
    /// `versionchange` で閉じた後に立つ。書き込みの入口が見る。
    closed: Rc<Cell<bool>>,
    /// `versionchange` のハンドラの持ち主。破棄のときに外してから解放する。
    on_version_change: Closure<dyn FnMut()>,
}

impl IdbStore {
    /// 開いた接続を包み、`versionchange` で閉じるハンドラを付ける。呼び出し元は `open_history_storage` だけ。
    fn new(db: IdbDatabase) -> IdbStore {
        let closed = Rc::new(Cell::new(false));
        let (c, d) = (Rc::clone(&closed), db.clone());
        let on_version_change = Closure::<dyn FnMut()>::new(move || {
            c.set(true);
            d.close();
        });
        db.set_onversionchange(Some(on_version_change.as_ref().unchecked_ref()));
        IdbStore {
            db,
            closed,
            on_version_change,
        }
    }
}

/// `IdbDatabase` は破棄では閉じないため、包みの破棄で明示的に閉じる。開き直し（試験用の口の `debug_storage_open` が
/// 前の保持を捨てる等）で旧い接続が残ると、別のタブの版上げが塞がれる。
/// `close()` は発行済みのトランザクションの完了を待ってから接続を閉じるため、確定前の put・delete は失われない。
/// ハンドラは、フィールドの `Closure` が解放される前（この `drop` の中）に外す（外さずに解放すると、後から届いた
/// イベントが解放済みのクロージャを呼ぶ）。
impl Drop for IdbStore {
    fn drop(&mut self) {
        self.db.set_onversionchange(None);
        self.db.close();
        let _ = &self.on_version_change;
    }
}

/// 1 回だけ呼ぶ通知を、複数のイベントから共有するための入れ物。
type OnceCell = Rc<RefCell<Option<Done>>>;

/// ハンドラの持ち主。確定のときに [`release`] で空にする。各ハンドラはこの `Rc` を捕まえるため循環するが、
/// 空にした時点で循環は切れる。
type Handlers = Rc<RefCell<Vec<Closure<dyn FnMut()>>>>;

fn fire(cell: &OnceCell, r: Result<(), StorageError>) {
    let taken = cell.borrow_mut().take();
    if let Some(done) = taken {
        done(r);
    }
}

/// `f` を 1 回だけ呼ぶハンドラを作り、`keep` に持たせて JS の関数を返す。2 回目以降の呼び出しは何もしない。
fn handler(keep: &Handlers, f: impl FnOnce() + 'static) -> Function {
    let mut f = Some(f);
    let c = Closure::<dyn FnMut()>::new(move || {
        if let Some(f) = f.take() {
            f();
        }
    });
    let js: Function = c.as_ref().unchecked_ref::<Function>().clone();
    keep.borrow_mut().push(c);
    js
}

/// 持っているハンドラを解放する。呼ぶ前に、要求・トランザクションのハンドラを `None` に外しておく
/// （外さずに解放すると、後から届いたイベントが解放済みのクロージャを呼んで JS の例外になる）。
/// 実行中のハンドラ自身を解放してもよい（wasm-bindgen は呼び出しが戻るまで解放を遅らせる）。
fn release(keep: &Handlers) {
    // 借用を返してから解放する（解放の途中で同じ入れ物を借りても衝突しない）。
    let taken = keep.take();
    drop(taken);
}

/// トランザクションの完了・中断を `done` へ流し、確定のときにハンドラを外して解放する。
///
/// 失敗は `abort` だけで確定する。要求の失敗（上限超過を含む）は、要求の `error` イベントがトランザクションの
/// `error` へバブリングして先に届き、既定の動作（`preventDefault` しない）でトランザクションが中断されて後から `abort` が届く。
/// 中断の理由（要求の例外）は `abort` の時点で `tx.error()` に入るため、`error` では確定せず、`abort` で `name` を見て
/// 上限超過（`QuotaExceededError`）かを見分ける。`error` のハンドラを足すと、上限超過を `Failed` に誤分類する。
fn watch(tx: &IdbTransaction, done: Done) {
    let cell: OnceCell = Rc::new(RefCell::new(Some(done)));
    let keep: Handlers = Rc::default();
    let settle = {
        let (tx, keep) = (tx.clone(), Rc::clone(&keep));
        move |r: Result<(), StorageError>| {
            tx.set_oncomplete(None);
            tx.set_onabort(None);
            release(&keep);
            fire(&cell, r);
        }
    };
    let settle = Rc::new(settle);
    let s = Rc::clone(&settle);
    tx.set_oncomplete(Some(&handler(&keep, move || s(Ok(())))));
    let tx2 = tx.clone();
    tx.set_onabort(Some(&handler(&keep, move || {
        let quota = tx2
            .error()
            .map(|e| e.name() == "QuotaExceededError")
            .unwrap_or(false);
        settle(Err(if quota {
            StorageError::QuotaExceeded
        } else {
            StorageError::Failed
        }))
    })));
}

impl IdbStore {
    fn start(&self, done: Done) -> Option<(IdbTransaction, Done)> {
        if self.closed.get() {
            // `versionchange` で閉じた後。再度は開かない（`IdbStore` の doc）。
            done(Err(StorageError::Unavailable));
            return None;
        }
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
    fn get(&self, key: &str, done: Fetched) {
        if self.closed.get() {
            done(Err(StorageError::Unavailable));
            return;
        }
        let Ok(tx) = self.db.transaction_with_str(STORE) else {
            done(Err(StorageError::Failed));
            return;
        };
        let Ok(req) = tx
            .object_store(STORE)
            .and_then(|s| s.get(&JsValue::from_str(key)))
        else {
            done(Err(StorageError::Failed));
            return;
        };
        // 完了の後に結果を読む。文字列以外（未定義）は鍵が無いこととして扱う。
        watch(
            &tx,
            Box::new(move |r| {
                done(r.map(|()| req.result().ok().and_then(|v| v.as_string())));
            }),
        );
    }

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

    fn keys(&self, done: Listed) {
        if self.closed.get() {
            done(Err(StorageError::Unavailable));
            return;
        }
        let Ok(tx) = self.db.transaction_with_str(STORE) else {
            done(Err(StorageError::Failed));
            return;
        };
        let Ok(req) = tx.object_store(STORE).and_then(|s| s.get_all_keys()) else {
            done(Err(StorageError::Failed));
            return;
        };
        watch(
            &tx,
            Box::new(move |r| {
                done(r.map(|()| {
                    req.result()
                        .ok()
                        .map(|v| {
                            let a: Array = v.unchecked_into();
                            (0..a.length())
                                .filter_map(|i| a.get(i).as_string())
                                .collect()
                        })
                        .unwrap_or_default()
                }));
            }),
        );
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

/// 開いた結果。`Unreadable` は開けたが全件を読めなかった場合で、中身が分からないため保存を止める保存先にする（SAVE-11）。
enum Opened {
    /// 開けなかった（localStorage だけで動く）。
    Unavailable,
    /// 開けて全件を読めた。
    Ready(IdbStore, BTreeMap<String, String>),
    /// 開けたが全件を読めなかった（接続は閉じ済み）。
    Unreadable,
}

/// 準備完了の合図を出す（2 回目以降は何もしない）。合図を出した後に開けた接続が届いたら、破棄して（[`Drop`] で閉じて）残さない。
fn finish(ready: &Ready, opened: Opened) {
    let taken = ready.borrow_mut().take();
    match (taken, opened) {
        (Some(f), Opened::Ready(store, initial)) => {
            f(HistoryStorage::new(LocalStore::new(), Some(store), initial))
        }
        (Some(f), Opened::Unavailable) => f(HistoryStorage::new(
            LocalStore::new(),
            None,
            BTreeMap::new(),
        )),
        (Some(f), Opened::Unreadable) => f(HistoryStorage::unreadable(LocalStore::new())),
        (None, Opened::Ready(store, _)) => drop(store),
        (None, _) => {}
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

/// 開く要求のハンドラを外して解放する。`success` か `error` の後にだけ呼ぶ（`blocked` の後は、遅れて
/// `success` が届いたときに接続を閉じるため残す）。
fn detach_open(req: &IdbOpenDbRequest, keep: &Handlers) {
    req.set_onupgradeneeded(None);
    req.set_onblocked(None);
    req.set_onerror(None);
    req.set_onsuccess(None);
    release(keep);
}

/// 非同期に IndexedDB を開き、全件を手元の写しへ読んでから `on_ready` を呼ぶ（準備完了の合図）。
/// 開けないときは IndexedDB 無しの保存先（localStorage だけ）で呼ぶ。開けたが全件を読めなかったときは、保存を止める保存先
/// （[`HistoryStorage::unreadable`]。`read`・`write` が `Unavailable`）で呼ぶ（SAVE-11。旧い値による巻き戻りを避ける）。`on_ready` は必ず 1 回だけ呼ばれる
/// （開く要求の `error`・`blocked`・`success` と、全件の読み取りの完了・中断のどの経路でも）。
/// 他のタブに塞がれて（`blocked`）合図を出した後に遅れて開けたとき、および全件を読めなかったときは、接続を閉じる。
/// 開けた後に別のタブが新しい版で開こうとしたときの扱いは [`IdbStore`] の doc。
pub fn open_history_storage(on_ready: impl FnOnce(OpenedStorage) + 'static) {
    let ready: Ready = Rc::new(RefCell::new(Some(Box::new(on_ready))));
    let factory = web_sys::window().and_then(|w| w.indexed_db().ok().flatten());
    let Some(factory) = factory else {
        finish(&ready, Opened::Unavailable);
        return;
    };
    let Ok(req) = factory.open_with_u32(DB_NAME, DB_VERSION) else {
        finish(&ready, Opened::Unavailable);
        return;
    };
    let keep: Handlers = Rc::default();

    let req_up = req.clone();
    req.set_onupgradeneeded(Some(&handler(&keep, move || {
        if let Ok(db) = req_up.result().and_then(|r| r.dyn_into::<IdbDatabase>()) {
            // 版 1 の作成時だけ呼ばれる。失敗は後続の transaction の失敗として現れる。
            let _ = db.create_object_store(STORE);
        }
    })));

    let (r, k, req_err) = (Rc::clone(&ready), Rc::clone(&keep), req.clone());
    req.set_onerror(Some(&handler(&keep, move || {
        detach_open(&req_err, &k);
        finish(&r, Opened::Unavailable);
    })));
    let r = Rc::clone(&ready);
    req.set_onblocked(Some(&handler(&keep, move || {
        finish(&r, Opened::Unavailable)
    })));

    let (r, k, req_ok) = (Rc::clone(&ready), Rc::clone(&keep), req.clone());
    req.set_onsuccess(Some(&handler(&keep, move || {
        detach_open(&req_ok, &k);
        let Ok(db) = req_ok.result().and_then(|v| v.dyn_into::<IdbDatabase>()) else {
            finish(&r, Opened::Unavailable);
            return;
        };
        if r.borrow().is_none() {
            // 塞がれて合図を出した後に遅れて開けた。読まずに閉じる。
            db.close();
            return;
        }
        // 全件の読み取りの最中に届く `versionchange` でも閉じるよう、読む前に包む。
        let store = IdbStore::new(db);
        let reads = store
            .db
            .transaction_with_str(STORE)
            .and_then(|tx| tx.object_store(STORE).map(|s| (tx, s)))
            .and_then(|(tx, s)| Ok((tx, s.get_all_keys()?, s.get_all()?)));
        let Ok((tx, keys, values)) = reads else {
            drop(store);
            finish(&r, Opened::Unreadable);
            return;
        };
        // 完了・中断のどちらでも合図を出す（watch は abort でも確定する）。
        watch(
            &tx,
            Box::new(move |res| match res {
                Ok(()) => {
                    let initial = collect(&keys, &values);
                    finish(&r, Opened::Ready(store, initial));
                }
                Err(_) => {
                    drop(store);
                    finish(&r, Opened::Unreadable);
                }
            }),
        );
    })));
}
