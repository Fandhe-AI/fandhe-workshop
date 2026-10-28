//! 履歴の包み（書き出しの形）の大きさを測る共有部品（TASK-216。EVT-13、SAVE-26）。
//!
//! 呼び出し元は `history_size_rounds.rs` と `history_size_conditions.rs`（TASK-336。SAVE-26。条件 B／C と 5MB の判定）、web の `history_size_all_chapters.rs`（TASK-337。SAVE-27）。同じ測定を使えるよう、
//! 測り方の定義を 1 か所に置く。列の名前は PoC-6 の CSV（`testdata/save-restore-versions/test-data/synth-inputs.md`）に揃える。
//! 履歴の中身（下書き・本文）は返さず、大きさと件数だけを返す。

#![allow(dead_code, reason = "試験ファイルごとに使う部品が異なるため")]

use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::SessionEnvelope;

/// 1 つの履歴の大きさ。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistorySize {
    /// 書き出し（`export_bytes`）のバイト数。
    pub total_bytes: u64,
    /// `ops` の件数。
    pub ops: u64,
    /// `ops` を単独で JSON にしたバイト数。
    pub ops_bytes: u64,
    /// `snapshot` を単独で JSON にしたバイト数。
    pub snapshot_bytes: u64,
    /// `snapshot.log` の件数。
    pub log_entries: u64,
    /// `snapshot.log` の中の `Event::Changed` の件数。
    pub changed_entries: u64,
    /// `Event::Changed` を 1 件ずつ JSON にしたバイト数の合計。
    pub changed_bytes: u64,
    /// チェックポイントの件数。
    pub checkpoints: u64,
    /// チェックポイントの表を単独で JSON にしたバイト数。
    pub checkpoint_bytes: u64,
}

impl HistorySize {
    /// `changed_bytes` が書き出し全体に占める割合（百万分率。切り捨て）。
    pub fn changed_ratio_ppm(&self) -> u64 {
        self.changed_bytes * 1_000_000 / self.total_bytes
    }
}

fn len_of<T: serde::Serialize + ?Sized>(v: &T) -> u64 {
    serde_json::to_vec(v).expect("serialize").len() as u64
}

/// 包みと、その書き出し（`export_bytes`）のバイト列から大きさを求める。
pub fn measure(env: &SessionEnvelope, bytes: &[u8]) -> HistorySize {
    let log = &env.snapshot.log;
    let changed: Vec<&Event> = log
        .iter()
        .filter(|e| matches!(e, Event::Changed { .. }))
        .collect();
    HistorySize {
        total_bytes: bytes.len() as u64,
        ops: env.ops.len() as u64,
        ops_bytes: len_of(&env.ops),
        snapshot_bytes: len_of(&env.snapshot),
        log_entries: log.len() as u64,
        changed_entries: changed.len() as u64,
        changed_bytes: changed.iter().map(|e| len_of(*e)).sum(),
        checkpoints: env.snapshot.checkpoints.len() as u64,
        checkpoint_bytes: len_of(&env.snapshot.checkpoints),
    }
}
