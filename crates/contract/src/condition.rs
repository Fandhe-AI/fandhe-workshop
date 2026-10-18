//! 条件の語彙（状態・世界状態・論理。TASK-42 1/2）の型と、`Value` からの解釈。
//!
//! IR の `next[].when`・`rules[].when`・`starts[].requires`・効果の項目の `when` は `serde_json::Value` の
//! まま通す（TASK-41 の方針。版 1 の直列化と golden を変えないため、欄の型は変えない）。本モジュールは、
//! その `Value` を型にする境界の純関数 [`Condition::from_value`] を持つ。
//! 呼び出し元は 2 つある。pack の読み込み時の検査（pack crate の `condition_check`）と、
//! core の評価（`fandhe_workshop_core::condition`）である。2 つの責務が参照するため contract に置く。
//!
//! 語は 11 個。`step`・`flag`・`world_has`・`world_attr`・`count`・`chose` と、論理の `all`・`any`・`not`
//! は本件で評価まで実装する。`param`（EP-3）・`clarifying`（EP-4）は形だけを持ち、評価は TASK-42 2/2 が
//! 受け持つ。式やスクリプトの欄は持たない（未知の欄は拒否する）。
//!
//! 1 つの表にはちょうど 1 つの語を書く。`all`・`any`・`not` の入れ子は [`MAX_JSON_DEPTH`] で断る。
//! 解釈は I/O・panic を持たず、誤りの型は入力値を持たない（教材由来の値は診断に写さないため）。

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::effect::short_ident;
use crate::limits::MAX_JSON_DEPTH;

/// `step` 条件が比べる Step の状態。core の `StepStatus` と同じ 5 値（contract は core に依存しない）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// 未着手（未登録の Step もこれ）。
    NotStarted,
    /// 入って入力を待つ。
    Entered,
    /// 受付済み。
    Accepted,
    /// 達成。
    Achieved,
    /// 省略。
    Skipped,
}

/// `count` の比較演算子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum CmpOp {
    /// `<`。
    #[serde(rename = "<")]
    Lt,
    /// `<=`。
    #[serde(rename = "<=")]
    Le,
    /// `==`。
    #[serde(rename = "==")]
    Eq,
    /// `>=`。
    #[serde(rename = ">=")]
    Ge,
    /// `>`。
    #[serde(rename = ">")]
    Gt,
}

impl CmpOp {
    /// `a op b` が成り立つか。
    pub fn holds(self, a: u64, b: u64) -> bool {
        match self {
            CmpOp::Lt => a < b,
            CmpOp::Le => a <= b,
            CmpOp::Eq => a == b,
            CmpOp::Ge => a >= b,
            CmpOp::Gt => a > b,
        }
    }
}

/// `step` 条件の本体。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StepIs {
    /// Step の ID（根の中の名前）。
    pub step: String,
    /// 期待する状態。
    pub state: StepState,
}

/// `world_attr` 条件の本体。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldAttrIs {
    /// 世界状態の経路。
    pub path: String,
    /// 属性名。
    pub attr: String,
    /// 期待する値。
    pub eq: String,
}

/// `count` 条件の本体。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CountIs {
    /// 数える経路の前置。空なら全件。
    pub prefix: String,
    /// 絞り込み（属性名から値）。省略は絞り込まない。
    #[serde(default, rename = "where")]
    pub where_attrs: BTreeMap<String, String>,
    /// 比較演算子。
    pub op: CmpOp,
    /// 比べる件数。
    pub n: u64,
}

/// `chose` 条件の本体。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChoseIs {
    /// Step の ID（根の中の名前）。
    pub step: String,
    /// 選んだ選択肢の ID。
    pub option: String,
}

/// `param` 条件の本体（EP-3。評価は TASK-42 2/2）。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ParamIs {
    /// モジュールの引数名。
    pub name: String,
    /// 期待する値。
    pub eq: String,
}

/// 1 つの語による条件。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Condition {
    /// Step の状態。
    Step(StepIs),
    /// 実体ごとのフラグが立っている（名前）。
    Flag(String),
    /// 要素がある（経路）。
    WorldHas(String),
    /// 要素の属性が一致する。
    WorldAttr(WorldAttrIs),
    /// 要素の件数の比較。
    Count(CountIs),
    /// 選んだ選択肢。
    Chose(ChoseIs),
    /// すべてが真。
    All(Vec<Condition>),
    /// どれかが真。
    Any(Vec<Condition>),
    /// 否定。
    Not(Box<Condition>),
    /// モジュールの引数（EP-3）。
    Param(ParamIs),
    /// 問い返しの待ち（EP-4）。値は問い返しの ID。
    Clarifying(String),
}

/// 条件の解釈の誤り。入力値は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConditionParseError {
    /// 形の誤り。未知の欄（名前は短い識別子のときだけ持つ）、または型の不一致。
    Shape {
        /// 未知の欄の名前。型の不一致などでは `None`。
        unknown_field: Option<String>,
    },
    /// 語が 0 個、または 2 個以上。
    WordCount(usize),
    /// 名前が空。値は欄の名前。
    EmptyName(&'static str),
    /// `all`・`any`・`not` の入れ子が上限を超えた。
    TooDeep,
}

