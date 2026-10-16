//! localStorage の backend（TASK-317 1/2）。`storage_select::HistoryStorage` が [`SyncKv`] として使う薄い包み。
//!
//! 選択の規則・閾値は `storage_select` が持ち、ここはブラウザの API の呼び出しと失敗の種類への写像だけを行う。
//! 取得できない（プライベートモード・遮断）ときは [`StorageError::Unavailable`] を返す。
//! 例外は `name()` だけで種類を判定し、例外の文（message）は読まない・記録しない（履歴は受講者のデータのため）。

use crate::storage_select::{utf16_len, SyncKv};
use fandhe_workshop_contract::storage::StorageError;
use wasm_bindgen::JsCast;
use web_sys::{DomException, Storage};

/// `window.localStorage` の包み。
pub struct LocalStore {
    storage: Option<Storage>,
}

impl LocalStore {
    /// 取得する。取得できなくても作れ、各操作が [`StorageError::Unavailable`] を返す。
    pub fn new() -> Self {
        let storage = web_sys::window().and_then(|w| w.local_storage().ok().flatten());
        LocalStore { storage }
    }

    fn storage(&self) -> Result<&Storage, StorageError> {
        self.storage.as_ref().ok_or(StorageError::Unavailable)
    }
}

impl Default for LocalStore {
    fn default() -> Self {
        Self::new()
    }
}

/// 例外の `name` だけで上限超過かを判定する。Firefox の旧名も含める。
fn map_error(e: &wasm_bindgen::JsValue) -> StorageError {
    match e.dyn_ref::<DomException>().map(DomException::name) {
        Some(n) if n == "QuotaExceededError" || n == "NS_ERROR_DOM_QUOTA_REACHED" => {
            StorageError::QuotaExceeded
        }
        _ => StorageError::Failed,
    }
}

impl SyncKv for LocalStore {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.storage()?.get_item(key).map_err(|e| map_error(&e))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.storage()?
            .set_item(key, value)
            .map_err(|e| map_error(&e))
    }

    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.storage()?.remove_item(key).map_err(|e| map_error(&e))
    }

    fn keys(&self) -> Result<Vec<String>, StorageError> {
        let s = self.storage()?;
        let n = s.length().map_err(|e| map_error(&e))?;
        let mut out = Vec::new();
        for i in 0..n {
            if let Some(k) = s.key(i).map_err(|e| map_error(&e))? {
                out.push(k);
            }
        }
        Ok(out)
    }

    fn total_utf16(&self) -> Result<u64, StorageError> {
        let s = self.storage()?;
        let mut total = 0u64;
        for k in self.keys()? {
            total += utf16_len(&k);
            if let Some(v) = s.get_item(&k).map_err(|e| map_error(&e))? {
                total += utf16_len(&v);
            }
        }
        Ok(total)
    }
}
