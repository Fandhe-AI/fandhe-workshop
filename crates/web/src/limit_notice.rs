//! ログの通知欄（`log-notice`）に出す、遷移の上限で止めた理由の導出（SAFE-34・D-45・TASK-160 2/3）。
//!
//! 呼び出し元は `engine_parts` の `LogPart` で、描画のたびと束縛点の更新のたびに [`limit_notice`] を引いて
//! 通知欄の文を決める。右のログの見出しの行（`log_lines` の `limit`）と同じ事実を、案内の欄にも出す。
//! 文は固定の定数と、着地の回数（`u32`）を 10 進で区切った数だけで作り、教材・受講者の値を含めない（SAFE-11）。
//!
//! 規則は `run_status` と同じ形で、未読込は空に倒す。ログを末尾から読み、止めた直後に core が足す
//! `Changed`・`Ignored`・`Warning`・`Reloaded` は読み飛ばし、最初に当たったイベントが `TransitionLimitReached`
//! なら通知文、それ以外（未知のイベントを含む）は空文字列にする。許可リストを狭くするのは、止めた後に受講者が
//! 次の操作（移動・復帰・開始など）をしたら、古い理由を消すため。止めた後の戻り方（SAFE-35）が足すイベントも、
//! この許可リストに無いので自然に通知を消す。

use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::Session;

use crate::log_lines::group_digits;

/// 通知文で、数の前に置く固定の文言。
pub const LIMIT_NOTICE_BEFORE: &str = "実行時の上限（1 回の操作で内部の遷移 ";

/// 通知文で、数の後に置く固定の文言。
pub const LIMIT_NOTICE_AFTER: &str = " 回）に達したため、実行を止めました。";

/// 通知文を作る。文言は決定（D-45）に記録が無く、エンジンの側の仮置き。
pub fn limit_notice_text(landings: u32) -> String {
    format!(
        "{LIMIT_NOTICE_BEFORE}{}{LIMIT_NOTICE_AFTER}",
        group_digits(landings)
    )
}

/// 通知欄の文。直近が上限での停止でなければ空文字列。
pub fn limit_notice(session: Option<&Session>) -> String {
    let Some(s) = session else {
        return String::new();
    };
    for ev in s.log().iter().rev() {
        match ev {
            Event::Changed { .. }
            | Event::Ignored { .. }
            | Event::Warning { .. }
            | Event::Reloaded { .. } => {}
            Event::TransitionLimitReached { landings, .. } => {
                return limit_notice_text(*landings);
            }
            _ => return String::new(),
        }
    }
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_session_is_empty() {
        assert_eq!(limit_notice(None), "");
    }

    #[test]
    fn text_uses_grouped_number() {
        assert_eq!(
            limit_notice_text(1000),
            "実行時の上限（1 回の操作で内部の遷移 1,000 回）に達したため、実行を止めました。"
        );
    }
}
