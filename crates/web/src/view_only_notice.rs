//! 表示だけで開いた履歴の理由を出す通知欄（`view-only-notice`）の文（VER-8・TASK-176 1/2）。
//!
//! 版の違う履歴の判定（TASK-174）で「表示だけ」になった履歴は、操作しても状態・ログ・保存が変わらない。
//! 何も起きない理由が見えないと故障に見えるため、`host` が持つ理由のコード（`LoadReason`）から文を作って出す。
//! 文面は**仮置き**で、最終の文面は TASK-549、配置は TASK-534 が決める。変えるときはこのモジュールだけを直す。
//! 枠の通知（`shell` の `NoticeKind`、契約は版 1）は spec が先に変わらないと足せないため、`stale-notice` と同じ任意の欄にする。
//!
//! 呼び出し元は `engine_parts` の `LogPart`（描画の文を引く）で、理由は `host` の `HostModel::view_only_reason` が持つ。
//! 純関数だけを持ち、DOM・保存先に触れない。文は固定の文言と閉じた列挙の ASCII コードだけで作り、
//! 履歴・教材・受講者の値を含めない（SAFE-40・A03）。

use fandhe_workshop_contract::load_reason::LoadReason;

/// 表示だけの通知欄の束縛点の名前。
pub const VIEW_ONLY_NOTICE_FIELD: &str = "view_only_notice";

/// 通知欄の文（仮置き）。表示だけでなければ空文字列。
pub fn view_only_notice_text(reason: Option<LoadReason>) -> String {
    match reason {
        Some(r) => format!(
            "この履歴は表示だけで開いています。操作しても状態・ログ・保存は変わりません。（{}）",
            r.as_str()
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
}
