//! 推定した OS に合う修飾キーの表示の文言（決定 D-48・JUDGE-28。TASK-285 1/2）。
//!
//! core の [`LearnerOs`]（User-Agent からの推定）を受け、画面に出す固定の文字列だけを返す純関数を置く。
//! 呼び出し元は `hello` の入力欄の表示（`key-hint`）で、後続の操作プロファイルの説明・ヒントへの適用
//! （TASK-392・TASK-456）も同じ関数を使う。User-Agent そのものは受け取らず、画面にも属性にも出さない。
//! 推定できない OS（`Unknown`）は修飾キーを出さず、OS を選ぶよう促す（Ctrl に倒さない）。
//! 受講者が選ぶ欄（`os-select`。TASK-285 2/2）の値と OS の対応も、この 1 か所に置く。

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

/// 貼り付けのキーの案内文。修飾キーが決まらないときは、Cmd・Ctrl を出さず OS を選ぶよう促す。
#[must_use]
pub fn paste_key_hint(os: LearnerOs) -> &'static str {
    match os {
        LearnerOs::Mac => "貼り付け: Cmd+V",
        LearnerOs::Windows => "貼り付け: Ctrl+V",
        LearnerOs::Unknown => "OS を選ぶと、貼り付けのキーが表示されます",
    }
}

/// 選ぶ欄の option の値。`Unknown` は選択肢ではない（空の option で表す）。
#[must_use]
pub fn os_choice_value(os: LearnerOs) -> Option<&'static str> {
    match os {
        LearnerOs::Mac => Some("mac"),
        LearnerOs::Windows => Some("windows"),
        LearnerOs::Unknown => None,
    }
}

/// 選ぶ欄の `change` の payload を OS にする。`mac`・`windows` の完全一致だけを受け、
/// 空・未知・大文字違いは `None`（信頼できない入力として何もしない）。
#[must_use]
pub fn parse_os_choice(payload: &str) -> Option<LearnerOs> {
    match payload {
        "mac" => Some(LearnerOs::Mac),
        "windows" => Some(LearnerOs::Windows),
        _ => None,
    }
}
