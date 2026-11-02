//! 表示だけで開いた履歴の理由を出す通知欄（`view-only-notice`）の文（VER-8・TASK-176 1/2）。
//!
//! 版の違う履歴の判定（TASK-174）で「表示だけ」になった履歴は、操作しても状態・ログ・保存が変わらない。
//! 何も起きない理由が見えないと故障に見えるため、`host` が持つ理由のコード（`LoadReason`）から文を作って出す。
//! 文面は**仮置き**で、最終の文面は TASK-549、配置は TASK-534 が決める。変えるときはこのモジュールだけを直す。
//! 枠の通知（`shell` の `NoticeKind`、契約は版 1）は spec が先に変わらないと足せないため、`stale-notice` と同じ任意の欄にする。
//!
//! `format_older`（履歴の `ir_format` が開く教材より古い）だけは、変換しないことを伝える理由の文を持つ（VER-16・TASK-184 2/2）。
//! 他の理由は文を持たず、最終の文面は TASK-549、配置は TASK-534 が決める。
//!
//! 呼び出し元は `engine_parts` の `LogPart`（描画の文を引く）で、理由は `host` の `HostModel::view_only_reason` が持つ。
//! 純関数だけを持ち、DOM・保存先に触れない。文は固定の文言と閉じた列挙の ASCII コードだけで作り、
//! 履歴・教材・受講者の値を含めない（SAFE-40・A03）。

use fandhe_workshop_contract::load_reason::LoadReason;

/// 表示だけの通知欄の束縛点の名前。
pub const VIEW_ONLY_NOTICE_FIELD: &str = "view_only_notice";

/// 表示だけの理由ごとの文（仮置き）。文を持つのは `format_older` だけで、残りは `None`。
///
/// 理由が足されたときにコンパイルで止めるため、既定の腕を置かない（`shell_notice::corrupt_reason_text` と同じ形）。
/// 固定の文言だけで、履歴・教材・受講者の値を含めない（SAFE-40）。
pub fn view_only_reason_text(code: LoadReason) -> Option<&'static str> {
    match code {
        LoadReason::FormatOlder => Some(
            "この履歴は、今の教材より古い教材の形式で記録されています。古い形式の履歴は変換しないため、続きからは進められません。",
        ),
        LoadReason::ReplayMismatch
        | LoadReason::Truncated
        | LoadReason::NotJson
        | LoadReason::MissingVersion
        | LoadReason::OtherPack
        | LoadReason::Malformed
        | LoadReason::TooLarge
        | LoadReason::TooDeep
        | LoadReason::TooManyOps
        | LoadReason::HistoryFormatNewer
        | LoadReason::BadVersion
        | LoadReason::FormatNewer
        | LoadReason::VocabNewer
        | LoadReason::EngineNewer
        | LoadReason::EngineMajor
        | LoadReason::ContentMajor
        | LoadReason::StartMissing
        | LoadReason::ReplayDiverged => None,
    }
}

/// 通知欄の文（仮置き）。表示だけでなければ空文字列。
pub fn view_only_notice_text(reason: Option<LoadReason>) -> String {
    match reason {
        Some(r) => format!(
            "この履歴は表示だけで開いています。操作しても状態・ログ・保存は変わりません。（{}）{}",
            r.as_str(),
            view_only_reason_text(r).unwrap_or("")
        ),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_unless_view_only() {
        assert_eq!(view_only_notice_text(None), "");
    }

    #[test]
    fn carries_the_fixed_reason_code() {
        let t = view_only_notice_text(Some(LoadReason::ContentMajor));
        assert!(t.contains("content_major"));
        assert!(!t.is_empty());
    }

    #[test]
    fn format_older_adds_a_fixed_reason_after_the_code() {
        let t = view_only_notice_text(Some(LoadReason::FormatOlder));
        let reason = view_only_reason_text(LoadReason::FormatOlder).unwrap();
        assert!(t.find("format_older").unwrap() < t.find(reason).unwrap());
        assert!(t.ends_with(reason));
        assert!(!reason.contains(['{', '"', '<']));
    }

    #[test]
    fn other_reasons_keep_the_plain_form() {
        for r in [
            LoadReason::ContentMajor,
            LoadReason::EngineMajor,
            LoadReason::ReplayDiverged,
        ] {
            assert_eq!(view_only_reason_text(r), None);
            let t = view_only_notice_text(Some(r));
            assert!(t.contains(r.as_str()));
            assert!(t.ends_with('）'));
        }
    }
}
