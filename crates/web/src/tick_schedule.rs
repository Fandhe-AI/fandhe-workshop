//! 次の進行の宛先を組み立てる純関数（TASK-349 1/2。FEND-4）。
//!
//! 呼び出し元は [`EngineHost`](crate::host::EngineHost) で、`Session` を更新するたびに、表示側のタイマーが
//! 次に届けるべき `tick` の宛先を glue へ知らせるために使う。ここは宛先を組み立てるだけで、届いた進行を
//! 受け付けるかどうかの判定（世代・Step・番号の照合。EVT-8・EVT-9）は core の `judge_tick` が持つ。
//! 表示層は進行管理を持たない（`tests/no_progress.rs` が固定する）ため、処理中の状態は読み取りだけに使う。

use fandhe_workshop_core::addr::TickAddr;
use fandhe_workshop_core::Session;

/// 処理中の Step の、次に届けるべき進行の宛先。処理中でなければ `None`。
///
/// 番号は core が持つ「次に来るべき進行の番号」をそのまま使う。
pub fn next_tick(session: &Session) -> Option<TickAddr> {
    let run = &session.state().run;
    let step = run.current.as_ref()?;
    let index = run.busy.as_ref().map(|b| b.done)?;
    Some(TickAddr {
        generation: run.generation,
        step: step.clone(),
        index,
    })
}
