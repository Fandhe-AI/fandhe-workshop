//! 履歴の保存先の選択の規則（TASK-317 1/2。D-52・SAVE-8）。
//!
//! 履歴は localStorage を既定とし、上限に近づいたら IndexedDB に切り替える（D-52）。この module は、
//! 2 つの保存先を contract の口 [`HistoryStore`] に載せる合成の保存先 [`HistoryStorage`] と、切り替えの閾値を置く。
//! ブラウザの API は直接呼ばず、小さな backend の trait（[`SyncKv`]・[`IdbBackend`]）を介す。
//! wasm 側の実体は `storage_local`・`storage_idb`、native の試験は手元の写しの偽物を渡す。
//! 呼び出し元は、離れる直前の書き込み（TASK-317 2/2。`save_pending`・`leave_flush`）・変更の後の遅延保存と測定（`save_schedule`・`persist`。TASK-318）・再読み込み（TASK-319）・復元（TASK-331）である。
//!
//! 仕様が決めていない点を、次のとおり決めた（推論）。SAVE-8 を確定にするかは spec 側で扱う。
//!
//! - 上限の単位: localStorage の上限は UTF-16 の符号単位数（PoC-6 の Chrome での実測は [`LOCAL_STORAGE_MAX_UTF16`]）で、
//!   履歴の上限（バイト）とは単位が違う。数えるのは鍵と値の両方で、同じオリジンのほかの鍵も含める
//! - 切り替えの閾値: 上限の 80%（[`SWITCH_THRESHOLD_UTF16`]）。補助の鍵・ブラウザ差・JSON の増分の余白である。
//!   2 周分の履歴は最大 471,568 バイト（F-5）で、閾値に対して十分な余裕がある
//! - 切り替えは一方向で、そのセッションの中で localStorage へ戻らない
//! - IndexedDB への書き込みの確定の後にだけ、同じ鍵の localStorage の写しを消す（確定前・失敗時は唯一の写しを失わない）
//! - 起動時に同じ鍵が両方にあれば、IndexedDB を正とする
//! - IndexedDB の書き込みの失敗は `last_async_error` に種類だけ残り、後の put の確定で消える。ホストが次の `update` で読んで
//!   表示する（SAVE-11・TASK-323 2/2）。IndexedDB を開けたが全件を読めなかったときは [`HistoryStorage::unreadable`] で
//!   `read`・`write` を断り、旧い値で巻き戻さない（自動運転での判断。人の承認は無い）
//! - IndexedDB が使えない（開けない）ときは localStorage だけで動く。localStorage の想定外の上限超過は
//!   [`StorageError::QuotaExceeded`] をそのまま返す（変更のたびの保存の失敗は `shell_notice` が表示する。TASK-323 1/2）
//!
//! 離れる直前の書き込み: 保存先が localStorage の間は `write` が同期で終わるため `pagehide` の中で足りる。IndexedDB へ切り替えた後は
//! `put` の発行までで確定を保証しない（SAVE-9 の検討中の論点で、仕様の決定を待つ）。
//!
//! 限界: [`LOCAL_STORAGE_MAX_UTF16`] は Chrome での実測で、Firefox・Safari は未確認である。

use fandhe_workshop_contract::storage::{
    DeliveryScope, HistoryKey, HistoryStore, StorageError, StorageKind, DEFAULT_STORAGE_KIND,
};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

/// localStorage に書ける最大（UTF-16 の符号単位数。PoC-6 の Chrome での実測）。オリジン単位で、鍵と値の両方を数える。
pub const LOCAL_STORAGE_MAX_UTF16: u64 = 5_242_877;

/// IndexedDB へ切り替える閾値（UTF-16 の符号単位数）。上限の 80%。この値を超える見込みの書き込みから切り替える。
pub const SWITCH_THRESHOLD_UTF16: u64 = LOCAL_STORAGE_MAX_UTF16 * 4 / 5;

/// UTF-16 の符号単位数。localStorage の上限の単位で、バイト数（`len`）でも文字数（`chars().count()`）でもない。
pub fn utf16_len(s: &str) -> u64 {
    u64::try_from(s.encode_utf16().count()).unwrap_or(u64::MAX)
}

