//! 入力欄 1 回分の入力の上限（SAFE-21。TASK-158）。
//!
//! 巨大な貼り付けによるメモリ・処理時間の浪費を防ぐため、状態へ入れる前に値を
//! コードポイントで 20,000 文字に切る。`hello` の画面が `SetDraft` を受けるときに呼ぶ純粋関数で、
//! DOM に触れない。値は contract の `limits`（上限の正）から取り、ここでは再定義しない。
//! HTML の `maxlength` は UTF-16 の単位で数えるため使えない。

use fandhe_workshop_contract::limits::{INPUT_TOO_LONG_MESSAGE, MAX_INPUT_CODE_POINTS};

/// 値を上限（コードポイント）で切る。戻り値は切った後の値と、切り捨てが起きたかどうか。
///
/// 切る位置は値全体の 20,001 コードポイント目以降（TERM-16 は検討中のため、挿入位置は見ない）。
/// 文字（`char`）の境界で切るため、サロゲートペアの文字を割らない。走査は上限で止まるので、
/// 200,000 文字の貼り付けでも O(上限) で済む。
pub fn clamp_draft(value: &str) -> (&str, bool) {
    // バイト数が上限以下なら、コードポイント数も上限以下（1 文字は 1 バイト以上）。
    if value.len() <= MAX_INPUT_CODE_POINTS {
        return (value, false);
    }
    match value.char_indices().nth(MAX_INPUT_CODE_POINTS) {
        Some((idx, _)) => (&value[..idx], true),
        None => (value, false),
    }
}

/// 超過の状態に対応する案内文。超過でなければ空文字列（案内を消す）。
pub fn notice_for(over_limit: bool) -> &'static str {
    if over_limit {
        INPUT_TOO_LONG_MESSAGE
    } else {
        ""
    }
}
