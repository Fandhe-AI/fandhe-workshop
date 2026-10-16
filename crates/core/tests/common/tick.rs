//! 進行 `Op::Tick` の試験の補助。
//!
//! 「次の進行の宛先」を状態から作る。web の側の同等の補助（FEND-4）は本件では足さない。

use fandhe_workshop_core::addr::TickAddr;
use fandhe_workshop_core::session::SessionState;

/// 現在の世代・現在の Step・処理中の `done` から、次に届くべき進行の宛先を作る。処理中でなければ `None`。
pub fn next_tick(state: &SessionState) -> Option<TickAddr> {
    let step = state.run.current.clone()?;
    let busy = state.run.busy.as_ref()?;
    Some(TickAddr {
        generation: state.run.generation,
        step,
        index: busy.done,
    })
}