/// 書き込み後の使用量の見込みが、切り替えの閾値を超えるか（ちょうどは切り替えない）。
pub fn should_switch(projected: u64) -> bool {
    projected > SWITCH_THRESHOLD_UTF16
}

/// 起動時の保存先の選択。IndexedDB にあればそちらを正とし（両方にあるときを含む）、それ以外は既定。
pub fn initial_kind(in_idb: bool) -> StorageKind {
    if in_idb {
        StorageKind::IndexedDb
    } else {
        DEFAULT_STORAGE_KIND
    }
}

/// 同期の key-value（localStorage）の backend。
pub trait SyncKv {
    /// 値を読む。
    fn get(&self, key: &str) -> Result<Option<String>, StorageError>;
    /// 値を書く。上限を超えれば [`StorageError::QuotaExceeded`]。
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError>;
    /// 値を消す。鍵が無くても `Ok`。
    fn remove(&self, key: &str) -> Result<(), StorageError>;
    /// 同じオリジンの全鍵（履歴の鍵以外を含む）。
    fn keys(&self) -> Result<Vec<String>, StorageError>;
    /// 全鍵の `鍵の長さ + 値の長さ`（UTF-16 の符号単位数）の合計。
    fn total_utf16(&self) -> Result<u64, StorageError>;
}

/// 非同期の完了の通知。確定・失敗は後から 1 回だけ届く。
pub type Done = Box<dyn FnOnce(Result<(), StorageError>)>;

/// 値の取得の完了の通知。無い鍵は `Ok(None)`。1 回だけ届く。
pub type Fetched = Box<dyn FnOnce(Result<Option<String>, StorageError>)>;

/// 完了を通知で返す key-value（IndexedDB）の backend。手元の写しへの反映は呼び出し側が同期で行う。
pub trait IdbBackend {
    /// 値を DB から取り直す。確定済みの書き込みだけが見え、手元の写しは使わない。完了で `done` が呼ばれる。
    fn get(&self, key: &str, done: Fetched);
    /// 値を書く。トランザクションの完了で `done` が呼ばれる。
    fn put(&self, key: &str, value: &str, done: Done);
    /// 値を消す。トランザクションの完了で `done` が呼ばれる。
    fn delete(&self, key: &str, done: Done);
}

impl<T: SyncKv + ?Sized> SyncKv for Rc<T> {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        (**self).get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        (**self).set(key, value)
    }
    fn remove(&self, key: &str) -> Result<(), StorageError> {
        (**self).remove(key)
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        (**self).keys()
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        (**self).total_utf16()
    }
}

impl<T: IdbBackend + ?Sized> IdbBackend for Rc<T> {
    fn get(&self, key: &str, done: Fetched) {
        (**self).get(key, done)
    }
    fn put(&self, key: &str, value: &str, done: Done) {
        (**self).put(key, value, done)
    }
    fn delete(&self, key: &str, done: Done) {
        (**self).delete(key, done)
    }
}

/// 非同期の完了の通知から触る状態。通知は `'static` の閉じた関数なので、`Rc` で共有する。
struct Shared<L> {
    local: L,
    /// localStorage 全体の使用量の見積もり。初回の書き込みで 1 回だけ全走査し、以後は差分で更新する。
    usage: Cell<Option<u64>>,
    /// 最後の非同期の失敗。種類だけを持ち、値・鍵・例外の文は持たない。
    last_async_error: Cell<Option<StorageError>>,
}

impl<L: SyncKv> Shared<L> {
    fn usage(&self) -> u64 {
        match self.usage.get() {
            Some(n) => n,
            None => {
                // 全走査に失敗したら cache せず、悲観側（閾値超過扱い）に倒す。
                match self.local.total_utf16() {
                    Ok(n) => {
                        self.usage.set(Some(n));
                        n
                    }
                    Err(_) => u64::MAX,
                }
            }
        }
    }

    fn rescan(&self) {
        self.usage.set(self.local.total_utf16().ok());
    }

    fn release(&self, key: &str, old_value: &str) {
        let freed = utf16_len(key) + utf16_len(old_value);
        // 見積もりが未確定なら更新しない（次の書き込みで全走査し直す）。
        if let Some(n) = self.usage.get() {
            self.usage.set(Some(n.saturating_sub(freed)));
        }
    }

