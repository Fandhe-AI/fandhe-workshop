//! 履歴を保存できなかったことの通知欄（`save-notice`）の文（SAVE-10・SHELL-18・TASK-323 1/2）。
//!
//! 呼び出し元は `engine_parts` の `LogPart` で、描画のたびと束縛点の更新のたびに [`save_notice_text`] を引く。
//! 引く値は `HostModel::save_failure`（`EngineHost` が、変更のたびの保存の結果から入れる）である。
//! 文は固定の定数だけで作り、例外の文・鍵・値・教材・受講者の値を含めない（履歴は受講者のデータである。`storage_local` と同じ方針）。
//!
//! `QuotaExceeded` の文は SAVE-10 の文言どおり。`Unavailable`・`Failed` の文は仮置きで、確定は TASK-534 と TASK-323 2/2 が持つ。

use fandhe_workshop_contract::storage::StorageError;

/// 通知欄の束縛点の名前（`engine_parts` の `F_LOG` の末尾と同じ。ホストが dirty に積むときに使う）。
pub const SAVE_NOTICE_FIELD: &str = "save_notice";

/// 容量超過（`QuotaExceededError`）の文。SAVE-10 の文言。
pub const SAVE_QUOTA_NOTICE: &str = "保存できませんでした（QuotaExceededError）";

/// 保存領域を使えないときの文（仮置き）。
pub const SAVE_UNAVAILABLE_NOTICE: &str = "保存できませんでした（保存領域を使えません）";

/// その他の失敗の文（仮置き）。
pub const SAVE_FAILED_NOTICE: &str = "保存できませんでした（書き込みに失敗しました）";

/// 通知欄の文。失敗が無ければ空文字列（表示が消える）。
pub fn save_notice_text(failure: Option<StorageError>) -> &'static str {
    match failure {
        None => "",
        Some(StorageError::QuotaExceeded) => SAVE_QUOTA_NOTICE,
        Some(StorageError::Unavailable) => SAVE_UNAVAILABLE_NOTICE,
        Some(StorageError::Failed) => SAVE_FAILED_NOTICE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_empty() {
        assert_eq!(save_notice_text(None), "");
    }

    #[test]
    fn quota_uses_the_spec_wording() {
        assert_eq!(
            save_notice_text(Some(StorageError::QuotaExceeded)),
            "保存できませんでした（QuotaExceededError）"
        );
    }

    #[test]
    fn other_failures_have_distinct_nonempty_text() {
        let a = save_notice_text(Some(StorageError::Unavailable));
        let b = save_notice_text(Some(StorageError::Failed));
        assert!(!a.is_empty() && !b.is_empty() && a != b);
    }
}
