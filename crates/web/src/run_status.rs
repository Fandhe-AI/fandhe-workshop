//! 状態表示（`run-status`）の導出（TASK-436 1/2。TERM-33）。
//!
//! 呼び出し元は `engine_parts` の `LogPart` で、描画のたびと束縛点の更新のたびに [`run_status`] を引いて
//! 右の操作エリアの状態表示の値（待機中・処理中）を決める。値と語そのものは `terminal_area::RunStatus`
//! （TASK-431 の契約）が持ち、ここは `Session` からどの値を選ぶかだけを持つ。
//!
//! 表示層は進行管理を持たない（`tests/no_progress.rs` が固定する）ため、`Session` は読み取りだけに使う。
//!
//! 規則（TASK-436 2/2）は次の順で、未読込は安全側の `Idle` に倒す。
//!
//! 1. 処理中（`run.busy` あり）なら `Busy`。再送信の直後は即座に `Busy` へ戻る。
//! 2. そうでなければログを末尾から読む。停止の直後に core が足す `Changed`（巻き戻しの差分）、停止後に
//!    遅れて届く古い進行の `Ignored`、`Warning` は読み飛ばす。最初に当たったイベントが `Stopped` なら
//!    `Stopped`、それ以外（未知のイベントを含む）は `Idle` に倒す。
//!
//! 読み飛ばすイベントを狭い許可リストにするのは、停止の直後に付く `Changed` などで「停止」が一瞬で
//! 消えないようにしつつ、受講者が次の操作（入力・移動・復帰）をしたら待機中へ戻すため。

use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::Session;

use crate::terminal_area::RunStatus;

/// 状態表示の値。処理中の Step があれば `Busy`、停止の後で次の操作が無ければ `Stopped`、それ以外は `Idle`。
pub fn run_status(session: Option<&Session>) -> RunStatus {
    let Some(s) = session else {
        return RunStatus::Idle;
    };
    if s.state().run.busy.is_some() {
        return RunStatus::Busy;
    }
    for ev in s.log().iter().rev() {
        match ev {
            Event::Changed { .. } | Event::Ignored { .. } | Event::Warning { .. } => {}
            Event::Stopped { .. } => return RunStatus::Stopped,
            _ => return RunStatus::Idle,
        }
    }
    RunStatus::Idle
}
