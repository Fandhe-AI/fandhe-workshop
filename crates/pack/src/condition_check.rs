//! 条件（`next[].when`・`rules[].when`・`starts[].requires`・効果の項目の `when`）の読み込み時の検査（TASK-42）。
//!
//! `compile` が章の開始点（`compile`）と、展開後の Step・モジュールの開始点（`expand`）から呼ぶ。
//! 効果の項目の `when` は `effect_check` が呼ぶ。IR の欄は `Value` のまま通すため（版 1 の契約）、ここで
//! contract の [`Condition::from_value`] を通して語彙を検査し、誤りを診断の種類と文面にして返す。
//! 診断の組み立て（`file`・`item`）は呼び出し側の責務で、本モジュールは `(コード, 文面)` だけを返す。
//!
//! 形の誤り（未知の欄・型の不一致・式やスクリプトの欄）は `E-PARSE`、語の数・空の名前・入れ子の深さは
//! `E-STRUCTURE`。文面には欄の経路と、短い識別子の欄名だけを入れ、条件の値は入れない。

use fandhe_workshop_contract::condition::{Condition, ConditionParseError};
use serde_json::Value;

use crate::diag::DiagCode;

/// 条件 `v`（欄の経路 `field`）を検査し、誤りごとに `(コード, 文面)` を返す。
///
/// 文面は `field: ...` の形。`null` は呼び出し側が「条件なし」として飛ばす（欄ごとに意味が違うため）。
/// 誤りの文面は欄の経路と短い識別子の欄名だけで、条件の値（`use` の引数の置換で入りうる値を含む）は
/// 入れない。
pub(crate) fn check_condition(v: &Value, field: &str) -> Vec<(DiagCode, String)> {
    match Condition::from_value(v) {
        Ok(_) => Vec::new(),
        Err(e) => {
            let code = match e {
                ConditionParseError::Shape { .. } => DiagCode::Parse,
                _ => DiagCode::Structure,
            };
            vec![(code, format!("{field}: {e}"))]
        }
    }
}

/// 省略可能な条件の欄を検査する。`None` と `null` は条件なしとして飛ばす。
pub(crate) fn check_optional(v: Option<&Value>, field: &str) -> Vec<(DiagCode, String)> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(v) => check_condition(v, field),
    }
}
