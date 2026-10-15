//! 状態変化の語彙（要素・属性・フラグ）の効果の型と、`Value` からの解釈（TASK-43 1/2）。
//!
//! IR の `on_enter`・`on_achieved`・`starts[].effects` は `serde_json::Value` のまま通す（TASK-41 の方針。
//! 版 1 の契約を変えない）。本モジュールは、その `Value` を型にする境界の純関数を持つ。
//! 呼び出し元は 2 つある。pack の読み込み時の検査（未知の欄・語の数・参照の拒否）と、core の適用
//! （`fandhe_workshop_core::effect`）である。2 つの責務が参照するため contract に置く。
//!
//! 対象の語は `put`・`remove`・`replace`・`set_attr`・`set_flag`・`clear_flag`・`set_flag_global`
//! と、進行（`processing`、[`ProcessingTick`]）・条件つき（`when`）である（TASK-43 2/2）。
//! `when` は、欄があるなら表（JSON オブジェクト）であることだけを検査し、中身（条件の語）は解釈せず
//! [`EffectItem::when`] に `Value` のまま保持する。条件の型（TASK-42）に置き換えても、本件で書いた教材を
//! 壊さない。`processing` は Step の欄で、効果の列の 1 項目ではないため [`Effect`] には加えない。
//!
//! `path` は世界状態の鍵（メモリ上の `BTreeMap` の鍵）で、ファイルパス・URL として扱わない。
//! モジュールの引数の差し込み（`${param}`）を拒否しないよう、空でないこと以外は検査しない。
//!
//! 解釈の関数は I/O・panic を持たない。誤りの型は入力値を持たない（型の合わない入力値は
//! 教材・利用者由来で、診断に写さないため）。

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

/// `put`・`replace` の項目。
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PutEffect {
    /// 世界状態の経路。
    pub path: String,
    /// 練習データの ID。
    #[serde(default)]
    pub fixture: Option<String>,
    /// 属性名から値への対応。
    #[serde(default)]
    pub attrs: BTreeMap<String, String>,
}

/// `set_attr` の項目。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetAttrEffect {
    /// 世界状態の経路。
    pub path: String,
    /// 属性名。
    pub attr: String,
    /// 属性の値。
    pub value: String,
}

/// 1 つの語による効果。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Effect {
    /// 要素を置く。
    Put(PutEffect),
    /// 要素を消す（経路）。
    Remove(String),
    /// 要素を置き換える。
    Replace(PutEffect),
    /// 属性を変える。
    SetAttr(SetAttrEffect),
    /// 実体ごとのフラグを立てる（名前）。
    SetFlag(String),
    /// 実体ごとのフラグを下ろす（名前）。
    ClearFlag(String),
    /// 教材全体のフラグを立てる（名前）。
    SetFlagGlobal(String),
}

/// 効果の列の 1 項目。ちょうど 1 つの語と、任意の `when` を持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct EffectItem {
    /// 語の効果。
    pub effect: Effect,
    /// 条件。表であることだけを検査し、`Value` のまま持つ（条件の語の解釈は TASK-42）。
    pub when: Option<Value>,
}

/// 効果の項目の解釈の誤り。入力値は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectParseError {
    /// 形の誤り。未知の欄（名前は短い識別子のときだけ持つ）、または型の不一致。
    Shape {
        /// 未知の欄の名前。型の不一致などでは `None`。
        unknown_field: Option<String>,
    },
    /// 語が 0 個、または 2 個以上。
    WordCount(usize),
    /// 名前が空。値は欄の名前（`path`・`attr`・`flag`）。
    EmptyName(&'static str),
}

impl std::fmt::Display for EffectParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EffectParseError::Shape {
                unknown_field: Some(n),
            } => write!(f, "unknown field `{n}`"),
            EffectParseError::Shape {
                unknown_field: None,
            } => {
                write!(
                    f,
                    "invalid effect: expected known fields with string values"
                )
            }
            EffectParseError::WordCount(n) => {
                write!(f, "effect must have exactly one word (found {n})")
            }
            EffectParseError::EmptyName(w) => write!(f, "effect has an empty {w}"),
        }
    }
}

