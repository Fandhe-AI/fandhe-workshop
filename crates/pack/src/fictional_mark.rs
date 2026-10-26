//! 練習データの見出しに【架空】の印があるかの検査（`E-FICTIONAL-MARK`。AUTHOR-18・D-81。TASK-420 2/2）。
//!
//! 呼び出し元は [`crate::validate::validate`] だけで、`compile` は行わない（`test` の関門は `validate` を呼ぶので効く）。
//! 教材パックの練習データは public の repo に載るため、本物と見分けのつかない値を公開の前に作成者へ知らせる。
//! ここでは「印があるか」だけを見て、内容が本物かどうかは判定しない。
//!
//! 規則:
//! - 対象は読み込みに成功した練習データ（`LoadedPack::fixture_texts`）だけ。章の本文は受講者への説明であり対象にしない。
//! - 印は 1 行の中の `【架空` とその後ろの `】`（`【架空】`・`【架空ログ】` が通る。半角の `[架空]` は印としない）。
//! - 場所は最初の空でない行（先頭の BOM は除く）。拡張子が `.json` で内容が object のときは、ルート直下の文字列値の
//!   どれかにあればよい（JSON には見出しの行が無く、`_note` に印を置くため）。JSON として読めなければ先頭行の規則に倒す。
//! - 診断は練習データのパスごとに 1 件。同じパスを指す ID が複数あるときは最初の ID で報告する。
//!
//! 限界（`docs/known-gaps.md`）:
//! - `workshop.fixtures` に宣言されていないファイル・章の本文は調べない。
//! - 実在の private repo・社内 runner の名前との紛らわしさは調べない（D-81。公開前の確認の受け持ち）。
//! - 診断は固定の英文で、練習データの内容を写さない。ファイル・項目の欄は秘密情報の検査と同じ伏せ方にする。

use std::collections::BTreeSet;

use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::secret_check::{file_name, item_name};
use crate::source::LoadedPack;
use crate::validate::diag;

/// 1 行に印があるか。`【架空` の後ろに `】` がある形。
fn single_line_has_mark(l: &str) -> bool {
    l.find("【架空")
        .is_some_and(|i| l[i + "【架空".len()..].contains('】'))
}

/// 文字列のどれかの行の中で印がそろうか（JSON の文字列値は改行を含みうるため行に分ける。行をまたぐ印は認めない）。
fn line_has_mark(s: &str) -> bool {
    s.lines().any(single_line_has_mark)
}

/// 最初の空でない行に印があるか。
fn heading_has_mark(text: &str) -> bool {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.lines()
        .find(|l| !l.trim().is_empty())
        .is_some_and(line_has_mark)
}

fn has_mark(path: &str, text: &str) -> bool {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    if path.to_ascii_lowercase().ends_with(".json") {
        if let Ok(Value::Object(map)) = serde_json::from_str::<Value>(text) {
            return map.values().any(|v| v.as_str().is_some_and(line_has_mark));
        }
    }
    heading_has_mark(text)
}

/// 練習データの印の欠けを集める。診断の順序は ID の走査順で固定。
pub(crate) fn check_fictional_marks(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for (n, (id, text)) in pack.fixture_texts.iter().enumerate() {
        let Some(def) = pack.workshop.fixtures.get(id) else {
            continue;
        };
        let path = def.path.as_str();
        if !seen.insert(path.to_string()) || has_mark(path, text) {
            continue;
        }
        out.push(diag(
            &file_name(path),
            &item_name("fixture", id, n),
            DiagCode::FictionalMark,
            "practice data has no fictional mark in its heading; put the mark on the first non-blank line (see docs/pack-authoring.md)",
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_forms() {
        assert!(has_mark("a.md", "# 【架空】x\n"));
        assert!(has_mark("a.txt", "\n  \n【架空ログ】x"));
        assert!(has_mark("a.txt", "\u{feff}【架空】"));
        assert!(!has_mark("a.md", "# x\n【架空】\n"));
        assert!(!has_mark("a.md", "# [架空]\n"));
        assert!(!has_mark("a.md", "# 【架空\n"));
        assert!(has_mark("a.JSON", r#"{"_note":"【架空】","a":1}"#));
        assert!(!has_mark("a.json", r#"{"x":{"_note":"【架空】"}}"#));
        assert!(has_mark(
            "a.json",
            "\u{feff}{\n  \"_note\": \"【架空】\"\n}"
        ));
        assert!(!has_mark("a.json", "\u{feff}{\n  \"_note\": \"x\"\n}"));
        assert!(!has_mark("a.json", r#"{"_note":"【架空\n】"}"#));
        assert!(has_mark("a.json", r#"{"_note":"x\n【架空】"}"#));
        assert!(has_mark("a.json", "【架空】 not json"));
        assert!(!has_mark("a.json", "not json"));
    }
}
