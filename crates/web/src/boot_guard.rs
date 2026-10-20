//! 保存の直前に起動番号を照合して、古いタブの保存を止める（TASK-324 1/2。SAVE-12・D-56）。
//!
//! 同じ教材を同じブラウザの 2 つのタブで開くと、どちらも同じ鍵（`HistoryKey`）へ書く。古いタブで操作すると
//! 新しいタブの保存を上書きするため、ページを開くたびに起動番号を補助の鍵（meta）へ書き、保存の直前に meta が
//! 自分の値と違えば書かない（PoC-6 S-8）。再読み込みで世代を上げる仕組みは各タブのメモリの中の話で、これだけでは防げない。
//! 呼び出し元は `persist`（起動で [`BootGuard::claim`]、書き出しの関所で [`Guarded`]）である。
//! 古いタブの画面に出す「保存されていない」表示と合流の規則は持たない（SHELL-23・D-56 の持ち分）。
//!
//! 仕様が決めていない点を、次のとおり決めた（推論。自動運転での判断で、人の承認は無い）。
//!
//! - 置き場所: 契約（`contract::storage`）は変えず、web が持つ。契約の版 1 に meta の鍵を含めない
//! - 常に localStorage に書く: 照合は保存の直前に同期で行う必要があり、タブ間で共有される同期の保存領域は localStorage だけである。
//!   履歴が IndexedDB へ切り替わった後も、meta は localStorage に置く
//! - 鍵は `HistoryKey` ごと: 全体で 1 つにすると、別の教材・別の章のタブ同士が互いを古いと見なす。
//!   形は `fandhe-workshop/meta/boot/<配信の区間>/<教材の ID>/<開始点の ID>`。`HistoryKey::parse` が拒む接頭辞なので、
//!   履歴の鍵の一覧（`keys`）にも SAVE-32 の削除の対象にも入らない
//! - 値は `<n>:<tag>`: `n` は前の値に 1 を足した 10 進、`tag` は 8 桁の小文字 16 進。localStorage の読み書きはタブ間で原子的でなく、
//!   ほぼ同時に起動した 2 つのタブが同じ `n` を書きうるため、`tag` で最後に書いた 1 つだけを一致させる。`tag` は一意化のためで、秘密ではない
//! - fail-closed: 取得の書き込みに失敗した・照合で読めない・meta が無い、はいずれも書かない。取得の失敗（容量超過など）は照合のたびに再取得を試みるが、meta が取得の直前の値のままのときに限る（変わっていれば後から起動したタブがあるので古いタブとして止める）。上書きを防ぐ側に倒し、保存済みの履歴は消さない。
//!   この 3 つは古いタブではなく保存の失敗なので、`write` は取得・照合の `StorageError` を `Err` で返す（SAVE-11 の通知経路へ届く）
//! - 古いタブ（meta が別の値）の `write` だけは `Ok` を返して捨てる: `Err` にすると古いタブに保存失敗の表示が出るが、その表示は SHELL-23 の持ち分である
//! - 書いた直後にもう一度照合する: 照合と書き込みの間の隙間で他のタブが起動していれば、以後の書き込みを止める
//!
//! 限界:
//!
//! - 照合と書き込みは、タブ間で原子的ではない。localStorage には排他の仕組みが無く、`navigator.locks` は非同期で同期の口に使えない。
//!   照合の後から書き込みの完了までの間に別のタブが起動して書くと、その 1 回だけは上書きしうる。書いた直後の再照合で、2 回目以降を止める
//! - localStorage が使えず IndexedDB だけが使える環境では、起動番号を確かめられないため保存が止まる
//! - SAVE-32 の教材ごとの削除は meta の鍵を消さない（値は小さい）
//! - meta の鍵は `HistoryStorage` の使用量の見積もり（localStorage の全鍵の合計）にわずかに含まれる
//!
//! 値・鍵の文字列は、結果にも記録にも出さない。meta は同じオリジンの誰でも書ける未信頼の値として厳格に解析する。

use std::cell::{Cell, RefCell};

use fandhe_workshop_contract::storage::{
    HistoryKey, HistoryStore, StorageError, StorageKind, HISTORY_KEY_PREFIX,
};

use crate::storage_select::SyncKv;

/// 起動番号の鍵の接頭辞。`HistoryKey::parse` が受ける `fandhe-workshop/history/` とは重ならない。
pub const BOOT_META_PREFIX: &str = "fandhe-workshop/meta/boot";

/// `key` の起動番号の鍵。`HistoryKey::render` の区間（符号化済みで単射）をそのまま使う。
pub fn boot_meta_key(key: &HistoryKey) -> String {
    let rendered = key.render();
    let rest = rendered
        .strip_prefix(HISTORY_KEY_PREFIX)
        .and_then(|r| r.strip_prefix('/'))
        .unwrap_or(&rendered);
    format!("{BOOT_META_PREFIX}/{rest}")
}

/// 値 `<n>:<tag>` を組み立てる。`tag` は 8 桁の小文字 16 進。
pub fn format_boot(n: u64, tag: u32) -> String {
    format!("{n}:{tag:08x}")
}

/// 値を厳格に解析して `n` を返す。数字だけの `n`・8 桁の小文字 16 進の `tag` 以外（空・符号・`:` 無し・余分な欄）は `None`。
pub fn parse_boot(s: &str) -> Option<u64> {
    let (n, tag) = s.split_once(':')?;
    let n_ok = !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit());
    let tag_ok = tag.len() == 8 && tag.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    if n_ok && tag_ok {
        n.parse().ok()
    } else {
        None
    }
}

