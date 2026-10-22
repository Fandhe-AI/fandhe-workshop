//! 履歴を保存できなかったことの通知の文と理由コード（SAVE-10・SHELL-18・TASK-323 1/2・TASK-534 1/2）。
//!
//! 呼び出し元は `shell_notice` で、枠の通知欄（`shell-notice`）の `save-failed` 項目を作るときに [`save_notice_text`]・[`save_failure_reason`] を引く。
//! 引く値は `HostModel::save_failure`（`EngineHost` が、変更のたびの保存の結果から入れる）である。
//! IndexedDB の保存の失敗（SAVE-11。非同期で届き、ホストが次の `update` で probe から求める）も同じ欄・同じ文で出す。
//! 文は固定の定数だけで作り、例外の文・鍵・値・教材・受講者の値を含めない（履歴は受講者のデータである。`storage_local` と同じ方針）。
//!
//! `QuotaExceeded` の文は SAVE-10 の文言どおり。`Unavailable`・`Failed` の文は仮置きで、確定は TASK-534 と TASK-323 2/2 が持つ。

use fandhe_workshop_contract::storage::StorageError;

/// 遅延保存の結果をホストへ届ける action の名前（`decode_action` が受ける私的な名前。`RuntimeAction` の契約には入れない）。
pub const ACTION_SAVE_RESULT: &str = "save-result";

/// 遅延保存の結果の payload。`<保存世代> <ok|失敗の種類の名前>`（値・鍵は含めない）。
///
/// 世代は、書き込んだ時点のホストの保存世代（`EngineHost::save_epoch`）。配送が遅れる間に即時の保存が走ったとき、
/// 古い結果が新しい通知を上書きしないよう、ホストが現在の世代との一致を見て捨てる（SAVE-10）。
pub fn encode_save_result(epoch: u64, result: Result<(), StorageError>) -> String {
    let kind = match result {
        Ok(()) => "ok",
        Err(StorageError::QuotaExceeded) => "quota",
        Err(StorageError::Unavailable) => "unavailable",
        Err(StorageError::Failed) => "failed",
    };
    format!("{epoch} {kind}")
}

/// [`encode_save_result`] の逆。完全一致だけを受け、未知の payload は `None`（信頼しない入力）。
/// 世代は正規形の 10 進（先頭 0 なし・u64 に収まる）だけを受ける。
pub fn decode_save_result(payload: &str) -> Option<(u64, Option<StorageError>)> {
    let (epoch, kind) = payload.split_once(' ')?;
    if epoch.is_empty() || (epoch.len() > 1 && epoch.starts_with('0')) {
        return None;
    }
    if !epoch.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let epoch = epoch.parse::<u64>().ok()?;
    let failure = match kind {
        "ok" => None,
        "quota" => Some(StorageError::QuotaExceeded),
        "unavailable" => Some(StorageError::Unavailable),
        "failed" => Some(StorageError::Failed),
        _ => return None,
    };
    Some((epoch, failure))
}

/// 容量超過（`QuotaExceededError`）の文。SAVE-10 の文言。
pub const SAVE_QUOTA_NOTICE: &str = "保存できませんでした（QuotaExceededError）";

/// 保存領域を使えないときの文（仮置き）。
pub const SAVE_UNAVAILABLE_NOTICE: &str = "保存できませんでした（保存領域を使えません）";

/// その他の失敗の文（仮置き）。
pub const SAVE_FAILED_NOTICE: &str = "保存できませんでした（書き込みに失敗しました）";

/// 通知の理由コード（`data-notice-reason`）。閉じた列挙から固定の ASCII へ写し、契約の `ReasonCode::parse` を通る形にする。
pub fn save_failure_reason(failure: StorageError) -> &'static str {
    match failure {
        StorageError::QuotaExceeded => "quota_exceeded",
        StorageError::Unavailable => "storage_unavailable",
        StorageError::Failed => "write_failed",
    }
}

/// 通知の文。失敗が無ければ空文字列。
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
    fn reasons_are_distinct_and_parse() {
        let all = [
            StorageError::QuotaExceeded,
            StorageError::Unavailable,
            StorageError::Failed,
        ];
        let r: Vec<&str> = all.into_iter().map(save_failure_reason).collect();
        assert!(r[0] != r[1] && r[1] != r[2] && r[0] != r[2]);
        for x in r {
            assert!(crate::shell::ReasonCode::parse(x).is_some(), "{x}");
        }
    }

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
