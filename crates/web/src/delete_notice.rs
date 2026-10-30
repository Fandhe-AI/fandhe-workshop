//! 保存済みの履歴の削除の完了をホストへ届ける私的な action の符号化（SAVE-32。TASK-341）。
//!
//! 呼び出し元は `persist`（`Scheduled::DeleteResult`）と `host::decode_action`。削除は IndexedDB の delete の確定を待つため
//! 非同期で、完了は配送のタイマー経由でホストへ届く。鍵・値は含めず、世代と件数・失敗の種類だけを運ぶ。

use fandhe_workshop_contract::storage::StorageError;

/// 削除の完了を届ける action の名前（`RuntimeAction` の契約には入れない）。
pub const ACTION_DELETE_RESULT: &str = "delete-result";

/// payload は `<世代> ok <件数>` か `<世代> <unavailable|failed|quota>`。
pub fn encode_delete_result(generation: u64, result: Result<usize, StorageError>) -> String {
    match result {
        Ok(n) => format!("{generation} ok {n}"),
        Err(StorageError::QuotaExceeded) => format!("{generation} quota"),
        Err(StorageError::Unavailable) => format!("{generation} unavailable"),
        Err(StorageError::Failed) => format!("{generation} failed"),
    }
}

fn canonical(s: &str) -> Option<u64> {
    if s.is_empty() || s.len() > 20 || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if s.len() > 1 && s.starts_with('0') {
        return None;
    }
    s.parse().ok()
}

/// [`encode_delete_result`] の逆。完全一致だけを受け、未知の payload は `None`（信頼しない入力）。
pub fn decode_delete_result(payload: &str) -> Option<(u64, Result<usize, StorageError>)> {
    let mut parts = payload.splitn(3, ' ');
    let generation = canonical(parts.next()?)?;
    let kind = parts.next()?;
    let rest = parts.next();
    let result = match (kind, rest) {
        ("ok", Some(n)) => Ok(usize::try_from(canonical(n)?).ok()?),
        ("quota", None) => Err(StorageError::QuotaExceeded),
        ("unavailable", None) => Err(StorageError::Unavailable),
        ("failed", None) => Err(StorageError::Failed),
        _ => return None,
    };
    Some((generation, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        for r in [
            Ok(0),
            Ok(7),
            Err(StorageError::Failed),
            Err(StorageError::Unavailable),
            Err(StorageError::QuotaExceeded),
        ] {
            let p = encode_delete_result(12, r);
            assert_eq!(decode_delete_result(&p), Some((12, r)));
        }
    }

    #[test]
    fn rejects_malformed() {
        for p in [
            "",
            "1",
            "1 ok",
            "01 ok 1",
            "1 ok 01",
            "1 failed 2",
            "1 x",
            "1 ok 1 2",
        ] {
            assert_eq!(decode_delete_result(p), None, "{p}");
        }
    }
}
