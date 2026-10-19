//! 旧いエンジンが新しい教材を断るときの通知文（VER-13・TASK-181 2/2）。
//!
//! 呼び出し元は `host::EngineHost::view` で、教材（IR）を読めなかったホストが、画面に `ir-notice` の 1 要素だけを
//! 描くときに [`ir_notice_text`] を引く。コードは core の `IrLoadError::reason_code` が返す固定の ASCII で、
//! 文は固定の文言とそのコードだけで作り、教材・受講者の値を含めない（SAFE-11・SAFE-40）。
//! 文言はエンジン側の仮置きである。受講者向けの画面の枠は TASK-545（SHELL-31）、通知の口の契約は TASK-511 が決める。

/// 通知文で、コードの前に置く固定の文言。
pub const IR_NOTICE_BEFORE: &str = "教材を読み込めませんでした。（";

/// 通知文で、コードの後に置く固定の文言。
pub const IR_NOTICE_AFTER: &str = "）";

/// 通知文を作る。文言は決定に記録が無く、エンジンの側の仮置き。
pub fn ir_notice_text(code: &'static str) -> String {
    format!("{IR_NOTICE_BEFORE}{code}{IR_NOTICE_AFTER}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_fixed_wording_plus_code() {
        assert_eq!(
            ir_notice_text("format_newer"),
            "教材を読み込めませんでした。（format_newer）"
        );
    }
}