impl std::fmt::Display for ConditionParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConditionParseError::Shape {
                unknown_field: Some(n),
            } => write!(f, "unknown field `{n}`"),
            ConditionParseError::Shape {
                unknown_field: None,
            } => write!(
                f,
                "invalid condition: expected known fields with values of the right type"
            ),
            ConditionParseError::WordCount(n) => {
                write!(f, "condition must have exactly one word (found {n})")
            }
            ConditionParseError::EmptyName(w) => write!(f, "condition has an empty {w}"),
            ConditionParseError::TooDeep => write!(f, "condition is nested too deeply"),
        }
    }
}

impl std::error::Error for ConditionParseError {}

/// 条件の語の欄。
const WORDS: &[&str] = &[
    "step",
    "flag",
    "world_has",
    "world_attr",
    "count",
    "chose",
    "all",
    "any",
    "not",
    "param",
    "clarifying",
];

const SHAPE: ConditionParseError = ConditionParseError::Shape {
    unknown_field: None,
};

fn non_empty(s: &str, what: &'static str) -> Result<(), ConditionParseError> {
    if s.is_empty() {
        Err(ConditionParseError::EmptyName(what))
    } else {
        Ok(())
    }
}

/// 語の本体を型にする。serde の文言から未知の欄の名前を取り出し、短い識別子のときだけ持つ。
fn body<T: serde::de::DeserializeOwned>(v: &Value) -> Result<T, ConditionParseError> {
    serde_json::from_value(v.clone()).map_err(|e| {
        let msg = e.to_string();
        let name = msg.strip_prefix("unknown field `").and_then(|rest| {
            ["`, expected", "`, there are no fields"]
                .iter()
                .filter_map(|d| rest.find(d))
                .min()
                .and_then(|end| short_ident(&rest[..end]))
        });
        ConditionParseError::Shape {
            unknown_field: name,
        }
    })
}

fn string(v: &Value) -> Result<String, ConditionParseError> {
    v.as_str().map(str::to_string).ok_or(SHAPE)
}

impl Condition {
    /// `Value` から条件を解釈する。語はちょうど 1 つで、語彙にない欄は拒否する。
    pub fn from_value(v: &Value) -> Result<Condition, ConditionParseError> {
        parse(v, 0)
    }
}

fn parse(v: &Value, depth: usize) -> Result<Condition, ConditionParseError> {
    if depth > MAX_JSON_DEPTH {
        return Err(ConditionParseError::TooDeep);
    }
    let obj = v.as_object().ok_or(SHAPE)?;
    if let Some(k) = obj.keys().find(|k| !WORDS.contains(&k.as_str())) {
        return Err(ConditionParseError::Shape {
            unknown_field: short_ident(k),
        });
    }
    if obj.len() != 1 {
        return Err(ConditionParseError::WordCount(obj.len()));
    }
    let (word, val) = obj.iter().next().ok_or(ConditionParseError::WordCount(0))?;
    match word.as_str() {
        "step" => {
            let s: StepIs = body(val)?;
            non_empty(&s.step, "step")?;
            Ok(Condition::Step(s))
        }
        "flag" => {
            let n = string(val)?;
            non_empty(&n, "flag")?;
            Ok(Condition::Flag(n))
        }
        "world_has" => {
            let n = string(val)?;
            non_empty(&n, "path")?;
            Ok(Condition::WorldHas(n))
        }
        "world_attr" => {
            let w: WorldAttrIs = body(val)?;
            non_empty(&w.path, "path")?;
            non_empty(&w.attr, "attr")?;
            Ok(Condition::WorldAttr(w))
        }
        "count" => {
            let c: CountIs = body(val)?;
            if c.where_attrs.contains_key("") {
                return Err(ConditionParseError::EmptyName("attr"));
            }
            Ok(Condition::Count(c))
        }
        "chose" => {
            let c: ChoseIs = body(val)?;
            non_empty(&c.step, "step")?;
            non_empty(&c.option, "option")?;
            Ok(Condition::Chose(c))
        }
        "all" | "any" => {
            let list = val.as_array().ok_or(SHAPE)?;
            let children = list
                .iter()
                .map(|c| parse(c, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(if word == "all" {
                Condition::All(children)
            } else {
                Condition::Any(children)
            })
        }
        "not" => Ok(Condition::Not(Box::new(parse(val, depth + 1)?))),
        "param" => {
            let p: ParamIs = body(val)?;
            non_empty(&p.name, "name")?;
            Ok(Condition::Param(p))
        }
        "clarifying" => {
            let n = string(val)?;
            non_empty(&n, "clarifying")?;
            Ok(Condition::Clarifying(n))
        }
        _ => Err(SHAPE),
    }
}