impl std::error::Error for EffectParseError {}

/// 項目の生の形。語はすべて `Option` で受け、ちょうど 1 つかを後で数える。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EffectRaw {
    put: Option<PutEffect>,
    remove: Option<String>,
    replace: Option<PutEffect>,
    set_attr: Option<SetAttrEffect>,
    set_flag: Option<String>,
    clear_flag: Option<String>,
    set_flag_global: Option<String>,
    when: Option<Value>,
}

/// serde の文言から未知の欄の名前を取り出す。短い識別子のときだけ返す。
fn unknown_field_name(msg: &str) -> Option<String> {
    let rest = msg.strip_prefix("unknown field `")?;
    let end = ["`, expected", "`, there are no fields"]
        .iter()
        .filter_map(|d| rest.find(d))
        .min()?;
    short_ident(&rest[..end])
}

/// 診断に写してよい短い識別子（欄名）のときだけ `Some`。教材由来の長い・奇妙な文字列は写さない。
fn short_ident(name: &str) -> Option<String> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    ok.then(|| name.to_string())
}

/// 効果の語の欄。
const WORDS: &[&str] = &[
    "put",
    "remove",
    "replace",
    "set_attr",
    "set_flag",
    "clear_flag",
    "set_flag_global",
];

fn non_empty(s: &str, what: &'static str) -> Result<(), EffectParseError> {
    if s.is_empty() {
        Err(EffectParseError::EmptyName(what))
    } else {
        Ok(())
    }
}

/// `put`・`replace` の `attrs` のキーを `set_attr` の `attr` と同じ規則（空文字の拒否）で検査する。
fn non_empty_attr_keys(
    attrs: &std::collections::BTreeMap<String, String>,
) -> Result<(), EffectParseError> {
    if attrs.contains_key("") {
        return Err(EffectParseError::EmptyName("attr"));
    }
    Ok(())
}

impl EffectItem {
    /// `Value` から項目を解釈する。語はちょうど 1 つで、語彙にない欄は拒否する。
    pub fn from_value(v: &Value) -> Result<EffectItem, EffectParseError> {
        // serde の構造体は位置指定の配列も受理し、`Option` は null を未指定として扱う。
        // 未信頼の教材入力のため、構造体へ入れる前に元のオブジェクトの形を検査する。
        let obj = v.as_object().ok_or(EffectParseError::Shape {
            unknown_field: None,
        })?;
        // `when` は表だけを許す。null・スカラー・配列は、条件が無いものとして素通りさせない。
        if obj.get("when").is_some_and(|w| !w.is_object()) {
            return Err(EffectParseError::Shape {
                unknown_field: None,
            });
        }
        let mut count = 0;
        for word in WORDS {
            if let Some(val) = obj.get(*word) {
                let shape_ok = match *word {
                    "put" | "replace" | "set_attr" => val.is_object(),
                    _ => val.is_string(),
                };
                if !shape_ok {
                    return Err(EffectParseError::Shape {
                        unknown_field: None,
                    });
                }
                count += 1;
            }
        }
        let raw: EffectRaw =
            serde_json::from_value(v.clone()).map_err(|e| EffectParseError::Shape {
                unknown_field: unknown_field_name(&e.to_string()),
            })?;
        if count != 1 {
            return Err(EffectParseError::WordCount(count));
        }
        let effect = if let Some(p) = raw.put {
            non_empty(&p.path, "path")?;
            non_empty_attr_keys(&p.attrs)?;
            Effect::Put(p)
        } else if let Some(p) = raw.replace {
            non_empty(&p.path, "path")?;
            non_empty_attr_keys(&p.attrs)?;
            Effect::Replace(p)
        } else if let Some(p) = raw.remove {
            non_empty(&p, "path")?;
            Effect::Remove(p)
        } else if let Some(s) = raw.set_attr {
            non_empty(&s.path, "path")?;
            non_empty(&s.attr, "attr")?;
            Effect::SetAttr(s)
        } else if let Some(n) = raw.set_flag {
            non_empty(&n, "flag")?;
            Effect::SetFlag(n)
        } else if let Some(n) = raw.clear_flag {
            non_empty(&n, "flag")?;
            Effect::ClearFlag(n)
        } else if let Some(n) = raw.set_flag_global {
            non_empty(&n, "flag")?;
            Effect::SetFlagGlobal(n)
        } else {
            return Err(EffectParseError::WordCount(0));
        };
        Ok(EffectItem {
            effect,
            when: raw.when,
        })
    }
}

