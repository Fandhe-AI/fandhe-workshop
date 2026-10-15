//! 契約のサンプル（`testdata/contract/event-sequence.json`、TASK-205）を core の型で読む部品。
//!
//! 呼び出し元は、サンプルの形を確かめる `contract_sample.rs` と、サンプルを使う適合試験（`contract_conformance.rs`、TASK-205 2/2）。
//! 型は core の公開型をそのまま使う。契約の型を変えるとサンプルが読めなくなり、サンプルの版上げを強制できる。
//! 試験の部品なので、読めない場合は理由を添えて panic する。

use std::path::PathBuf;

use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::{RunState, SessionState};
use serde::Deserialize;

/// 操作の種類（サンプルの `kinds`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// 受付。
    Accept,
    /// 拒否。
    Reject,
    /// 確認。
    Confirm,
    /// 達成。
    Achieve,
    /// 進行。
    Progress,
    /// 停止。
    Stop,
    /// 移動。
    Move,
    /// 保存。
    Save,
    /// 復帰。
    Restore,
}

impl Kind {
    /// 9 種すべて。
    pub const ALL: [Kind; 9] = [
        Kind::Accept,
        Kind::Reject,
        Kind::Confirm,
        Kind::Achieve,
        Kind::Progress,
        Kind::Stop,
        Kind::Move,
        Kind::Save,
        Kind::Restore,
    ];
}

/// 操作 1 件の前後の状態と、期待のイベント列。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleEntry {
    /// `e01` から始まる連番。
    pub id: String,
    /// 該当する操作の種類。
    pub kinds: Vec<Kind>,
    /// EVT-6 の項番（1〜6）。
    pub evt6: u8,
    /// 日本語の短い説明。
    pub note: String,
    /// 適用する操作。
    pub op: Op,
    /// 適用前の実行状態。
    pub before: RunState,
    /// 適用後の実行状態。
    pub after: RunState,
    /// 期待のイベント列。
    pub events: Vec<Event>,
}

/// サンプル全体。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sample {
    /// サンプルの版。
    pub sample_version: u32,
    /// 対象の `CONTRACT_VERSION`。
    pub contract_version: u32,
    /// 教材（IR）の `testdata/` からの相対パス。
    pub ir: String,
    /// 最初の状態。
    pub initial: SessionState,
    /// 操作の列。
    pub entries: Vec<SampleEntry>,
}

/// `testdata/` の絶対パス。
pub fn testdata_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// サンプルを読む。形が合わなければ panic する。
pub fn load() -> Sample {
    let path = testdata_dir().join("contract/event-sequence.json");
    let bytes = std::fs::read(&path).expect("contract sample must be readable");
    serde_json::from_slice(&bytes).expect("contract sample must match the core types")
}
