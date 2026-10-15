//! 宛先の部品（世代・Step の実体 ID・進行の宛先）のデータの形（TASK-204 1/3）。
//!
//! `Op::Tick` とすべてのイベントの `at` が参照する。呼び出し元は web・pack・保存の担当で、
//! 本モジュールは形だけを持つ。実体 ID の書式検査・宛先の照合・世代の変わり方は TASK-204 の 2/3 が持つ。
//! 版（`contract_version`）も 2/3 で付ける。

use serde::{Deserialize, Serialize};

/// 実行の世代。入力の判定が古い結果を捨てるための印である（値の増やし方は 2/3 が決める）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Generation(pub u64);

/// Step の実体 ID（`<モジュール ID>@<実体名>/<Step>`。章は `main/<Step>`）。IR の `Node::uid` と同じ値を持つ。
///
/// 書式は本型では検査しない（検査は 2/3）。値は識別子として持つだけで、ファイル I/O や HTML には使わない。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StepUid(pub String);

/// すべてのイベントが持つ「その時点の世代と Step」。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Stamp {
    /// その時点の世代。
    pub generation: Generation,
    /// その時点の Step の実体 ID。
    pub step: StepUid,
}

/// 進行（`Tick`）の宛先。世代・Step の実体 ID・何番目の進行かの 3 要素で届き先を一意にする（EVT-8）。
///
/// `index` は、同じ番号の進行が二重に届いても 1 回だけ効かせるために要る（F-5）。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TickAddr {
    /// 世代。
    pub generation: Generation,
    /// Step の実体 ID。
    pub step: StepUid,
    /// その Step の何番目の進行か（0 始まり）。
    pub index: u32,
}
