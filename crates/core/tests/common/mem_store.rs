//! 試験用のメモリ上の `HistoryStore`。書き込み・削除の呼び出し回数を数え、拒否が保存領域に触れないことを照合する。

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;

use fandhe_workshop_contract::storage::{HistoryKey, HistoryStore, StorageError, StorageKind};

#[derive(Default)]
pub struct MemStore {
    map: RefCell<BTreeMap<String, String>>,
    writes: Cell<usize>,
    removes: Cell<usize>,
}

impl MemStore {
    pub fn reset_counts(&self) {
        self.writes.set(0);
        self.removes.set(0);
    }
    pub fn writes(&self) -> usize {
        self.writes.get()
    }
    pub fn removes(&self) -> usize {
        self.removes.get()
    }
    pub fn snapshot(&self) -> BTreeMap<String, String> {
        self.map.borrow().clone()
    }
}

impl HistoryStore for MemStore {
    fn kind(&self) -> StorageKind {
        StorageKind::LocalStorage
    }
    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        self.writes.set(self.writes.get() + 1);
        self.map.borrow_mut().insert(key.render(), value.to_owned());
        Ok(())
    }
    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        Ok(self.map.borrow().get(&key.render()).cloned())
    }
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        self.removes.set(self.removes.get() + 1);
        self.map.borrow_mut().remove(&key.render());
        Ok(())
    }
    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        Ok(self
            .map
            .borrow()
            .keys()
            .filter_map(|k| HistoryKey::parse(k))
            .collect())
    }
}