    /// IndexedDB への書き込みの確定の後に、同じ鍵の localStorage の写しを消す。
    fn drop_local_copy(&self, key: &str) {
        if let Ok(Some(old)) = self.local.get(key) {
            if self.local.remove(key).is_ok() {
                self.release(key, &old);
            }
        }
    }
}

/// 合成の保存先。localStorage を既定とし、閾値を超える見込みの書き込みから IndexedDB へ一方向に切り替える。
pub struct HistoryStorage<L, I> {
    shared: Rc<Shared<L>>,
    idb: Option<I>,
    /// IndexedDB の手元の写し。読みは同期で、書き込みの確定は後から届く（口が同期のため）。
    mirror: Rc<RefCell<BTreeMap<String, String>>>,
    active: Cell<StorageKind>,
    /// IndexedDB を開けたが全件を読めなかった印（SAVE-11）。中身が分からないまま書くと、再読み込みで両方が読めたときに
    /// D-52（IndexedDB を正とする）で旧い値が勝って履歴が巻き戻るため、`read`・`write` を `Unavailable` で断る（fail-closed）。
    idb_unreadable: bool,
}

impl<L: SyncKv + 'static, I: IdbBackend> HistoryStorage<L, I> {
    /// 作る。`idb` が `None` なら IndexedDB は使えず、localStorage だけで動く。
    /// `idb_initial` は開いた時点の IndexedDB の全件（[`HistoryKey::parse`] できる鍵だけを渡す）。
    pub fn new(local: L, idb: Option<I>, idb_initial: BTreeMap<String, String>) -> Self {
        let mirror = if idb.is_some() {
            idb_initial
        } else {
            BTreeMap::new()
        };
        HistoryStorage {
            shared: Rc::new(Shared {
                local,
                usage: Cell::new(None),
                last_async_error: Cell::new(None),
            }),
            idb,
            mirror: Rc::new(RefCell::new(mirror)),
            active: Cell::new(DEFAULT_STORAGE_KIND),
            idb_unreadable: false,
        }
    }

    /// IndexedDB を開けたが全件を読めなかった保存先（SAVE-11）。`read`・`write` は [`StorageError::Unavailable`] を返し、
    /// どちらの保存先の既存の値も変えない。呼び出し元（`storage_idb`）は接続を閉じてから呼ぶ。
    pub fn unreadable(local: L) -> Self {
        let mut s = Self::new(local, None, BTreeMap::new());
        s.idb_unreadable = true;
        s
    }

    /// IndexedDB の中身を読めなかった保存先か（[`HistoryStorage::unreadable`]）。
    pub fn idb_unreadable(&self) -> bool {
        self.idb_unreadable
    }

    /// IndexedDB を使えるか（開けたか）。
    pub fn idb_available(&self) -> bool {
        self.idb.is_some()
    }

    /// 直近の IndexedDB への書き込み（put）の失敗の種類。値・鍵・例外の文は持たない。
    ///
    /// put の確定で `None` に戻る（後の保存の成功で、表示が消えるようにするため。SAVE-11）。呼び出し元の `persist` が
    /// ホストの表示の元として読み、`shell_notice` が文にする。
    pub fn last_async_error(&self) -> Option<StorageError> {
        self.shared.last_async_error.get()
    }

    /// 起動時の鍵ごとの選択。IndexedDB の側にあればこのセッションの保存先を IndexedDB に固定する。
    pub fn kind_for(&self, key: &HistoryKey) -> StorageKind {
        let k = key.render();
        let in_idb = self.idb.is_some() && self.mirror.borrow().contains_key(&k);
        let kind = initial_kind(in_idb);
        if kind == StorageKind::IndexedDb {
            self.active.set(StorageKind::IndexedDb);
        }
        kind
    }

    /// 保存先の実データから手元の写しを取り直す（「この入力を残さない」の読み直し用。SAFE-28）。
    ///
    /// IndexedDB を使えるときは DB から鍵の値を取得し、完了してから写しを DB の内容に合わせて `done(Ok)` を呼ぶ。
    /// 別のタブの保存は取り込まれ、自タブの put が失敗していれば未保存の値は写しから消える。取得に失敗したときは
    /// 写しを変えずに `done(Err)` を呼ぶ（呼び出し元は古い写しで復元してはならない）。IndexedDB を使えない・localStorage の
    /// 読みは常に実データなので、取り直さずに `done(Ok)` を即座に呼ぶ。
    pub fn refetch(&self, key: &HistoryKey, done: Box<dyn FnOnce(Result<(), StorageError>)>) {
        if self.idb_unreadable {
            done(Err(StorageError::Unavailable));
            return;
        }
        let Some(idb) = &self.idb else {
            done(Ok(()));
            return;
        };
        let k = key.render();
        let mirror = Rc::clone(&self.mirror);
        let kk = k.clone();
        idb.get(
            &k,
            Box::new(move |r| match r {
                Ok(v) => {
                    match v {
                        Some(v) => mirror.borrow_mut().insert(kk, v),
                        None => mirror.borrow_mut().remove(&kk),
                    };
                    done(Ok(()));
                }
                Err(e) => done(Err(e)),
            }),
        );
    }

    /// 指定した配信先・教材の履歴の鍵をすべて消す（SAVE-32。保存済みの履歴の削除の中核）。消した件数を返す。
    ///
    /// 対象は [`HistoryKey::belongs_to`] に当たる鍵だけで、別の教材・別の配信先・`unreadable` の鍵・履歴でない鍵（起動番号など）は
    /// 消さない。IndexedDB を全件読めなかった保存先は中身が分からないため、何も消さずに `Unavailable` を返す（fail-closed）。
    /// 失敗した鍵があっても残りの鍵は試し、最後に `keys` を引き直して対象が残っていれば `Failed` を返す（一部だけ消えうる）。
    /// IndexedDB の delete は確定を待たず、後からの失敗は `last_async_error` に残る（SAVE-11 の経路で出る）。
    ///
    /// # Errors
    ///
    /// 読めない保存先・鍵の一覧を引けない・対象が残った場合。
    pub fn remove_pack(&self, scope: &DeliveryScope, pack_id: &str) -> Result<usize, StorageError> {
        if self.idb_unreadable {
            return Err(StorageError::Unavailable);
        }
        let targets: Vec<HistoryKey> = self
            .keys()?
            .into_iter()
            .filter(|k| k.belongs_to(scope, pack_id))
            .collect();
        let mut first_error = None;
        for key in &targets {
            if let Err(e) = self.remove(key) {
                first_error.get_or_insert(e);
            }
        }
        let remaining = self.keys()?.iter().any(|k| k.belongs_to(scope, pack_id));
        if remaining {
            return Err(first_error.unwrap_or(StorageError::Failed));
        }
        Ok(targets.len())
    }

    /// IndexedDB へ書く。put の確定を待たずに手元の写しを更新して `Ok` を返す（口の [`HistoryStore::write`] が同期のため）。
    ///
    /// put が後から失敗したときの状態（設計上の限界）: 手元の写しは未保存の値を持ち、このセッションの `read` は
    /// その値（メモリ上の最新の状態と同じ）を返す。localStorage の写しは消さずに残す（既存の保存を壊さない）。
    /// 失敗の種類は `last_async_error` に残る。再読み込みすると、IndexedDB・localStorage に残った旧い値が復元される。
    /// 失敗の利用者への表示は、ホストが `last_async_error` を次の `update` で読んで行う（SAVE-11。TASK-323 2/2）。
    /// 写しと保存先のずれは直さない（次の書き込みが同じ鍵の写しを上書きし、その確定で表示が消える）。
    fn write_idb(&self, idb: &I, k: &str, value: &str) -> Result<(), StorageError> {
        self.active.set(StorageKind::IndexedDb);
        // 順序: 手元の写しを更新 → put を発行 → 確定の通知で localStorage の写しを消す。
        self.mirror
            .borrow_mut()
            .insert(k.to_string(), value.to_string());
        let shared = Rc::clone(&self.shared);
        let key = k.to_string();
        idb.put(
            k,
            value,
            Box::new(move |r| match r {
                Ok(()) => {
                    shared.drop_local_copy(&key);
                    // 後の保存の成功で、前の失敗の表示が残り続けないようにする（SAVE-11）。
                    shared.last_async_error.set(None);
                }
                Err(e) => shared.last_async_error.set(Some(e)),
            }),
        );
        Ok(())
    }
}

