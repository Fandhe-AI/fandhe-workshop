//! 状態表示（`run-status`）の導出（TASK-436 1/2。TERM-33）。
//!
//! 呼び出し元は `engine_parts` の `LogPart` で、描画のたびと束縛点の更新のたびに [`run_status`] を引いて
//! 右の操作エリアの状態表示の値（待機中・処理中）を決める。値と語そのものは `terminal_area::RunStatus`
//! （TASK-431 の契約）が持ち、ここは `Session` からどの値を選ぶかだけを持つ。
//!
//! 表示層は進行管理を持たない（`tests/no_progress.rs` が固定する）ため、`Session` は読み取りだけに使う。
//! 処理中かどうかは core が持つ `run.busy` の有無そのままで、処理が終わって `busy` が `None` になれば
//! `Idle` に戻る。未読込は安全側の `Idle` に倒す。
//!
//! `Stopped` と、停止から待機中へ戻す時機は TASK-436 2/2（#325）がこの関数に分岐を足す。

use fandhe_workshop_core::Session;

use crate::terminal_area::RunStatus;

/// 状態表示の値。処理中の Step があれば `Busy`、それ以外（未読込を含む）は `Idle`。
pub fn run_status(session: Option<&Session>) -> RunStatus {
    match session {
        Some(s) if s.state().run.busy.is_some() => RunStatus::Busy,
        _ => RunStatus::Idle,
    }
}
