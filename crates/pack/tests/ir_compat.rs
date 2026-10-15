//! エンジンの IR の読み込み口 `load_ir` の互換の適合試験（TASK-41 の 2/2、04-behavior の PACK-24）。
//!
//! 読み込み口は、加算の前に作られた IR を読み、未知の欄を持つ IR を黙って捨てずに拒否する。
//! ここでは現在の `load_ir` を「その欄を知らない旧エンジン」の立場として使い、次を固定する。
//!
//! - 旧 → 新: 任意の欄を使わない IR（compile の golden、`null` を含む試作の golden）を読め、同じ値に戻る。
//! - 新 → 旧: 版 1 のまま型付きの項目に未知の欄を足した IR を拒否する。拒否の理由に入力値を含めない。
//! - 読めない版（3 など）の IR は、未知の欄を含んでいても「版が未対応」として断る（型の不一致にしない）。
//! - 版 2（TASK-60）: 版 2 の golden を読め、同じバイト列に戻る。木に未知の種別・欄があれば拒否する。
//!
//! 旧 → 新の読み先は、版 1 の golden を凍結した `tests/data/ir_format_1/`（TASK-60 の前の出力）。
//!
//! 版 1 の IR の扱い（TASK-60 の 2/2、D-52・D-59、PACK-24）: 版 2 の欄を持たない版 1 の IR は変換せずそのまま
//! 読む（木を補わない）。版 1 と名乗る木つきの IR・版の食い違い・未対応の版は理由つきで拒否する。
//! 版 1 の履歴の扱いは TASK-174・TASK-310 が受け持つ。本文を表示するタスクと、IR を読み書きする他タスクは、
//! 統合の完了条件としてこの試験と `crates/contract/tests/ir_v1_handling.rs` を参照する。
//! 型の水準の試験は `crates/contract/tests/ir_compat.rs`。
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
    golden_in("ir_format_1", name)
}

fn golden_in(dir: &str, name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir)
        .join(format!("{name}.ir.json"));
    fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {dir}/{name}"))
}

#[test]
fn v2_goldens_load_and_roundtrip_byte_identical() {
    for name in GOLDENS.into_iter().chain(["fixture-pack"]) {
        let g = golden_in("golden", name);
        let ir = load_ir(&g).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(ir.format_version, 2, "{name}");
        assert_eq!(to_canonical_json(&ir).unwrap(), g, "{name}");
    }
}

#[test]
fn v2_tree_with_unknown_kind_or_field_is_rejected_without_echo() {
    let base: Value = serde_json::from_slice(&golden_in("golden", "fixture-pack")).unwrap();
    for ptr in [
        "/nodes/0/body_doc",
        "/fixture_docs/f-doc",
        "/nodes/0/body_doc/blocks/0",
    ] {
        let mut v = base.clone();
        inject(&mut v, ptr);
        let Err(e) = load_ir(&bytes(&v)) else {
            panic!("{ptr}: unknown field in a tree must be rejected")
        };
        assert!(matches!(e, IrLoadError::Json(_)), "{ptr}: {e}");
        let s = e.to_string();
        assert!(
            !s.contains(SENTINEL_VALUE) && !s.contains(SENTINEL_KEY),
            "{s}"
        );
    }
    let mut v = base;
    v["nodes"][0]["body_doc"]["blocks"][0] = json!({"t": "script", "text": SENTINEL_VALUE});
    let Err(e) = load_ir(&bytes(&v)) else {
        panic!("unknown block kind must be rejected")
    };
    assert!(matches!(e, IrLoadError::Json(_)));
    assert!(!e.to_string().contains(SENTINEL_VALUE));
}

#[test]
fn v1_declared_ir_with_trees_is_rejected() {
    let mut v: Value = serde_json::from_slice(&golden_in("golden", "fixture-pack")).unwrap();
    v["format_version"] = json!(1);
    v["pack"]["ir_format"] = json!(1);
    assert!(matches!(
        load_ir(&bytes(&v)).unwrap_err(),
        IrLoadError::FormatInconsistent { .. }
    ));
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
        // 読むときに版 2 へ変換しない（木を補わない）。
        assert_eq!(ir.format_version, 1, "{name}");
        assert!(ir.fixture_docs.is_empty(), "{name}");
        assert!(
            ir.nodes
                .iter()
                .all(|n| n.body_doc.is_none() && n.body_levels_doc.is_empty()),
            "{name}"
        );
    }
}

#[test]
fn v1_declared_ir_with_each_v2_field_is_rejected_without_echo() {
    let v2: Value = serde_json::from_slice(&golden_in("golden", "fixture-pack")).unwrap();
    for name in GOLDENS {
        let base: Value = serde_json::from_slice(&golden(name)).unwrap();
        for field in ["body_doc", "body_levels_doc", "fixture_docs"] {
            let mut v = base.clone();
            if field == "fixture_docs" {
                v[field] = v2[field].clone();
            } else {
                v["nodes"][0][field] = v2["nodes"][0][field].clone();
            }
            let Err(e) = load_ir(&bytes(&v)) else {
                panic!("{name}/{field}: a v1 IR with a v2 field must be rejected")
            };
            assert!(
                matches!(
                    e,
                    IrLoadError::FormatInconsistent {
                        format_version: 1,
                        pack_ir_format: 1
                    }
                ),
                "{name}/{field}: {e}"
            );
        }
    }
}

#[test]
fn v1_golden_with_mismatched_versions_is_rejected() {
    let base: Value = serde_json::from_slice(&golden("min-3step-pack")).unwrap();
    let mut a = base.clone();
    a["format_version"] = json!(2);
    let mut b = base;
    b["pack"]["ir_format"] = json!(2);
    for v in [a, b] {
        assert!(matches!(
            load_ir(&bytes(&v)).unwrap_err(),
            IrLoadError::FormatInconsistent { .. }
        ));
    }
}

#[test]
fn version_zero_is_unsupported() {
    let base: Value = serde_json::from_slice(&golden("min-3step-pack")).unwrap();
    let mut a = base.clone();
    a["format_version"] = json!(0);
    assert!(matches!(
        load_ir(&bytes(&a)).unwrap_err(),
        IrLoadError::FormatVersion(0)
    ));
    let mut b = base;
    b["pack"]["ir_format"] = json!(0);
    assert!(matches!(
        load_ir(&bytes(&b)).unwrap_err(),
        IrLoadError::PackIrFormat(0)
    ));
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
    v["format_version"] = json!(3);
    assert_eq!(
        load_ir(&bytes(&v)).unwrap_err(),
        IrLoadError::FormatVersion(3)
    );

    let mut v = base;
    v["pack"]["ir_format"] = json!(3);
    assert_eq!(
        load_ir(&bytes(&v)).unwrap_err(),
        IrLoadError::PackIrFormat(3)
    );
}
