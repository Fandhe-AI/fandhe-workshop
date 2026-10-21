//! 枠の通知欄（`shell-notice`）の描画。いまは「版の違いを変換して開いた」通知（SHELL-20。TASK-535 1/2）だけを出す。
//!
//! 呼び出し元は `host::EngineHost::view` だけで、変換して開いたか（履歴ファイルの読み込みと起動時の復元の判定結果）を
//! 真偽で受け取る。判定そのものは core（`history_open`）が持ち、本モジュールは条件を複製しない。
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

/// 閉じるボタンの表示名（仮置き）。
const DISMISS_LABEL: &str = "閉じる";

/// 通知欄を作る。`version_converted` が真なら「変換して開いた」通知を 1 件出す。
///
/// 通知の値が契約に反して作れなければ（起きない想定）、項目を出さず panic もしない。
pub fn notice_area(version_converted: bool) -> Node {
    let mut items: Vec<(String, Node)> = Vec::new();
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
    let owned = notice.attrs();
    let attrs: Vec<(&str, &str)> = owned.iter().map(|(k, v)| (*k, v.as_str())).collect();
    let mut children = vec![el(
        "p",
        vec![("data-testid", NOTICE_TEXT_TESTID)],
        vec![text(VERSION_CONVERTED_NOTICE)],
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
    Some((
        notice.kind().as_str().to_owned(),
        el("div", attrs, children),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_area_has_no_items() {
        let s = format!("{:?}", notice_area(false));
        assert!(!s.contains("shell-notice-item"), "{s}");
    }

    #[test]
    fn converted_area_has_one_item_and_dismiss() {
        let n = notice_area(true);
        let s = format!("{n:?}");
        assert_eq!(s.matches("shell-notice-item").count(), 1, "{s}");
        assert!(s.contains("shell-dismiss"));
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
        assert!(!VERSION_CONVERTED_NOTICE.is_empty());
    }
}