/// 進行 1 つ（`processing[]` の要素）。名前と、進むたびに適用する効果の列を持つ。
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessingTick {
    /// 進行の名前（`Event::Progress` の表示名）。メモリ上の文字列で、パス・URL として解決しない。
    pub name: String,
    /// 適用する効果の列。
    pub effects: Vec<EffectItem>,
}

/// 進行の解釈の誤り。入力値は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProcessingParseError {
    /// 形の誤り。未知の欄（名前は短い識別子のときだけ持つ）、または型の不一致。
    Shape {
        /// 未知の欄の名前。型の不一致などでは `None`。
        unknown_field: Option<String>,
    },
    /// 名前が空。値は欄の名前（`name`）。
    EmptyName(&'static str),
    /// `effects[index]` の誤り。
    Effect {
        /// 効果の列の添字。
        index: usize,
        /// 項目の誤り。
        error: EffectParseError,
    },
}

impl std::fmt::Display for ProcessingParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessingParseError::Shape {
                unknown_field: Some(n),
            } => write!(f, "unknown field `{n}`"),
            ProcessingParseError::Shape {
                unknown_field: None,
            } => write!(
                f,
                "invalid processing: expected an object with `name` (string) and `effects` (list)"
            ),
            ProcessingParseError::EmptyName(w) => write!(f, "processing has an empty {w}"),
            ProcessingParseError::Effect { index, error } => write!(f, "effects[{index}]: {error}"),
        }
    }
}

impl std::error::Error for ProcessingParseError {}

impl ProcessingTick {
    /// 進行の外形（オブジェクト・欄は `name`・`effects` のみ・`name` は空でない文字列・`effects` は配列）
    /// だけを検査し、名前と `effects` の生の列を返す。項目の解釈は [`ProcessingTick::from_value`]、
    /// または pack の検査が項目ごとに行う。
    pub fn split_value(v: &Value) -> Result<(String, Vec<Value>), ProcessingParseError> {
        let shape = |unknown_field| ProcessingParseError::Shape { unknown_field };
        let obj = v.as_object().ok_or_else(|| shape(None))?;
        if let Some(k) = obj
            .keys()
            .find(|k| !matches!(k.as_str(), "name" | "effects"))
        {
            return Err(shape(short_ident(k)));
        }
        let name = obj
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| shape(None))?;
        let effects = match obj.get("effects") {
            None => Vec::new(),
            Some(Value::Array(a)) => a.clone(),
            Some(_) => return Err(shape(None)),
        };
        if name.is_empty() {
            return Err(ProcessingParseError::EmptyName("name"));
        }
        Ok((name.to_string(), effects))
    }

    /// `Value` から進行を解釈する。誤りは入力値を持たない。
    pub fn from_value(v: &Value) -> Result<ProcessingTick, ProcessingParseError> {
        let (name, raw) = Self::split_value(v)?;
        let mut effects = Vec::with_capacity(raw.len());
        for (index, e) in raw.iter().enumerate() {
            effects.push(
                EffectItem::from_value(e)
                    .map_err(|error| ProcessingParseError::Effect { index, error })?,
            );
        }
        Ok(ProcessingTick { name, effects })
    }
}
