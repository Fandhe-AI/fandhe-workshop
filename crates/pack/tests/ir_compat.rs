//! エンジンの IR の読み込み口 `load_ir` の互換の適合試験（TASK-41 の 2/2、04-behavior の PACK-24）。
//!
//! 読み込み口は、加算の前に作られた IR を読み、未知の欄を持つ IR を黙って捨てずに拒否する。
//! ここでは現在の `load_ir` を「その欄を知らない旧エンジン」の立場として使い、次を固定する。
//!
//! - 旧 → 新: 任意の欄を使わない IR（compile の golden、`null` を含む試作の golden）を読め、同じ値に戻る。
//! - 新 → 旧: 版 1 のまま型付きの項目に未知の欄を足した IR を拒否する。拒否の理由に入力値を含めない。
//! - 版が 1 でない IR は、未知の欄を含んでいても「版が未対応」として断る（型の不一致にしない）。
//!
//! IR を読み書きする他タスクは、統合の完了条件としてこの試験を参照できる。範囲は版 1 だけで、
//! 版 2 の受け入れは TASK-60 が扱う。型の水準の試験は `crates/contract/tests/ir_compat.rs`。
//! 限界: `Value` で通している語彙の中身（`input`・`confirm` 等）の中の未知の欄は現状では拒否されない
//! （型付けは TASK-42 以降）。golden には `Instance`・`FixtureDef` が無いため、その位置は contract 側が受け持つ。
//! golden は読むだけで書き換えない。試験では `unwrap` を使ってよい。

mod common;

use std::fs;
use std::path::Path;

use common::testdata;
use fandhe_workshop_contract::ir::to_canonical_json;
use fandhe_workshop_core::ir_load::{load_ir, IrLoadError};
use serde_json::{json, Value};

const GOLDENS: [&str; 4] = ["min-3step-pack", "ch1-pack", "ime-pack", "min-3step-pack-b"];
const SENTINEL_KEY: &str = "future_v1_x";
const SENTINEL_VALUE: &str = "SECRET-VALUE";

fn golden(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/golden")
        .join(format!("{name}.ir.json"));
    fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {name}"))
}

fn bytes(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}

#[test]
fn old_to_new_compile_goldens_roundtrip_byte_identical() {
    for name in GOLDENS {
        let g = golden(name);
        let ir = load_ir(&g).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(to_canonical_json(&ir).unwrap(), g, "{name}");
    }
}

#[test]
fn old_to_new_prototype_goldens_with_null_are_readable() {
    let files = [
        "engine-event-consistency/test-data/min-3step.json",
        "input-field-ime/test-data/ch1-pack.json",
        "input-field-ime/test-data/ime-pack.json",
    ];
    for f in files {
        let ir = load_ir(&fs::read(testdata(f)).unwrap()).unwrap_or_else(|e| panic!("{f}: {e}"));
        let out = to_canonical_json(&ir).unwrap();
        // 型付きの任意の欄の null は書き出さない（`Value` で通す語彙の中身の null は値として残る）。
        let v: Value = serde_json::from_slice(&out).unwrap();
        let mut typed = vec![&v["pack"]];
        for n in v["nodes"].as_array().unwrap() {
            typed.extend([n, &n["step"]]);
        }
        for o in typed {
            let nulls: Vec<_> = o
                .as_object()
                .unwrap()
                .iter()
                .filter(|(_, x)| x.is_null())
                .collect();
            assert!(nulls.is_empty(), "{f}: {nulls:?}");
        }
        assert_eq!(load_ir(&out).unwrap(), ir, "{f}");
    }
}

/// `ptr`（JSON Pointer）が指すオブジェクトに番兵の欄を足す。
fn inject(v: &mut Value, ptr: &str) {
    v.pointer_mut(ptr)
        .and_then(Value::as_object_mut)
        .unwrap_or_else(|| panic!("no object at {ptr:?}"))
        .insert(SENTINEL_KEY.into(), json!(SENTINEL_VALUE));
}

/// golden に実在する型付きの位置（JSON Pointer と型名）を列挙する。
fn typed_positions(v: &Value) -> Vec<(String, &'static str)> {
    let mut out = vec![(String::new(), "Ir"), ("/pack".to_owned(), "PackInfo")];
    for (i, n) in v["nodes"].as_array().unwrap().iter().enumerate() {
        out.push((format!("/nodes/{i}"), "Node"));
        out.push((format!("/nodes/{i}/step"), "Step"));
        let nexts = n["step"]["next"].as_array().map_or(0, Vec::len);
        for j in 0..nexts {
            out.push((format!("/nodes/{i}/step/next/{j}"), "Transition"));
        }
    }
    for k in 0..v["starts"].as_array().unwrap().len() {
        out.push((format!("/starts/{k}"), "Start"));
    }
    for k in 0..v["instances"].as_array().map_or(0, Vec::len) {
        out.push((format!("/instances/{k}"), "Instance"));
    }
    for k in v["fixtures"]
        .as_object()
        .map_or(vec![], |m| m.keys().cloned().collect())
    {
        out.push((
            format!("/fixtures/{}", k.replace('~', "~0").replace('/', "~1")),
            "FixtureDef",
        ));
    }
    out
}

#[test]
fn new_to_old_unknown_field_is_rejected_without_echoing_input() {
    let mut seen = std::collections::BTreeSet::new();
    for name in GOLDENS {
        let base: Value = serde_json::from_slice(&golden(name)).unwrap();
        for (ptr, ty) in typed_positions(&base) {
            let mut v = base.clone();
            inject(&mut v, &ptr);
            let Err(e) = load_ir(&bytes(&v)) else {
                panic!("{name} {ptr:?} ({ty}): unknown field must be rejected")
            };
            assert!(matches!(e, IrLoadError::Json(_)), "{name} {ptr:?}: {e}");
            let s = e.to_string();
            assert!(
                !s.contains(SENTINEL_VALUE) && !s.contains(SENTINEL_KEY),
                "{s}"
            );
            seen.insert(ty);
        }
    }
    for ty in ["Ir", "PackInfo", "Node", "Step", "Transition", "Start"] {
        assert!(seen.contains(ty), "no injection into {ty}");
    }
}

#[test]
fn new_to_old_other_version_is_unsupported_even_with_unknown_fields() {
    let mut base: Value = serde_json::from_slice(&golden("ch1-pack")).unwrap();
    inject(&mut base, "");
    inject(&mut base, "/pack");
    inject(&mut base, "/nodes/0/step");

    let mut v = base.clone();
    v["format_version"] = json!(2);
    assert_eq!(
        load_ir(&bytes(&v)).unwrap_err(),
        IrLoadError::FormatVersion(2)
    );

    let mut v = base;
    v["pack"]["ir_format"] = json!(2);
    assert_eq!(
        load_ir(&bytes(&v)).unwrap_err(),
        IrLoadError::PackIrFormat(2)
    );
}
