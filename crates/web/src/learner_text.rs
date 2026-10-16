//! 受講者に由来する文字列を、解釈せずに文字として表示する（SAFE-11。TASK-150）。
//!
//! 受講者が送信した入力と、履歴から復元した `Input::Text`・`Input::Command` の値は、Markdown でも HTML でも
//! ない。見出し・リンク・画像・コードブロックなどの記法を要素にせず、書いたとおりの文字で出す。
//! そのため、web crate で受講者の文字列を描く経路はこの関数だけにする。子は `text()`（描画時にエスケープする
//! 文字ノード）の 1 つに限り、生 HTML の出力口・Markdown の変換・属性への埋め込みは使わない。Markdown の解釈器は
//! pack crate だけにあり、web からは依存できない（`make check-deps`）ため、実行時に解釈する経路は構造上ない。
//!
//! 呼び出し元は `hello.rs` の送信済みの一覧と、右のログの行（`log_lines`。TASK-434。部品の要素は [`learner_part`]）。
//! 改行は文字ノードの中にそのまま残る。見た目の改行（`white-space`）は、スタイルシートを持つ後続のタスクに任せる
//! （`style` 属性は SAFE-15 で使えない）。

use fandhe_frontend_core::{el, text, Node};

/// 受講者の文字列 1 件を、文字ノードだけを子に持つ一覧の項目として返す。
///
/// 文字列は属性値（`href`・`src`・`class`・`data-*`）に入れない。
pub fn learner_line(s: &str) -> Node {
    el("li", vec![], vec![text(s)])
}

/// 受講者・教材の文字列 1 件を、文字ノードだけを子に持つ部品の要素（`span`）として返す。
///
/// 呼び出し元は右のログの行（`log_lines`）。`testid` は `terminal_area` が定める固定の識別子で、
/// 文字列そのものは属性値に入れない（子の `text()` だけで描く）。
pub fn learner_part(testid: &'static str, s: &str) -> Node {
    el("span", vec![("data-testid", testid)], vec![text(s)])
}
