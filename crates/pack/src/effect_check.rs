//! 効果の列（`on_enter`・`on_achieved`・`starts[].effects`）の読み込み時の検査（TASK-43 1/2）。
//!
//! `compile` が章の Step と開始点から呼ぶ。IR の欄は `Value` のまま通すため（版 1 の契約）、ここで
//! contract の [`EffectItem::from_value`] を通して語彙を検査し、誤りを診断の種類と文面にして返す。
//! 診断の組み立て（`file`・`item`）は呼び出し側の責務で、本モジュールは `(コード, 文面)` だけを返す。
//! 規則・入力方式の語彙（TASK-44 など）が `rules[].outcome.effects` を検査するときも再利用する。
//!
//! 文面には欄名と添字だけを入れ、属性値・本文は入れない。

use std::collections::BTreeMap;

use fandhe_workshop_contract::effect::{Effect, EffectItem, EffectParseError};
use fandhe_workshop_contract::ir::FixtureDef;
use serde_json::Value;

use crate::diag::DiagCode;

/// `list`（欄名 `field`）の各項目を検査し、誤りごとに `(コード, 文面)` を返す。
///
/// 文面は `field[添字]: ...` の形。`fixtures` は `workshop.fixtures` で、`put`・`replace` の `fixture` の
/// 参照を解決する。
pub(crate) fn check_effects(
    list: &[Value],
    field: &str,
    fixtures: &BTreeMap<String, FixtureDef>,
) -> Vec<(DiagCode, String)> {
    let mut out = Vec::new();
    for (i, v) in list.iter().enumerate() {
        let prefix = format!("{field}[{i}]");
        match EffectItem::from_value(v) {
            Err(e) => {
                let code = match e {
                    EffectParseError::Shape { .. } => DiagCode::Parse,
                    _ => DiagCode::Structure,
                };
                out.push((code, format!("{prefix}: {e}")));
            }
            Ok(item) => {
                if let Effect::Put(p) | Effect::Replace(p) = &item.effect {
                    if let Some(f) = &p.fixture {
                        if !fixtures.contains_key(f) {
                            out.push((
                                DiagCode::RefFixture,
                                format!("{prefix}: fixture is undefined: {f}"),
                            ));
                        }
                    }
                }
            }
        }
    }
    out
}
