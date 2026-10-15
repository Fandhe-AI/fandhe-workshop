//! Step の状態の型（TASK-204 1/3）。呼び出し元は表示・保存・案内の担当。契約の版は [`crate::CONTRACT_VERSION`]。

use serde::{Deserialize, Serialize};

/// Step の状態。
///
/// 仕様の 4 状態（未着手・受付済み・達成・省略）に、`Entered` を加えた 5 値である。
/// MOD-3・MOD-6・EVT-20 が「現在の Step は `entered`」と状態名で書くため、版 1 で落とすと後から版上げになる。
///
/// | 値 | 意味 |
/// |---|---|
/// | `NotStarted` | 未着手（未登録の Step もこれ） |
/// | `Entered` | 入って入力を待つ |
/// | `Accepted` | 入力を受け付けた（受付済み） |
/// | `Achieved` | 完了条件を満たした（達成） |
/// | `Skipped` | 省略した |
///
/// 途中開始で省かれた Step の `Skipped` と、まだ着手していない `NotStarted` は別である。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// 未着手。
    #[default]
    NotStarted,
    /// 入って入力を待つ。
    Entered,
    /// 受付済み。
    Accepted,
    /// 達成。
    Achieved,
    /// 省略。
    Skipped,
}
