//! 推定した OS に合う修飾キーの表示の文言（決定 D-48・JUDGE-28。TASK-285 1/2）。
//!
//! core の [`LearnerOs`]（User-Agent からの推定）を受け、画面に出す固定の文字列だけを返す純関数を置く。
//! 呼び出し元は `hello` の入力欄の表示（`key-hint`）で、後続の操作プロファイルの説明・ヒントへの適用
//! （TASK-392・TASK-456）も同じ関数を使う。User-Agent そのものは受け取らず、画面にも属性にも出さない。
//! 推定できない OS（`Unknown`）は修飾キーを出さない（Ctrl に倒さない）。受講者が選ぶ欄は TASK-285 2/2 の担当。

use fandhe_workshop_core::learner_os::LearnerOs;

/// 修飾キーの語。Mac は `Cmd`、Windows は `Ctrl`、推定できなければ `None`。
#[must_use]
pub fn modifier_label(os: LearnerOs) -> Option<&'static str> {
    match os {
        LearnerOs::Mac => Some("Cmd"),
        LearnerOs::Windows => Some("Ctrl"),
        LearnerOs::Unknown => None,
    }
}

/// 貼り付けのキーの案内文。修飾キーが決まらないときは空文字列（案内を出さない）。
#[must_use]
pub fn paste_key_hint(os: LearnerOs) -> &'static str {
    match os {
        LearnerOs::Mac => "貼り付け: Cmd+V",
        LearnerOs::Windows => "貼り付け: Ctrl+V",
        LearnerOs::Unknown => "",
    }
}
