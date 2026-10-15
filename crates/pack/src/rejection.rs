//! 拒否の分類の語を、contract の `Category`（8 語）で読んで検査する（TASK-45 1/2、PACK-11）。
//!
//! `compile` から 1 回だけ呼ばれる。分類の語が現れる 3 か所を対象にする。
//! - `workshop.*` の `rejections.<id>.category`
//! - `workshop.*` の `defaults` のキー
//! - 章・モジュールの Step の `on_reject` のキー
//!
//! 型の正は contract の `judgment`（TASK-256）で、本モジュールは [`crate::vocab::rejection_category`] を通して
//! 読むだけである。語彙にない語・欠け・型の違いは構文・語彙の違反なので `E-PARSE` にする（`input.rs` の未知の `kind` と同じ）。
//! `free_text_unjudged` は 3 か所とも受け付ける（拒否かどうかの区別は実行時に `RejectCategory` が持つ）。
//! `on_reject` のキーは引数の置換の対象外なので、展開前の宣言を 1 回だけ検査すれば足りる。
//!
//! 扱わないこと（`docs/diagnostics.md` 版 1 の `E-STRUCTURE` の条件に無いため）: `defaults` が 8 分類を網羅すること、
//! `defaults`・`on_reject` のキーと参照先の文面の `category` の一致、`next_action` の空（TASK-46）、`validate` への組み込み。
//! メッセージに写すのは `safe_ident` を通った識別子だけで、値そのものは写さない。

use serde_json::Value;

use crate::diag::{DiagCode, Diagnostic};
use crate::input::diag as step_diag;
use crate::source::{safe_ident, LoadedPack};
use crate::vocab::rejection_category;

fn pack_diag(file: &str, item: &str, message: String) -> Diagnostic {
    let file = if file.trim().is_empty() { "." } else { file };
    // file と item を空でない値に直したので、Diagnostic::new は失敗しない。
    Diagnostic::new(file, item, DiagCode::Parse, message).expect("file and item are non-empty")
}

/// 語を読めないときのメッセージ（`<場所>: unknown category <語>`）。語は安全な識別子のときだけ写す。
fn unknown(label: &str, term: &str) -> String {
    if safe_ident(term) {
        format!("{label}: unknown category {term}")
    } else {
        format!("{label}: unknown category")
    }
}

/// 文面の `category` を読み、問題があればメッセージを返す。
fn rejection_problem(value: &Value) -> Option<String> {
    let Some(map) = value.as_object() else {
        return Some("rejection: must be a mapping".to_string());
    };
    match map.get("category") {
        None => Some("rejection: category is missing".to_string()),
        Some(Value::String(c)) => rejection_category(c)
            .is_none()
            .then(|| unknown("rejection", c)),
        Some(_) => Some("rejection: category must be a string".to_string()),
    }
}

/// `rejections`・`defaults`・`on_reject` の分類の語を検査し、診断を集める。順序は決定的（キー順・スコープ順・Step 順）。
pub(crate) fn check_categories(pack: &LoadedPack) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    let file = pack.workshop_file.as_str();
    for (id, value) in &pack.workshop.rejections {
        if let Some(msg) = rejection_problem(value) {
            let item = if safe_ident(id) {
                format!("rejection:{id}")
            } else {
                "rejection".to_string()
            };
            out.push(pack_diag(file, &item, msg));
        }
    }
    for key in pack.workshop.defaults.keys() {
        if rejection_category(key).is_none() {
            let item = if safe_ident(key) {
                format!("defaults:{key}")
            } else {
                "defaults".to_string()
            };
            out.push(pack_diag(file, &item, unknown("defaults", key)));
        }
    }
    for scope in &pack.scopes {
        for step in &scope.src.steps {
            for key in step.on_reject.keys() {
                if rejection_category(key).is_none() {
                    out.push(step_diag(
                        scope,
                        &step.id,
                        DiagCode::Parse,
                        unknown("on_reject", key),
                    ));
                }
            }
        }
    }
    out
}
