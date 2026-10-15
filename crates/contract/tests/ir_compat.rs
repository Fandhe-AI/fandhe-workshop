//! IR の加算の互換の適合試験（TASK-41 の 2/2、04-behavior の PACK-24）。
//!
//! `ir_format` 1 は、語彙の追加を任意の欄の加算だけで行う契約である。そこから次の互換の向きが決まる。
//!
//! - 新 → 旧: 版 1 のまま型付きの項目に未知の欄を足した IR は、その欄を知らない型が拒否する
//!   （黙って欄を捨てて読み進めない）。ここでは、入れ子の位置（配列・連想配列の要素を含む）の全部で
//!   拒否が伝わることを固定する。型ごとの単体の拒否は `ir_contract.rs` が受け持つ。
//! - 旧 → 新: 任意の欄を使わない旧い IR は、任意の欄を足した新しい型でも読め、同じバイト列に戻る。
//!   本物の加算は語彙の追加のタスク（TASK-194 等）で行う。ここでは契約の型を変えず、試験の中だけで
//!   加算の模型の型を定義して、加算の規則（既定値は書き出さない・未知の欄は拒否する）を固定する。
//!
//! 限界: `Value` で通している語彙の中身（`input`・`confirm`・`on_enter` 等）の中に足した欄は、
//! 現状では拒否されない（PoC-6 の `AnswerOption.label` はこの中身に当たる。型付けは TASK-42 以降）。
//! エンジンの読み込み口 `load_ir` の水準の試験は `crates/pack/tests/ir_compat.rs` が受け持つ。
//! golden は書き換えない（契約の変更になる）。試験では `unwrap` を使ってよい。

use std::path::Path;

use fandhe_workshop_contract::ir::{Ir, Transition};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

const SENTINEL_KEY: &str = "future_v1_x";
const SENTINEL_VALUE: &str = "SECRET-VALUE";

fn full() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/ir_format_1/full.json");
    serde_json::from_slice(&std::fs::read(&p).expect("cannot read golden full.json")).unwrap()
}

/// 番兵の欄を `target` に足した IR を返す。
fn inject(
    name: &'static str,
    target: impl FnOnce(&mut Value) -> &mut Value,
) -> (&'static str, Value) {
    let mut v = full();
    target(&mut v)
        .as_object_mut()
        .unwrap()
        .insert(SENTINEL_KEY.into(), json!(SENTINEL_VALUE));
    (name, v)
}

/// 型付きの 8 か所のそれぞれに、番兵の欄を 1 つだけ足した IR を返す。
fn injected_variants() -> Vec<(&'static str, Value)> {
    let fixture_key = full()["fixtures"]
        .as_object()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    vec![
        inject("Ir", |v| v),
        inject("PackInfo", |v| &mut v["pack"]),
        inject("Node", |v| &mut v["nodes"][0]),
        inject("Step", |v| &mut v["nodes"][0]["step"]),
        inject("Transition", |v| &mut v["nodes"][0]["step"]["next"][0]),
        inject("Start", |v| &mut v["starts"][0]),
        inject("Instance", |v| &mut v["instances"][0]),
        inject("FixtureDef", |v| &mut v["fixtures"][fixture_key.as_str()]),
    ]
}

#[test]
fn new_to_old_unknown_field_is_rejected_at_every_nested_position() {
    serde_json::from_value::<Ir>(full()).expect("control: full.json must be readable");
    let variants = injected_variants();
    assert_eq!(variants.len(), 8);
    for (name, v) in variants {
        let err = serde_json::from_value::<Ir>(v)
            .err()
            .unwrap_or_else(|| panic!("{name}: unknown field must be rejected"));
        assert!(
            err.to_string().contains(SENTINEL_KEY),
            "{name}: rejection must come from the injected field: {err}"
        );
    }
}

/// 加算の模型。`Transition` に版 1 の後で任意の欄 `label` が足された、という想定の型。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct TransitionNext {
    to: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    when: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    label: Option<String>,
}

#[test]
fn old_to_new_additive_field_keeps_old_documents_byte_identical() {
    let olds = [r#"{"to":"b"}"#, r#"{"to":"b","when":{"all":[]}}"#];
    for old in olds {
        let new: TransitionNext = serde_json::from_str(old).unwrap();
        assert_eq!(new.label, None);
        // 新しい項目を使わない IR は、新しい型で書き戻してもバイト単位で同じ。
        assert_eq!(serde_json::to_string(&new).unwrap(), old);
        // 旧い型と新しい型は、使わない欄については同じ値に読める。
        let legacy: Transition = serde_json::from_str(old).unwrap();
        assert_eq!(legacy.to, new.to);
        assert_eq!(legacy.when, new.when);
    }
}

#[test]
fn new_to_old_additive_field_is_rejected_by_the_old_type() {
    let used = r#"{"to":"b","label":"x"}"#;
    let new: TransitionNext = serde_json::from_str(used).unwrap();
    assert_eq!(serde_json::to_string(&new).unwrap(), used);
    let err = serde_json::from_str::<Transition>(used).unwrap_err();
    assert!(err.to_string().contains("label"), "{err}");
}
