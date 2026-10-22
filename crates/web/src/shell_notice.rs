//! 枠の通知欄（`shell-notice`）の描画。「保存できなかった」通知（SHELL-18・SAVE-10。TASK-534 1/2）と、
//! 「版の違いを変換して開いた」通知（SHELL-20。TASK-535 1/2）と、
//! 「開かない履歴」の理由の通知（SHELL-16・VER-8。TASK-176 2/2）を出す。
//!
//! 呼び出し元は `host::EngineHost::view` だけで、変換して開いたか（真偽）と開かない通知（理由のコードと閉じられるか）を
//! 履歴ファイルの読み込みと起動時の復元の判定結果から、保存の失敗の種類を `HostModel::save_failure` から受け取る。判定そのものは core（`history_open`）が持ち、本モジュールは条件を複製しない。
//! 従う契約は `shell`（版 2。通知の種類・文面の識別子・操作の表・属性）と `a11y`（通知欄は 0 か 1 個で role は status）である。
//!
//! 通知欄の親は常に 1 個描き、通知が無いときは項目 0 件の空の keyed list にする。
//! 親の属性を切り替えず項目の出し入れだけで表すので、root は全置換にならない（FEND-3）。
//!
//! 保存の失敗は閉じる操作を置かない（成功すると消える）。文は `save_notice` が持つ。
//! 管轄外: 文面の確定・見た目（TASK-516・TASK-549）と、他の 3 種類の通知。
//! 開かない通知には理由ごとの次の行動の文を添える（TASK-334。割り当ては案で contract の `load_next_action`）。
//! 文面（[`VERSION_CONVERTED_NOTICE`]）と root の先頭という置き場は仮置きである。
//!
//! 【推論】SHELL-20 の前提は内容の副版の違いだが、変換して開いたときは違った軸（内容・語彙・エンジン・参照元）に
//! 関わらず知らせる。黙って変換するより知らせる側に倒す。版の違いの中身は履歴由来の文字列を含むので、画面には出さない（SAFE-40）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::load_next_action::{next_action, LoadNextAction};
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_contract::storage::StorageError;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::save_notice::{save_failure_reason, save_notice_text};
use crate::shell::{
    MessageId, Notice, NoticeKind, ShellOp, NOTICE_AREA_TESTID, NOTICE_TEXT_TESTID,
};

/// 通知欄の keyed list の field。表示が変わると `host::EngineHost` がこの名前を dirty に積む。
pub const SHELL_NOTICES_FIELD: PartField = PartField {
    name: "shell_notices",
    kind: FieldKind::KeyedList,
};

/// 「変換して開いた」通知の文（仮置き。最終の文面は TASK-549・TASK-534 が持つ）。
pub const VERSION_CONVERTED_NOTICE: &str =
    "変換して開きました。教材の内容または版が、この履歴を書いたときから変わっています。";

/// 「開かない履歴」の通知の文の前半と後半（仮置き。最終の文面は TASK-549・TASK-534 が持つ）。
/// 末尾に、理由ごとの次の行動の文（[`next_action_text`]）を付ける。割り当ては contract の `load_next_action`（TASK-334。案）、
/// 押せる操作にするのは TASK-538 である。
/// 履歴由来の文字列は入れず、閉じた列挙の理由のコード（ASCII）だけを挟む（SAFE-40）。
const IMPORT_REFUSED_HEAD: &str =
    "この履歴は開けません。保存された履歴と画面の状態は変えていません。（";
const IMPORT_REFUSED_TAIL: &str = "）";

/// 次の行動の文（仮置き。固定の文言だけで、履歴由来の値を含めない）。
pub fn next_action_text(a: LoadNextAction) -> &'static str {
    match a {
        LoadNextAction::ChooseAnotherFile => "別の履歴ファイルを選んで読み込んでください。",
        LoadNextAction::OpenWithSourceVersion => {
            "この履歴を書き出したときの版の教材で開いてください。"
        }
        LoadNextAction::StartFresh => "この履歴は使わずに、新しく始めてください。",
    }
}

/// 「開かない履歴」の通知の文を作る。理由のコードの後ろに、理由に対応する次の行動の文を付ける。
pub fn import_refused_message(code: LoadReason) -> String {
    format!(
        "{IMPORT_REFUSED_HEAD}{}{IMPORT_REFUSED_TAIL}{}",
        code.as_str(),
        next_action_text(next_action(code))
    )
}

/// 開かない通知の内容。`dismissable` が偽のものは閉じる操作を置かない（起動時に開けず保存が止まった印を隠さない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusedNotice {
    /// 理由のコード（閉じた列挙。履歴由来の文字列ではない）。
    pub code: LoadReason,
    /// 閉じる操作を置くか。
    pub dismissable: bool,
}

/// 閉じるボタンの表示名（仮置き）。
const DISMISS_LABEL: &str = "閉じる";

/// 通知欄を作る。`save_failure` があれば保存の失敗の通知を、`version_converted` が真なら「変換して開いた」通知を、
/// `import_refused` があれば開かない通知を出す。並びは `NoticeKind::ALL` の順（保存の失敗が先頭）。
///
/// 通知の値が契約に反して作れなければ（起きない想定）、項目を出さず panic もしない。
pub fn notice_area(
    version_converted: bool,
    import_refused: Option<RefusedNotice>,
    save_failure: Option<StorageError>,
) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
    if let Some(failure) = save_failure {
        if let Some(item) = save_failed_item(failure) {
            items.push(item);
        }
    }
    if let Some(refused) = import_refused {
        if let Some(item) = import_refused_item(refused) {
            items.push(item);
        }
    }
    if version_converted {
        if let Some(item) = version_converted_item() {
            items.push(item);
        }
    }
    keyed_view_or_notice(
        "div",
        vec![("data-testid", NOTICE_AREA_TESTID), ("role", "status")],
        &SHELL_NOTICES_FIELD,
        items,
    )
}

