//! モジュール・章の引数の型と範囲外の値の検査（TASK-71 2/3。PACK-51・MOD-17 (c)・D-43）。
//!
//! 呼び出し元は `compile`・`validate`（宣言の検査 [`check_param_types`]）と、モジュールの展開
//! `expand::expand_use`（呼び出しの引数の検査 [`arg_message`]）である。引数は文字列のまま IR に載り
//! （`Instance.args`）、`condition.param` は文字列の完全一致で比べる。綴り違い（`"flase"`）や範囲外の値は
//! 黙って別の分岐に倒れるため、作成者が `param_types` で宣言した型と範囲に合わない値を、実体を作る前に拒否する。
//!
//! - `param_types` は `params` の隣の任意の欄で、鍵は `params` の名前。省略した引数は従来どおり検査しない
//! - 型は `bool`（`true`・`false`）・`int`（10 進の正準形。`min`・`max` は両端を含む）・`enum`（`values` のどれか）
//! - 既定値（`params`）も、渡す引数も、同じ [`value_fits`] で判定する
//! - `param_types` は検査のための宣言であり、IR には載せない（使わない教材と IR は変わらない）
//!
//! 診断は固定の英語で、値・`values` の中身を写さない。名前は `safe_ident` を通るものだけを写す。
//! 教材データは信頼しない入力として扱い、整数の溢れは不合格にする（panic しない）。

use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;

use crate::diag::{DiagCode, Diagnostic};
use crate::source::{safe_ident, LoadedScope};
use crate::validate::diag;

/// 呼び出しの引数が宣言した型・範囲に合わないときの診断の文面の接頭。`validate` が展開の診断を選ぶのに使う。
pub(crate) const ARG_OUT_OF_RANGE: &str = "argument value is outside the declared type or range";

/// 型の閉じた集合。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ParamKind {
    /// `"true"` か `"false"`。
    Bool,
    /// 10 進の整数（正準形）。
    Int,
    /// `values` のどれか。
    Enum,
}

/// 引数 1 つの型の宣言（`param_types.<名前>`）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamTypeSrc {
    /// 型。
    #[serde(rename = "type")]
    pub kind: ParamKind,
    /// `enum` の取りうる値。
    #[serde(default)]
    pub values: Option<Vec<String>>,
    /// `int` の下限（含む）。
    #[serde(default)]
    pub min: Option<i64>,
    /// `int` の上限（含む）。
    #[serde(default)]
    pub max: Option<i64>,
}

/// `int` の正準形（`-?(0|[1-9][0-9]*)`。`-0` を除く）なら値を返す。`condition.param` が文字列の完全一致で
/// 比べるので、`"01"`・`"+1"` のような同じ数の別表記は通さない。溢れは `None`（不合格）にする。
fn canonical_int(v: &str) -> Option<i64> {
    let digits = v.strip_prefix('-').unwrap_or(v);
    let first = digits.chars().next()?;
    if !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    if first == '0' && (digits.len() > 1 || v.starts_with('-')) {
        return None;
    }
    v.parse::<i64>().ok()
}

/// 値 `v` が宣言 `decl` の型と範囲に合うか。
pub(crate) fn value_fits(decl: &ParamTypeSrc, v: &str) -> bool {
    match decl.kind {
        ParamKind::Bool => v == "true" || v == "false",
        ParamKind::Int => canonical_int(v)
            .is_some_and(|n| decl.min.is_none_or(|m| n >= m) && decl.max.is_none_or(|m| n <= m)),
        ParamKind::Enum => decl
            .values
            .as_ref()
            .is_some_and(|vs| vs.iter().any(|x| x == v)),
    }
}

/// 宣言そのものの誤り。空なら宣言は使える。
fn declaration_errors(decl: &ParamTypeSrc) -> Vec<&'static str> {
    let mut out = Vec::new();
    if decl.kind == ParamKind::Enum {
        match decl.values.as_deref() {
            None | Some([]) => out.push("param_types enum requires non-empty values"),
            Some(vs) => {
                let uniq: BTreeSet<&String> = vs.iter().collect();
                if uniq.len() != vs.len() {
                    out.push("param_types values must not repeat");
                }
            }
        }
    } else if decl.values.is_some() {
        out.push("param_types values are only for enum");
    }
    if decl.kind == ParamKind::Int {
        if let (Some(lo), Some(hi)) = (decl.min, decl.max) {
            if lo > hi {
                out.push("param_types min must not exceed max");
            }
        }
    } else if decl.min.is_some() || decl.max.is_some() {
        out.push("param_types min and max are only for int");
    }
    out
}