impl<L: SyncKv + 'static, I: IdbBackend> HistoryStore for HistoryStorage<L, I> {
    /// このセッションの保存先。状態は変えない。
    ///
    /// IndexedDB にある鍵があっても、その鍵で [`HistoryStorage::kind_for`] か `write` を呼ぶまでは
    /// [`StorageKind::LocalStorage`] を返す（切り替えはこの 2 つだけが行う）。
    fn kind(&self) -> StorageKind {
        self.active.get()
    }

    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        if self.idb_unreadable {
            return Err(StorageError::Unavailable);
        }
        let k = key.render();
        if let Some(idb) = &self.idb {
            // 写しに鍵があれば、kind_for が未呼び出しでも IndexedDB へ書く（read が写しを先に引くため）。
            if self.active.get() == StorageKind::IndexedDb || self.mirror.borrow().contains_key(&k)
            {
                return self.write_idb(idb, &k, value);
            }
        }
        let old_len = self
            .shared
            .local
            .get(&k)
            .ok()
            .flatten()
            .map(|v| utf16_len(&v));
        let key_part = if old_len.is_none() { utf16_len(&k) } else { 0 };
        let projected = self
            .shared
            .usage()
            .saturating_sub(old_len.unwrap_or(0))
            .saturating_add(utf16_len(value))
            .saturating_add(key_part);
        if let Some(idb) = &self.idb {
            if should_switch(projected) {
                return self.write_idb(idb, &k, value);
            }
        }
        match self.shared.local.set(&k, value) {
            Ok(()) => {
                // 見積もりが未確定（全走査に失敗）のときは、悲観値を cache しない。
                if self.shared.usage.get().is_some() {
                    self.shared.usage.set(Some(projected));
                }
                Ok(())
            }
            Err(StorageError::QuotaExceeded) => {
                // 見積もりが外れた。全走査し直したうえで、IndexedDB が使えれば切り替える。
                self.shared.rescan();
                match &self.idb {
                    Some(idb) => self.write_idb(idb, &k, value),
                    None => Err(StorageError::QuotaExceeded),
                }
            }
            Err(e) => Err(e),
        }
    }

    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        if self.idb_unreadable {
            return Err(StorageError::Unavailable);
        }
        let k = key.render();
        if let Some(v) = self.mirror.borrow().get(&k) {
            return Ok(Some(v.clone()));
        }
        self.shared.local.get(&k)
    }

    /// 鍵を両方の保存先から消す。IndexedDB の delete は確定を待たずに発行する。
    ///
    /// 設計上の限界: 手元の写しの削除と IndexedDB の delete を発行した後で localStorage の削除が失敗すると `Err` を返すが、
    /// IndexedDB 側は削除済み（または削除の途中）のまま元へ戻さない（`Err` は何も消えていないことを意味しない）。
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        let k = key.render();
        if let Some(idb) = &self.idb {
            // write_idb と同じく、delete の確定を待たずに手元の写しから消す。失敗は種類だけを last_async_error に残す。
            self.mirror.borrow_mut().remove(&k);
            let shared = Rc::clone(&self.shared);
            idb.delete(
                &k,
                Box::new(move |r| {
                    if let Err(e) = r {
                        shared.last_async_error.set(Some(e));
                    }
                }),
            );
        }
        let old = self.shared.local.get(&k).ok().flatten();
        self.shared.local.remove(&k)?;
        if let Some(old) = old {
            self.shared.release(&k, &old);
        }
        Ok(())
    }

    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        let mut all: BTreeSet<String> = self.mirror.borrow().keys().cloned().collect();
        match self.shared.local.keys() {
            Ok(ks) => all.extend(ks),
            Err(e) if self.idb.is_none() => return Err(e),
            Err(_) => {}
        }
        Ok(all.iter().filter_map(|k| HistoryKey::parse(k)).collect())
    }
}
