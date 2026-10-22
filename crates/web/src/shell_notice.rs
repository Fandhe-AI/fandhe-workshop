//! 枠の通知欄（`shell-notice`）の描画。「版の違いを変換して開いた」通知（SHELL-20。TASK-535 1/2）と、
//! 「開かない履歴」の理由の通知（SHELL-16・VER-8。TASK-176 2/2）を出す。
//!
//! 呼び出し元は `host::EngineHost::view` だけで、変換して開いたか（真偽）と開かない通知（理由のコードと閉じられるか）を
//! 履歴ファイルの読み込みと起動時の復元の判定結果から受け取る。判定そのものは core（`history_open`）が持ち、本モジュールは条件を複製しない。
//! 従う契約は `shell`（版 1。通知の種類・文面の識別子・操作の表・属性）と `a11y`（通知欄は 0 か 1 個で role は status）である。
//!
//! 通知欄の親は常に 1 個描き、通知が無いときは項目 0 件の空の keyed list にする。
//! 親の属性を切り替えず項目の出し入れだけで表すので、root は全置換にならない（FEND-3）。
//!
//! 管轄外: 文面の確定・配置・見た目（TASK-516・TASK-534・TASK-549）と、他の 4 種類の通知。
//! 文面（[`VERSION_CONVERTED_NOTICE`]）と root の先頭という置き場は仮置きである。
//!
//! 【推論】SHELL-20 の前提は内容の副版の違いだが、変換して開いたときは違った軸（内容・語彙・エンジン・参照元）に
//! 関わらず知らせる。黙って変換するより知らせる側に倒す。版の違いの中身は履歴由来の文字列を含むので、画面には出さない（SAFE-40）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::load_reason::LoadReason;

use crate::artifact_view::{ACTION_ATTR, PAYLOAD_ATTR};
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
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

/// 「開かない履歴」の通知の文の前半と後半（仮置き。最終の文面と次の行動は TASK-549・TASK-534・TASK-538 が持つ）。
/// 履歴由来の文字列は入れず、閉じた列挙の理由のコード（ASCII）だけを挟む（SAFE-40）。
const IMPORT_REFUSED_HEAD: &str =
    "この履歴は開けません。保存された履歴と画面の状態は変えていません。（";
const IMPORT_REFUSED_TAIL: &str = "）";

/// 「開かない履歴」の通知の文を作る。
pub fn import_refused_message(code: LoadReason) -> String {
    format!(
        "{IMPORT_REFUSED_HEAD}{}{IMPORT_REFUSED_TAIL}",
        code.as_str()
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

/// 通知欄を作る。`version_converted` が真なら「変換して開いた」通知を、`import_refused` があれば開かない通知を出す。
/// 並びは `NoticeKind::ALL` の順（開かない通知が先）。
///
/// 通知の値が契約に反して作れなければ（起きない想定）、項目を出さず panic もしない。
pub fn notice_area(version_converted: bool, import_refused: Option<RefusedNotice>) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
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
        let s = format!("{:?}", notice_area(false, None));
        assert!(!s.contains("shell-notice-item"), "{s}");
    }

    #[test]
    fn converted_area_has_one_item_and_dismiss() {
        let n = notice_area(true, None);
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(s.contains("shell-dismiss"));
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
        assert!(!VERSION_CONVERTED_NOTICE.is_empty());
    }

    #[test]
    fn refused_area_has_one_item_with_reason_and_dismiss() {
        let n = notice_area(false, refused(true));
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
        let s = format!("{:?}", notice_area(false, refused(false)));
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(!s.contains("shell-dismiss"), "{s}");
    }

    #[test]
    fn both_notices_show_two_items() {
        let n = notice_area(true, refused(true));
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 2, "{s}");
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