/// 「保存できなかった」通知の項目。理由は閉じた列挙から写した固定の ASCII で、押せる操作は置かない。
fn save_failed_item(failure: StorageError) -> Option<(String, Node)> {
    let notice = Notice::new(
        NoticeKind::SaveFailed,
        Some(save_failure_reason(failure)),
        MessageId::SaveFailed,
        &[],
    )
    .ok()?;
    Some(item_of(&notice, save_notice_text(Some(failure))))
}

/// 「変換して開いた」通知の項目（キーは種類の識別子）。
fn version_converted_item() -> Option<(String, Node)> {
    let notice = Notice::new(
        NoticeKind::VersionConverted,
        None,
        MessageId::VersionConverted,
        &[ShellOp::DismissNotice],
    )
    .ok()?;
    Some(item_of(&notice, VERSION_CONVERTED_NOTICE))
}

/// 「開かない履歴」の通知の項目。理由のコードは契約の形（`ReasonCode::parse`）を通ったものだけを属性に載せる。
fn import_refused_item(refused: RefusedNotice) -> Option<(String, Node)> {
    let ops: &[ShellOp] = if refused.dismissable {
        &[ShellOp::DismissNotice]
    } else {
        &[]
    };
    let notice = Notice::new(
        NoticeKind::ImportRefused,
        Some(refused.code.as_str()),
        MessageId::ImportRefused,
        ops,
    )
    .ok()?;
    Some(item_of(&notice, &import_refused_message(refused.code)))
}

/// 通知 1 件の DOM。文は固定の文言（と閉じた列挙のコード）だけで、`text()` で描く。
fn item_of(notice: &Notice, message: &str) -> (String, Node) {
    let owned = notice.attrs();
    let attrs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mut children = vec![el(
        "p",
        vec![("data-testid", NOTICE_TEXT_TESTID)],
        vec![text(message)],
    )];
    for (op, action) in notice.ops().iter().zip(notice.actions()) {
        let (name, payload) = action.encode();
        children.push(el(
            "button",
            vec![
                ("type", "button"),
                ("data-testid", op.testid()),
                (ACTION_ATTR, name),
                (PAYLOAD_ATTR, payload.as_str()),
            ],
            vec![text(DISMISS_LABEL)],
        ));
    }
    (
        notice.kind().as_str().to_owned(),
        el("div", attrs, children),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(dismissable: bool) -> Option<RefusedNotice> {
        Some(RefusedNotice {
            code: LoadReason::FormatNewer,
            dismissable,
        })
    }

    #[test]
    fn empty_area_has_no_items() {
        let s = format!("{:?}", notice_area(false, None, None));
        assert!(!s.contains("shell-notice-item"), "{s}");
    }

    #[test]
    fn converted_area_has_one_item_and_dismiss() {
        let n = notice_area(true, None, None);
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(s.contains("shell-dismiss"));
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
        assert!(!VERSION_CONVERTED_NOTICE.is_empty());
    }

    #[test]
    fn refused_area_has_one_item_with_reason_and_dismiss() {
        let n = notice_area(false, refused(true), None);
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(s.contains("import-refused"), "{s}");
        assert!(s.contains("format_newer"), "{s}");
        assert!(s.contains("shell-dismiss"), "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn refused_without_dismiss_has_no_button() {
        let s = format!("{:?}", notice_area(false, refused(false), None));
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(!s.contains("shell-dismiss"), "{s}");
    }

    #[test]
    fn both_notices_show_two_items() {
        let n = notice_area(true, refused(true), None);
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 2, "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn message_has_code_and_next_action_for_every_reason() {
        for code in LoadReason::ALL {
            let m = import_refused_message(code);
            assert!(m.contains(code.as_str()), "{}", code.as_str());
            assert!(
                m.ends_with(next_action_text(next_action(code))),
                "{}",
                code.as_str()
            );
        }
    }

    #[test]
    fn next_action_texts_are_distinct_and_non_empty() {
        let t: Vec<&str> = LoadNextAction::ALL
            .into_iter()
            .map(next_action_text)
            .collect();
        assert!(t.iter().all(|x| !x.is_empty()));
        assert!(t[0] != t[1] && t[1] != t[2] && t[0] != t[2]);
    }

    #[test]
    fn every_save_failure_makes_one_item_without_dismiss() {
        for f in [
            StorageError::QuotaExceeded,
            StorageError::Unavailable,
            StorageError::Failed,
        ] {
            let n = notice_area(false, None, Some(f));
            let s = format!("{n:?}");
            assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
            assert!(s.contains("save-failed"), "{s}");
            assert!(s.contains(save_failure_reason(f)), "{s}");
            assert!(s.contains(save_notice_text(Some(f))), "{s}");
            assert!(!s.contains("shell-dismiss"), "{s}");
            assert!(crate::shell::check_no_undefined(&n).is_ok());
            assert!(crate::a11y::check_no_undefined(&n).is_ok());
        }
    }

    #[test]
    fn all_three_notices_show_three_items_with_save_failed_first() {
        let n = notice_area(true, refused(true), Some(StorageError::QuotaExceeded));
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 3, "{s}");
        assert!(s.find("save-failed") < s.find("import-refused"), "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn every_reason_makes_an_item() {
        for code in LoadReason::ALL {
            let item = import_refused_item(RefusedNotice {
                code,
                dismissable: true,
            });
            assert!(item.is_some(), "{}", code.as_str());
        }
    }
}
