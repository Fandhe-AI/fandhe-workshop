//! 世界状態の要素の型（TASK-204 1/3）。`Event::Changed` の変更前後の値に要る。
//!
//! 世界状態全体の容れ物は TASK-204 の 2/3 が置く。値は教材・利用者由来の文字列を含みうるため、
//! HTML へ埋め込む際のエスケープは web 層の責務である。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// 世界状態の 1 要素。属性は `BTreeMap` で持ち、直列化の順を決定的にする（EVT-22）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Element {
    /// 見た目・振る舞いの部品の名前。無ければ `None`。
    pub fixture: Option<String>,
    /// 属性名から値への対応。
    pub attrs: BTreeMap<String, String>,
}
