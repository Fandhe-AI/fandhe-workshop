//! 枠の履歴の書き出し・読み込みの操作の描画（TASK-531 1/2。SHELL-15）。
//!
//! 呼び出し元は `host::EngineHost::view` だけで、root の直下の通知欄の後ろ・`tablist` の前に [`history_ops`] を置く（仮置き。
//! 最終の配置は TASK-534・TASK-548）。tabpanel の外なので、幅 960px 未満でもどちらのタブでも見える。
//! 識別子・action の名前・宛先の形は `shell`（版 1）が固定済みで、本モジュールは従う側である。
//!
//! フレームワークの属性の検査（`is_safe_url`）は `blob:` の `href` を落とすため、リンクの URL は `data-export-url`（[`EXPORT_URL_ATTR`]）に
//! 載せ、`static/app.js` が `a.href` へ写す（`setAttribute` は使わない）。写すのは `blob:<自分のオリジン>/` で始まる値だけである。
//! 書き出しは 2 段である。「履歴を書き出す」ボタン（`shell-export`）を押すとホストが `export_live` の関門を通し、出せるときだけ
//! `blob:` URL のリンク（`export-download`）が現れる。受講者がそのリンクを押してダウンロードする。プログラムからの
//! `.click()` と属性の書き込みは `tests/no_hidden_triggers.rs` が禁じているため、押下を受講者の操作に残す。
//! 出せないとき（秘密情報の警告の間・履歴が無い）は、リンクの代わりに固定の短い文（`export-status`）を出す。
//! リンクと文は同じ keyed list（[`EXPORT_RESULT_FIELD`]。項目は 0 か 1 件）に置き、履歴が変わるとホストが外す。
//!
//! 読み込みは、`shell-import` の入れ物（`data-action="import-history"`。契約の要求）の中に `label` と `input[type=file]` を置く。
//! 入れ物は `display: contents`（`static/app.css`）で箱を持たず、クリックの起点にならない。label・input は上流の
//! 「対話要素の境界」で止まるため、ファイル画面を開くクリックでは `import-history` が届かない（空の本文と区別できないため。
//! 届くと何もしていないのに開かない通知が出る）。ファイルの本文は `static/app.js` が読んで `import_history`（wasm 書き出し）で
//! 届け、Rust は DOM の値を読まない（BUILD-6）。`data-payload` は置かない（`shell` の `import-payload-in-dom`）。

use fandhe_frontend_core::{el, text, Node};

use crate::artifact_view::ACTION_ATTR;
use crate::list_policy::keyed_view_or_notice;
use crate::parts::{FieldKind, PartField};
use crate::shell::{ShellOp, ACTION_EXPORT_HISTORY, ACTION_IMPORT_HISTORY};

/// 書き出しの結果（リンクか文。0 か 1 件）の keyed list の field。変わると `host::EngineHost` がこの名前を dirty に積む。
pub const EXPORT_RESULT_FIELD: PartField = PartField {
    name: "export_result",
    kind: FieldKind::KeyedList,
};

/// 操作の入れ物の識別子。
pub const HISTORY_OPS_TESTID: &str = "history-ops";
/// 書き出しのリンクの識別子。
pub const EXPORT_DOWNLOAD_TESTID: &str = "export-download";
/// 書き出せないときの文の識別子。
pub const EXPORT_STATUS_TESTID: &str = "export-status";

/// 書き出しのボタンの文字。
pub const EXPORT_LABEL: &str = "履歴を書き出す";
/// 読み込みの label の文字。
pub const IMPORT_LABEL: &str = "履歴を読み込む";
/// 読み込みで選べるファイルの種類（`input` の `accept`）。
const IMPORT_ACCEPT: &str = "application/json,.json";

/// リンクの URL を載せる属性。`href` には直接書けない（上流が `blob:` を落とす）ため、`app.js` が `href` へ写す。
pub const EXPORT_URL_ATTR: &str = "data-export-url";

/// 書き出しの結果 1 件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportResult<'a> {
    /// 受講者が押すリンク。URL はホストが作った `blob:` URL（`data-export-url` に載せる）、`file_name` は `history_export::file_name` の値。
    Link {
        /// `blob:` URL。
        href: &'a str,
        /// 保存するファイルの名前。
        file_name: &'a str,
    },
    /// 出せない理由の固定の文。
    Status(&'static str),
}