/// 宣言が使えるか（誤りが無いか）。呼び出しの検査は、壊れた宣言を判定に使わず二重に報告しない。
fn declaration_ok(decl: &ParamTypeSrc) -> bool {
    declaration_errors(decl).is_empty()
}

/// 宣言の検査。章・モジュールの定義ごとに、`param_types` の鍵・組み合わせ・既定値を `E-STRUCTURE` にする。
/// 呼ばれないモジュールも検査する。診断の item は `param_types:<名前>`（名前は `safe_ident` を通るときだけ）。
pub(crate) fn check_param_types(scopes: &[LoadedScope]) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    for scope in scopes {
        for (name, decl) in &scope.src.param_types {
            let item = if safe_ident(name) {
                format!("param_types:{name}")
            } else {
                "param_types".to_string()
            };
            let mut push = |m: &str| out.push(diag(&scope.file, &item, DiagCode::Structure, m));
            let Some(default) = scope.src.params.get(name) else {
                push("param_types name is not declared in params");
                continue;
            };
            let errs = declaration_errors(decl);
            if errs.is_empty() {
                if !value_fits(decl, default) {
                    push("params default value is outside the declared type or range");
                }
            } else {
                for e in errs {
                    push(e);
                }
            }
        }
    }
    out
}

/// 呼び出しの引数 `name` の値 `value` が宣言に合わないときの診断の文面。合う・宣言が無い・宣言が壊れている
/// ときは `None`。値は文面に入れない。
pub(crate) fn arg_message(
    types: &BTreeMap<String, ParamTypeSrc>,
    name: &str,
    value: &str,
) -> Option<String> {
    let decl = types.get(name)?;
    if !declaration_ok(decl) || value_fits(decl, value) {
        return None;
    }
    Some(if safe_ident(name) {
        format!("{ARG_OUT_OF_RANGE}: {name}")
    } else {
        ARG_OUT_OF_RANGE.to_string()
    })
}

/// `validate` が展開の診断から選ぶための判定。
pub(crate) fn is_arg_range_message(m: &str) -> bool {
    m.starts_with(ARG_OUT_OF_RANGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int(min: Option<i64>, max: Option<i64>) -> ParamTypeSrc {
        ParamTypeSrc {
            kind: ParamKind::Int,
            values: None,
            min,
            max,
        }
    }

    #[test]
    fn bool_accepts_only_lowercase_words() {
        let d = ParamTypeSrc {
            kind: ParamKind::Bool,
            values: None,
            min: None,
            max: None,
        };
        assert!(value_fits(&d, "true") && value_fits(&d, "false"));
        for v in ["True", "flase", "", "1", " true"] {
            assert!(!value_fits(&d, v), "{v}");
        }
    }

    #[test]
    fn int_requires_canonical_decimal_inside_inclusive_range() {
        let d = int(Some(1), Some(3));
        assert!(value_fits(&d, "1") && value_fits(&d, "3") && value_fits(&d, "2"));
        for v in [
            "0", "4", "01", "+1", " 1", "1.0", "abc", "", "-", "0x1", "-0",
        ] {
            assert!(!value_fits(&d, v), "{v}");
        }
        let any = int(None, None);
        assert!(value_fits(&any, "0") && value_fits(&any, "-5"));
        assert!(!value_fits(&any, "-0") && !value_fits(&any, "00"));
        assert!(value_fits(&any, "9223372036854775807"));
        assert!(!value_fits(&any, "9223372036854775808"));
        assert!(value_fits(&any, "-9223372036854775808"));
    }

    #[test]
    fn enum_matches_exactly() {
        let d = ParamTypeSrc {
            kind: ParamKind::Enum,
            values: Some(vec!["fast".into(), "slow".into()]),
            min: None,
            max: None,
        };
        assert!(value_fits(&d, "fast"));
        assert!(!value_fits(&d, "Fast") && !value_fits(&d, "fast "));
    }

    #[test]
    fn messages_never_echo_the_value() {
        let mut m = BTreeMap::new();
        m.insert("n".to_string(), int(Some(1), Some(2)));
        let msg = arg_message(&m, "n", "SECRET-VALUE").expect("out of range");
        assert_eq!(msg, format!("{ARG_OUT_OF_RANGE}: n"));
        assert!(arg_message(&m, "n", "1").is_none());
        assert!(arg_message(&m, "other", "x").is_none());
        let mut odd = BTreeMap::new();
        odd.insert("a b".to_string(), int(None, Some(1)));
        assert_eq!(
            arg_message(&odd, "a b", "9").as_deref(),
            Some(ARG_OUT_OF_RANGE)
        );
    }
}