/// 取得の元になった meta の生の値から、自分の値を組み立てる。前の値が無い・壊れていれば 1 から始める。
fn next_value(base: Option<&str>, tag: u32) -> String {
    let prev = base.and_then(parse_boot).unwrap_or(0);
    format_boot(prev.saturating_add(1), tag)
}

/// このページの起動番号。保存の直前の照合（[`BootGuard::is_current`]）を持つ。
pub struct BootGuard<K: SyncKv> {
    kv: K,
    meta_key: String,
    tag: u32,
    /// 取得の直前に読んだ meta の生の値（外側の `None` は「読めなかった」）。再取得の条件の基準になる。
    base: Option<Option<String>>,
    /// 自分の値。取得の書き込みに失敗したときはその理由（照合のたびに再取得を試み、成功するまで書かない）。
    mine: RefCell<Result<String, StorageError>>,
    /// 一度でも他のタブに追い越されたか。立てたら戻さない。
    stale: Cell<bool>,
}

impl<K: SyncKv> BootGuard<K> {
    /// ページを開いた時点の起動番号を取得して meta に書く。前の値が無い・壊れていれば 1 から始める。
    /// 書き込みに失敗したときは自分の値を持たず、書かない（fail-closed）。容量が戻れば [`BootGuard::verify`] が再取得する。
    pub fn claim(kv: K, key: &HistoryKey, tag: u32) -> Self {
        let meta_key = boot_meta_key(key);
        let base = kv.get(&meta_key).ok();
        let value = next_value(base.as_ref().and_then(|b| b.as_deref()), tag);
        let mine = kv.set(&meta_key, &value).map(|()| value);
        BootGuard {
            kv,
            meta_key,
            tag,
            base,
            mine: RefCell::new(mine),
            stale: Cell::new(false),
        }
    }

    /// 取得に失敗していたときの再取得。meta が取得の直前に読んだ値のままのときだけ、自分の値を書く。
    /// 別の値に変わっていれば他のタブが先に起動したので、古いタブとして記憶する（後から起動したタブを追い越さない）。
    /// 値が消えていれば保存領域の消失なので、古いとは記憶せず失敗を返す。
    /// 取得の直前に meta を読めていなければ、基準が無いので再取得しない。
    fn reclaim(&self, err: StorageError) -> Result<(), StorageError> {
        let base = self.base.as_ref().ok_or(err)?;
        let current = self.kv.get(&self.meta_key)?;
        // 取得の前に在った meta が消えているのは、他のタブの取得ではなく保存領域の消失である。verify と同じく失敗として返し、古いとは確定させない。
        if current.is_none() && base.is_some() {
            return Err(StorageError::Failed);
        }
        if current != *base {
            self.stale.set(true);
            return Ok(());
        }
        let value = next_value(base.as_deref(), self.tag);
        self.kv.set(&self.meta_key, &value)?;
        *self.mine.borrow_mut() = Ok(value);
        Ok(())
    }

    /// 保存してよいか。`Ok(true)` は meta が自分の値と一致、`Ok(false)` は別の値（古いタブとして記憶する）。
    /// 取得に失敗した・読めない・meta が無いときは、その `StorageError`（古いとは記憶しない）。取得に失敗していれば先に再取得を試みる。
    pub fn verify(&self) -> Result<bool, StorageError> {
        if self.stale.get() {
            return Ok(false);
        }
        let failed = self.mine.borrow().as_ref().err().copied();
        if let Some(e) = failed {
            self.reclaim(e)?;
            if self.stale.get() {
                return Ok(false);
            }
        }
        let mine = self.mine.borrow().clone()?;
        match self.kv.get(&self.meta_key)? {
            Some(v) if v == mine => Ok(true),
            Some(_) => {
                self.stale.set(true);
                Ok(false)
            }
            None => Err(StorageError::Failed),
        }
    }

    /// 保存してよいか。[`BootGuard::verify`] が `Ok(true)` のときだけ `true`。
    pub fn is_current(&self) -> bool {
        matches!(self.verify(), Ok(true))
    }

    /// 他のタブに追い越されたことがあるか。
    pub fn is_stale(&self) -> bool {
        self.stale.get()
    }
}

/// 書き込みの直前に [`BootGuard::is_current`] を照合する保存先の包み。
/// 書き出しは必ずこれを通す（`persist::flush_guarded` が唯一の入口）。
pub struct Guarded<'a, S: HistoryStore + ?Sized, K: SyncKv> {
    /// 包む保存先。
    pub inner: &'a S,
    /// 照合する起動番号。
    pub guard: &'a BootGuard<K>,
}

impl<S: HistoryStore + ?Sized, K: SyncKv> HistoryStore for Guarded<'_, S, K> {
    fn kind(&self) -> StorageKind {
        self.inner.kind()
    }

    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        if !self.guard.verify()? {
            return Ok(());
        }
        let out = self.inner.write(key, value);
        // 書いた直後の再照合。隙間で他のタブが起動していれば、以後の書き込みを止める。
        let _ = self.guard.verify();
        out
    }

    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        self.inner.read(key)
    }

    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        self.inner.remove(key)
    }

    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        self.inner.keys()
    }
}