/// 枠の書き出し・読み込みの操作。`result` は書き出しの結果（無ければ項目 0 件）。
#[must_use]
pub fn history_ops(result: Option<&ExportResult<'_>>) -> Node {
    let items: Vec<(String, Node)> = result
        .map(|r| match r {
            ExportResult::Link { href, file_name } => (
                (*href).to_owned(),
                el(
                    "a",
                    vec![
                        ("data-testid", EXPORT_DOWNLOAD_TESTID),
                        (EXPORT_URL_ATTR, href),
                        ("download", file_name),
                        ("class", "sh-history-link"),
                    ],
                    vec![text(crate::history_export::LINK_LABEL)],
                ),
            ),
            ExportResult::Status(message) => (
                "status".to_owned(),
                el(
                    "p",
                    vec![("data-testid", EXPORT_STATUS_TESTID)],
                    vec![text(*message)],
                ),
            ),
        })
        .into_iter()
        .collect();
    let export = el(
        "button",
        vec![
            ("type", "button"),
            ("data-testid", ShellOp::Export.testid()),
            (ACTION_ATTR, ACTION_EXPORT_HISTORY),
        ],
        vec![text(EXPORT_LABEL)],
    );
    let result_list = keyed_view_or_notice(
        "div",
        vec![("class", "sh-history-result")],
        &EXPORT_RESULT_FIELD,
        items,
    );
    let import = el(
        "div",
        vec![
            ("data-testid", ShellOp::Import.testid()),
            (ACTION_ATTR, ACTION_IMPORT_HISTORY),
            ("class", "sh-import"),
        ],
        vec![el(
            "label",
            vec![("class", "sh-import-label")],
            vec![
                text(IMPORT_LABEL),
                el(
                    "input",
                    vec![("type", "file"), ("accept", IMPORT_ACCEPT)],
                    vec![],
                ),
            ],
        )],
    );
    el(
        "div",
        vec![("data-testid", HISTORY_OPS_TESTID), ("class", "sh-history")],
        vec![export, result_list, import],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dump(n: &Node) -> String {
        format!("{n:?}")
    }

    #[test]
    fn 結果なしは項目を持たず操作は1つずつ() {
        let n = history_ops(None);
        let s = dump(&n);
        assert_eq!(s.matches("shell-export").count(), 1, "{s}");
        assert_eq!(s.matches("shell-import").count(), 1, "{s}");
        assert!(!s.contains(EXPORT_DOWNLOAD_TESTID), "{s}");
        assert!(!s.contains(EXPORT_STATUS_TESTID), "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn リンクは_url_と_download_を持ち_href_は直接書かない() {
        let r = ExportResult::Link {
            href: "blob:x/1",
            file_name: "f.json",
        };
        let n = history_ops(Some(&r));
        let s = dump(&n);
        assert!(s.contains("blob:x/1") && s.contains("f.json"), "{s}");
        assert!(s.contains(EXPORT_URL_ATTR), "{s}");
        assert!(!s.contains("\"href\""), "{s}");
        assert_eq!(s.matches(EXPORT_DOWNLOAD_TESTID).count(), 1, "{s}");
        assert!(crate::shell::check_no_undefined(&n).is_ok());
        assert!(crate::a11y::check_no_undefined(&n).is_ok());
    }

    #[test]
    fn 文は固定の文を出す() {
        let r = ExportResult::Status(crate::history_export::EMPTY_STATUS);
        let s = dump(&history_ops(Some(&r)));
        assert!(s.contains(crate::history_export::EMPTY_STATUS), "{s}");
        assert!(!s.contains(EXPORT_DOWNLOAD_TESTID), "{s}");
    }

    #[test]
    fn 読み込みの入れ物は_payload_を持たない() {
        let s = dump(&history_ops(None));
        assert_eq!(s.matches(&format!("data-{}", "payload")).count(), 0, "{s}");
        assert_eq!(s.matches("import-history").count(), 1, "{s}");
    }
}
