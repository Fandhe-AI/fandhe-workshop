//! 動きの抑制（`prefers-reduced-motion: reduce`）のときに、段階の表示を出すかどうかの判定（TASK-372 1/2。A11Y-11・EVT-19）。
//!
//! 呼び出し元は、右のログの描画（`engine_parts::LogPart::view`）と試験用の状態の書き出し（`host`）である。規則を 1 か所に置き、
//! 描画と書き出しで食い違わないようにする。省くのは行の描画だけで、進行のイベント列・状態・履歴は動きを抑えない場合と同じになる。
//! `run-status` などの状態の語は段階ではないため、ここでは扱わない（TERM-33）。
//!
//! 仮のもの: 段階を省く方式の採否と、処理中の止め方（LO-3）への影響は TASK-377・TASK-378 が決める（A11Y-29・A11Y-13）。

use crate::log_lines::LogLine;

/// ログの 1 行を表示するなら真。`reduced_motion` のとき、段階の行（`Progress`）だけを省く。
pub fn shows_line(reduced_motion: bool, line: &LogLine) -> bool {
    !(reduced_motion && matches!(line, LogLine::Progress { .. }))
}
