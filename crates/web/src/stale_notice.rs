//! 古いタブの画面に出す「保存されていない」通知欄（`stale-notice`）の文（SAVE-13・SHELL-23・D-56・TASK-325 1/2）。
//!
//! 同じ教材を複数のタブで開くと、起動番号の照合（`boot_guard`）で追い越された古いタブは保存先へ書かない（SAVE-12）。
//! その書き込みは `Ok` で捨てられるため、受講者は保存されていないことに気づけない。決定 D-56（2026-10-12）は、合流の規則は足さず
//! 古いタブに「保存されていない」と分かる表示を出すと決めた。本モジュールはその文と束縛点の名前だけを持つ。
//! 文面は**仮置き**で、最終の文面は TASK-549、配置は TASK-534 が決める。変えるときはこのモジュールだけを直す。
//!
//! 呼び出し元は `engine_parts` の `LogPart`（描画の文を引く）で、真偽は `host` の `HostModel::stale_tab` が持つ。
//! 保存失敗の通知（`save-notice`）とは別の欄にする（あちらは保存に成功すると消えるが、こちらは消えない）。
//! 純関数だけを持ち、DOM・保存先に触れない。文は固定の定数だけで、教材・受講者・鍵・起動番号の値を含めない（A03）。

/// 古いタブの通知欄の束縛点の名前。
pub const STALE_NOTICE_FIELD: &str = "stale_notice";

/// 古いタブの通知の文（仮置き）。
pub const STALE_TAB_NOTICE: &str = "別のタブで同じ教材が開かれたため、このタブでの操作は今後保存されません。保存したい場合は、新しいほうのタブで続けてください。";

/// 通知欄の文。古いタブでなければ空文字列。
pub fn stale_notice_text(stale: bool) -> &'static str {
    if stale {
        STALE_TAB_NOTICE
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_unless_stale() {
        assert_eq!(stale_notice_text(false), "");
        assert_eq!(stale_notice_text(true), STALE_TAB_NOTICE);
        assert!(!STALE_TAB_NOTICE.is_empty());
    }
}
