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

/// 開始点の指定（`?start=`）を断るときの、コードの前に置く固定の文言（TASK-109 1/2。DLV-9）。値は文に入れない。
pub const START_NOTICE_BEFORE: &str = "指定された開始点を開けませんでした。（";

/// 通知文を作る。文言は決定に記録が無く、エンジンの側の仮置き。
///
/// コードが開始点の断り（`start_select::START_CODE_PREFIX`）なら開始点の文言、それ以外は教材の読み込みの文言にする。
pub fn ir_notice_text(code: &'static str) -> String {
    let before = if code.starts_with(crate::start_select::START_CODE_PREFIX) {
        START_NOTICE_BEFORE
    } else {
        IR_NOTICE_BEFORE
    };
    format!("{before}{code}{IR_NOTICE_AFTER}")
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

    #[test]
    fn start_code_uses_start_wording() {
        assert_eq!(
            ir_notice_text("start_unknown"),
            "指定された開始点を開けませんでした。（start_unknown）"
        );
    }
}
