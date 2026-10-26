//! 動きの抑制（`prefers-reduced-motion: reduce`）のときに、段階の表示を出すかどうかの判定（TASK-372 1/2。A11Y-11・EVT-19）。
//!
//! 呼び出し元は、右のログの描画（`engine_parts::LogPart::view`）と試験用の状態の書き出し（`host`）である。規則を 1 か所に置き、
//! 描画と書き出しで食い違わないようにする。省くのは行の描画だけで、進行のイベント列・状態・履歴は動きを抑えない場合と同じになる。
//! `run-status` などの状態の語は段階ではないため、ここでは扱わない（TERM-33）。
//!
//! 方式は D-82（2026-10-26）で現状のまま確定した（A11Y-29・A11Y-13）。`reduce` のためにエンジンが停止を体験させる仕組みは足さず、
//! 設定は読み込み時に 1 回だけ読む（A11Y-12。追随しないことは既知の不足 H-12）。`reduce` の ST-1.5 と設定を反映する時期は
//! `tools/browser/reduced-motion-st15.test.mjs`（TASK-378 1/2）が確かめる。

use crate::log_lines::LogLine;

/// ログの 1 行を表示するなら真。`reduced_motion` のとき、段階の行（`Progress`）だけを省く。
pub fn shows_line(reduced_motion: bool, line: &LogLine) -> bool {
    !(reduced_motion && matches!(line, LogLine::Progress { .. }))
}
