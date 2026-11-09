//! 受講者の選択による案内レベルの切り替え `Control::SetGuidance` の分岐（TASK-294 2/2、GUIDE-3）。
//!
//! 案内レベルは実行全体で 1 つ（GUIDE-3）。受講者が選んだ水準を `run.guidance_level` へ書くだけで、
//! 判定の経路（受付・判定・確認・進行）は水準を読まないので、切り替えで判定の結果は変わらない
//! （試験は `tests/guidance_judge_invariance.rs`）。水準を読むのはヒントの選択（`hint_select`）と web の表示の充填だけである。
//! 呼び出し元は [`crate::session::apply`] だけで、公開 API には含めない（契約の対象外）。
//!
//! # 規則
//!
//! 開始前・処理中・章の完了後のどれでも受け付け、水準を書いて空のイベント列を返す。世代は変わらない
//! （`Keep`。`apply` の関門が表を強制し、本モジュールは世代に触れない）。世界状態も変えないので `Changed` も出ない。
//! 処理中の停止は水準を戻さない（受講者の選択は処理の効果ではない。`stop`）。復帰は写しの水準へ戻す（MOD-9）。
//!
//! # 扱わないこと
//!
//! - 切り替えのイベント化と履歴での扱い（GUIDE-16、TASK-307・TASK-308）。操作の記録（`ops`）には残る
//! - 画面の切り替えの部品（SHELL-37）、`Session::entered_at` での開始点の水準の反映
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!`・添字の直接参照を使わない）で決定的。I/O・時刻・乱数を持たず、IR を読まない。

use fandhe_workshop_contract::guidance::GuidanceLevel;

use crate::event::Event;
use crate::session::SessionState;

/// `Control::SetGuidance` の分岐。水準を書き、イベントは出さない。
pub(crate) fn on_set_guidance(state: &mut SessionState, level: GuidanceLevel) -> Vec<Event> {
    state.run.guidance_level = level;
    Vec::new()
}
