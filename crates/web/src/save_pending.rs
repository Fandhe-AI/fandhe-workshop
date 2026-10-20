//! 未保存の変更の保留と、同期の書き出し（TASK-317 2/2。D-52・SAVE-9）。
//!
//! 変更の直後に保存の遅延を待たずにページを離れても、直前の変更が残るようにするための型である。
//! 呼び出し元は、変更を保存先へ流す経路（TASK-318・TASK-319）が [`PendingSave::stage`] で値を積み、
//! `persist::save_now`（状態を変える操作のたび。TASK-323 1/2）と `leave_flush`（wasm のみ）が `pagehide` の中で [`PendingSave::flush`] を同期に呼ぶ。
//! 呼び出し先は contract の [`HistoryStore`] だけで、ブラウザの API には触れない（native で試験できる）。
//!
//! 保留は鍵ごとに最後の 1 つへ畳む。書き出しで書けなかった鍵は保留に残し、次の書き出しで再び試す
//! （既存の保存も、メモリ上の値も失わない）。
//!
//! 限界（SAVE-9 の検討中の論点。仕様の決定は spec 側で行う）:
//! - 保存先が localStorage のときは `write` が同期で `setItem` まで終わるため、`flush` の戻りの時点で書き込みは完了している。
//!   IndexedDB へ切り替えた後は `put` の発行までで、確定は保証しない。離れる直前の変更は落ちうる
//! - 準備完了（`storage_idb::open_history_storage` の合図）の前の保留は書かない。呼び出し側が保存先を持った後にだけ書き出す
//! - 保存の遅延のタイマーは `glue`、判定は `save_schedule`、Session の変更を保留へ積む配線は `persist`（TASK-318・TASK-319）にあり、この module の外である
//!
//! 値と鍵の文字列は、結果にも記録にも含めない（履歴は下書きを含む受講者のデータである）。

use fandhe_workshop_contract::storage::{HistoryKey, HistoryStore, StorageError};
use std::cell::RefCell;
use std::collections::BTreeMap;

/// 書き出しの結果。値・鍵の文字列は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlushOutcome {
    /// 保存先へ書けた件数。
    pub written: usize,
    /// 書けずに保留へ残した件数。
    pub failed: usize,
    /// 最初の失敗の種類。失敗が無ければ `None`。
    pub first_error: Option<StorageError>,
}

/// 鍵ごとの未保存の値の保留。
#[derive(Default)]
pub struct PendingSave {
    pending: RefCell<BTreeMap<HistoryKey, String>>,
}

impl PendingSave {
    /// 空の保留を作る。
    pub fn new() -> Self {
        Self::default()
    }

    /// 未保存の値を積む。同じ鍵の値は置き換わり、最後の 1 つだけが残る。保存先には書かない。
    pub fn stage(&self, key: HistoryKey, value: String) {
        self.pending.borrow_mut().insert(key, value);
    }

    /// 保留があるか。
    pub fn has_pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

    /// 保留の件数（値は返さない）。
    pub fn pending_len(&self) -> usize {
        self.pending.borrow().len()
    }

    /// 保留を鍵の順に保存先へ書く。書けた鍵は保留から外し、書けなかった鍵は残す。
    ///
    /// `write` の呼び出しの間は内部の借用を持たない（`write` が同じ保留へ積み直しても二重借用にならない）。
    /// 書き出しの間に新しい値が積まれた鍵は、失敗しても新しい値を優先する。
    pub fn flush<S: HistoryStore + ?Sized>(&self, store: &S) -> FlushOutcome {
        let taken = self.pending.replace(BTreeMap::new());
        let mut outcome = FlushOutcome {
            written: 0,
            failed: 0,
            first_error: None,
        };
        for (key, value) in taken {
            match store.write(&key, &value) {
                Ok(()) => outcome.written += 1,
                Err(e) => {
                    outcome.failed += 1;
                    outcome.first_error.get_or_insert(e);
                    self.pending.borrow_mut().entry(key).or_insert(value);
                }
            }
        }
        outcome
    }
}
