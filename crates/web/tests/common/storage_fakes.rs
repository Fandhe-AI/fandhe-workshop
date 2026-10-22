//! `HistoryStorage` の試験が共有する backend の偽物（TASK-317 2/2）。
//!
//! `save_pending`・`storage_store_contract` の試験が `#[path]` で取り込む。`storage_select` の試験は 1/2 の
//! 時点の自前の偽物を持ち続ける（試験名を動かさないため）。IndexedDB の偽物は確定を保留し、試験が手で起こす。

#![allow(
    dead_code,
    reason = "試験の crate ごとに使う項目が異なり、使わない側で未使用の警告が出るため"
)]

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, StorageError};
use fandhe_workshop_web::storage_select::{
    utf16_len, Done, Fetched, HistoryStorage, IdbBackend, SyncKv,
};
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

#[derive(Default)]
pub struct FakeLocal {
    pub map: RefCell<BTreeMap<String, String>>,
    /// 全体の上限（鍵と値の UTF-16 の合計）。`None` なら無制限。
    pub limit: Cell<Option<u64>>,
}

impl FakeLocal {
    pub fn total(&self) -> u64 {
        self.map
            .borrow()
            .iter()
            .map(|(k, v)| utf16_len(k) + utf16_len(v))
            .sum()
    }
    pub fn raw(&self, k: &str) -> Option<String> {
        self.map.borrow().get(k).cloned()
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
        self.map
            .borrow_mut()
            .insert(key.to_string(), value.to_string());
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
        Ok(self.total())
    }
}

/// 保留中の操作。書き込みなら鍵と値、削除なら `None`。
type Pending = (Option<(String, String)>, Done);

#[derive(Default)]
pub struct FakeIdb {
    pub store: RefCell<BTreeMap<String, String>>,
    pending: RefCell<Vec<Pending>>,
    /// 次の `get` を失敗させる種類（1 回で消える）。
    pub get_error: Cell<Option<StorageError>>,
}

impl FakeIdb {
    /// 保留中の通知を、確定（`Ok`）か失敗で起こす。確定したときだけ保存先に入る。
    pub fn settle(&self, result: Result<(), StorageError>) {
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
    pub fn pending_len(&self) -> usize {
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

pub type Storage = HistoryStorage<Rc<FakeLocal>, Rc<FakeIdb>>;

pub struct Fixture {
    pub local: Rc<FakeLocal>,
    pub idb: Rc<FakeIdb>,
    pub storage: Storage,
}

/// 鍵（区間 `repo`・開始点 `s-0-1`）。
pub fn key(pack: &str) -> HistoryKey {
    HistoryKey::for_pack(DeliveryScope::named("repo"), pack, "s-0-1")
}

pub fn fixture(idb_available: bool) -> Fixture {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let storage = HistoryStorage::new(
        Rc::clone(&local),
        idb_available.then(|| Rc::clone(&idb)),
        BTreeMap::new(),
    );
    Fixture {
        local,
        idb,
        storage,
    }
}

/// 枠の通知欄（`shell-notice`）の `save-failed` 項目の（理由コード, 文）。項目が無ければ `None`（TASK-534 1/2）。
/// 項目は 0 件か 1 件で、2 件以上なら panic する（同じ失敗の二重表示を見逃さない）。
pub fn save_failed_item(h: &fandhe_workshop_web::host::EngineHost) -> Option<(String, String)> {
    use fandhe_frontend_core::Node;
    use fandhe_frontend_interactive::Component;
    fn attr<'a>(n: &'a Node, k: &str) -> Option<&'a str> {
        match n {
            Node::Element { attrs, .. } => attrs
                .iter()
                .find(|(name, _)| name == k)
                .map(|(_, v)| v.as_str()),
            _ => None,
        }
    }
    fn text_of(n: &Node) -> String {
        match n {
            Node::Text(t) => t.clone(),
            Node::Element { children, .. } => children.iter().map(text_of).collect(),
            _ => String::new(),
        }
    }
    fn find_text(n: &Node, out: &mut Vec<String>) {
        if attr(n, "data-testid") == Some("shell-notice-text") {
            out.push(text_of(n));
        }
        if let Node::Element { children, .. } = n {
            for c in children {
                find_text(c, out);
            }
        }
    }
    fn walk(n: &Node, out: &mut Vec<(String, String)>) {
        if attr(n, "data-testid") == Some("shell-notice-item")
            && attr(n, "data-notice-kind") == Some("save-failed")
        {
            let reason = attr(n, "data-notice-reason").unwrap_or("").to_owned();
            let mut texts = Vec::new();
            find_text(n, &mut texts);
            out.push((reason, texts.concat()));
        }
        if let Node::Element { children, .. } = n {
            for c in children {
                walk(c, out);
            }
        }
    }
    let mut found = Vec::new();
    walk(&h.view(), &mut found);
    assert!(found.len() <= 1, "save-failed の項目が複数: {found:?}");
    found.pop()
}
