//! IR の値の上限（配列・連想配列の要素数と文字列の長さ。SAFE-25・D-45）の判定。
//!
//! 呼び出し元は [`crate::ir_load::load_ir`]（型の検査の入口）と、pack の `compile` の出力検査である。
//! 大きさ・入れ子の上限を通った `serde_json::Value` を 1 回走査し、契約（`contract::limits`）の
//! `MAX_COLLECTION_ELEMENTS`・`MAX_STRING_CHARS` を超えるものがあれば最初の 1 件の理由を返す。
//!
//! 規則: 配列は要素数、連想配列はキーの数で数え、ちょうど上限は通す。文字列（連想配列のキーを含む）は
//! コードポイント（`chars().count()`）で数える。バイト数が上限以下の文字列は数えずに通す
//! （コードポイント数はバイト数以下のため）。走査は明示のスタックで行い再帰しない（`json_depth` と同じ方針）。
//! 違反が複数あるときにどれを返すかは固定しない。エラーには測った値・位置・内容を含めない（SAFE-11・SAFE-40）。

use fandhe_workshop_contract::limits::{
    IrValueLimitReason, MAX_COLLECTION_ELEMENTS, MAX_STRING_CHARS,
};
use serde_json::Value;

fn string_too_long(s: &str) -> bool {
    s.len() > MAX_STRING_CHARS && s.chars().count() > MAX_STRING_CHARS
}

/// 値の木に上限を超える配列・連想配列・文字列が無いかを確かめる。
///
/// # Errors
///
/// 要素数の超過は `TooManyElements`、文字列の超過は `StringTooLong`。
pub fn check_value_limits(value: &Value) -> Result<(), IrValueLimitReason> {
    let mut stack = vec![value];
    while let Some(v) = stack.pop() {
        match v {
            Value::String(s) => {
                if string_too_long(s) {
                    return Err(IrValueLimitReason::StringTooLong);
                }
            }
            Value::Array(a) => {
                if a.len() > MAX_COLLECTION_ELEMENTS {
                    return Err(IrValueLimitReason::TooManyElements);
                }
                stack.extend(a.iter());
            }
            Value::Object(m) => {
                if m.len() > MAX_COLLECTION_ELEMENTS {
                    return Err(IrValueLimitReason::TooManyElements);
                }
                for (k, child) in m {
                    if string_too_long(k) {
                        return Err(IrValueLimitReason::StringTooLong);
                    }
                    stack.push(child);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn arr(n: usize) -> Value {
        Value::Array(vec![Value::Null; n])
    }

    #[test]
    fn arrays_boundary() {
        assert_eq!(check_value_limits(&arr(MAX_COLLECTION_ELEMENTS)), Ok(()));
        assert_eq!(
            check_value_limits(&arr(MAX_COLLECTION_ELEMENTS + 1)),
            Err(IrValueLimitReason::TooManyElements)
        );
        assert_eq!(
            check_value_limits(&json!({"a": [arr(MAX_COLLECTION_ELEMENTS * 10)]})),
            Err(IrValueLimitReason::TooManyElements)
        );
    }

    #[test]
    fn objects_boundary() {
        let obj =
            |n: usize| Value::Object((0..n).map(|i| (format!("k{i}"), Value::Null)).collect());
        assert_eq!(check_value_limits(&obj(MAX_COLLECTION_ELEMENTS)), Ok(()));
        assert_eq!(
            check_value_limits(&obj(MAX_COLLECTION_ELEMENTS + 1)),
            Err(IrValueLimitReason::TooManyElements)
        );
    }

    #[test]
    fn strings_count_code_points_not_bytes() {
        let s = |c: &str, n: usize| Value::String(c.repeat(n));
        assert_eq!(check_value_limits(&s("a", MAX_STRING_CHARS)), Ok(()));
        assert_eq!(check_value_limits(&s("あ", MAX_STRING_CHARS)), Ok(()));
        assert_eq!(
            check_value_limits(&s("あ", MAX_STRING_CHARS + 1)),
            Err(IrValueLimitReason::StringTooLong)
        );
        assert_eq!(
            check_value_limits(&s("a", MAX_STRING_CHARS * 10)),
            Err(IrValueLimitReason::StringTooLong)
        );
    }

    #[test]
    fn keys_count_as_strings() {
        let mut m = serde_json::Map::new();
        m.insert("a".repeat(MAX_STRING_CHARS), Value::Null);
        assert_eq!(check_value_limits(&Value::Object(m)), Ok(()));
        let mut m = serde_json::Map::new();
        m.insert("a".repeat(MAX_STRING_CHARS + 1), Value::Null);
        assert_eq!(
            check_value_limits(&Value::Object(m)),
            Err(IrValueLimitReason::StringTooLong)
        );
    }
}
