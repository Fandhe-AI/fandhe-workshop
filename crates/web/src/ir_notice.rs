//! 旧いエンジンが新しい教材を断るときの通知文（VER-13・TASK-181 2/2）。
//!
//! 呼び出し元は `host::EngineHost::view` で、教材（IR）を読めなかったホストが、画面の枠の通知欄（`pack-unloadable` 項目。
//! `shell_notice::pack_unloadable_item`。旧 `ir-notice` は廃止）に出すときに [`ir_notice_text`] を引く。コードは core の `IrLoadError::reason_code` が返す固定の ASCII で、
//! 文は固定の文言とそのコードだけで作り、教材・受講者の値を含めない（SAFE-11・SAFE-40）。
//! 上限（`too_large`・`too_deep`）だけは、契約の `rejection_message` の文面を引いて画面に出す（SAFE-17。TASK-154）。
//! 文言はエンジン側の仮置きである。通知の口の契約は TASK-511、断った画面の枠への載せ替えは TASK-545（SHELL-31）である。

use fandhe_workshop_contract::limits::{
    ir_value_rejection_message, rejection_message, IrValueLimitReason, LimitReason, LoadEntry,
};

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
    // 大きさ・深さの上限（SAFE-17）は、契約（TASK-199）が固定した受講者向けの文面をそのまま挟む。
    // 文面は契約から引くだけで、ここで新しい文言を作らない。測った値・教材の値は含まれない。
    let limit_message = LimitReason::ALL
        .iter()
        .find(|r| r.as_str() == code)
        .and_then(|r| rejection_message(LoadEntry::Ir, *r));
    if let Some(message) = limit_message {
        return format!("教材を読み込めませんでした。{message}（{code}）");
    }
    // 要素数・文字列長の上限（SAFE-25。TASK-162）も、契約の固定の文面を挟む。
    if let Some(r) = IrValueLimitReason::ALL.iter().find(|r| r.as_str() == code) {
        return format!(
            "教材を読み込めませんでした。{}（{code}）",
            ir_value_rejection_message(*r)
        );
    }
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
    fn limit_codes_carry_the_contract_message() {
        for (code, reason) in [
            ("too_large", LimitReason::TooLarge),
            ("too_deep", LimitReason::TooDeep),
        ] {
            let message = rejection_message(LoadEntry::Ir, reason).unwrap();
            let text = ir_notice_text(code);
            assert!(text.contains(message), "{text}");
            assert!(text.ends_with(&format!("（{code}）")), "{text}");
        }
        assert!(ir_notice_text("too_large").contains("ファイルが上限の大きさを超えています"));
    }

    #[test]
    fn value_limit_codes_carry_the_contract_message() {
        for r in IrValueLimitReason::ALL {
            let text = ir_notice_text(r.as_str());
            assert!(text.contains(ir_value_rejection_message(r)), "{text}");
            assert!(text.ends_with(&format!("（{}）", r.as_str())), "{text}");
        }
    }

    #[test]
    fn start_code_uses_start_wording() {
        assert_eq!(
            ir_notice_text("start_unknown"),
            "指定された開始点を開けませんでした。（start_unknown）"
        );
    }
}
