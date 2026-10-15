//! 版 1 の IR の扱いの契約適合試験（TASK-60 の 2/2、D-52・D-59、04-behavior の PACK-24）。
//!
//! 版 2 の欄を持たない版 1 の IR は、変換せずにそのまま読める（木を補わず、書き戻すとバイト一致する）。
//! 版 2 の欄を足すと必要な版が 2 になり、版を偽る組み合わせは `Ir::has_consistent_format` が偽を返す。
//! `load_ir` の水準は `crates/pack/tests/ir_compat.rs`（contract は core に依存できないため、ここでは呼ばない）。
//! 版 1 の履歴の扱いは TASK-174・TASK-310 が受け持つ。本文を表示するタスクは、統合の完了条件として
//! この試験を参照する。golden は読むだけで書き換えない。試験では `unwrap` を使ってよい。

use std::path::Path;

use fandhe_workshop_contract::ir::*;
use serde_json::{json, Value};

fn golden(dir: &str, name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(dir)
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {dir}/{name}"))
}

fn parse(v: &Value) -> Ir {
    serde_json::from_value(v.clone()).unwrap()
}

const V1: [&str; 2] = ["minimal.json", "full.json"];

#[test]
fn v1_goldens_are_read_as_is_without_trees() {
    for name in V1 {
        let g = golden("ir_format_1", name);
        let ir: Ir = serde_json::from_slice(&g).unwrap();
        assert_eq!(ir.format_version, 1, "{name}");
        assert_eq!(ir.pack.ir_format, 1, "{name}");
        assert!(ir.fixture_docs.is_empty(), "{name}");
        assert!(
            ir.nodes
                .iter()
                .all(|n| n.body_doc.is_none() && n.body_levels_doc.is_empty()),
            "{name}"
        );
        assert_eq!(ir.required_ir_format(), 1, "{name}");
        assert!(ir.has_consistent_format(), "{name}");
        assert_eq!(to_canonical_json(&ir).unwrap(), g, "{name}");
    }
}

#[test]
fn adding_each_v2_field_requires_version_two() {
    let v2: Value = serde_json::from_slice(&golden("ir_format_2", "full.json")).unwrap();
    let base: Value = serde_json::from_slice(&golden("ir_format_1", "full.json")).unwrap();
    assert!(!base["nodes"].as_array().unwrap().is_empty());
    for field in ["body_doc", "body_levels_doc", "fixture_docs"] {
        let mut v = base.clone();
        if field == "fixture_docs" {
            v[field] = v2[field].clone();
        } else {
            v["nodes"][0][field] = v2["nodes"][0][field].clone();
        }
        let declared_v1 = parse(&v);
        assert_eq!(declared_v1.required_ir_format(), 2, "{field}");
        assert!(!declared_v1.has_consistent_format(), "{field}");
        v["format_version"] = json!(2);
        v["pack"]["ir_format"] = json!(2);
        assert!(parse(&v).has_consistent_format(), "{field}");
    }
}

#[test]
fn mismatched_versions_are_inconsistent() {
    let base: Value = serde_json::from_slice(&golden("ir_format_1", "full.json")).unwrap();
    let mut a = base.clone();
    a["format_version"] = json!(2);
    let mut b = base;
    b["pack"]["ir_format"] = json!(2);
    assert!(!parse(&a).has_consistent_format());
    assert!(!parse(&b).has_consistent_format());
}

#[test]
fn unsupported_versions_are_inconsistent_even_when_equal() {
    let base: Value = serde_json::from_slice(&golden("ir_format_1", "full.json")).unwrap();
    for n in [0u32, 3, u32::MAX] {
        assert!(!is_supported_ir_format(n), "{n}");
        let mut v = base.clone();
        v["format_version"] = json!(n);
        v["pack"]["ir_format"] = json!(n);
        assert!(!parse(&v).has_consistent_format(), "{n}");
    }
    assert!(is_supported_ir_format(1) && is_supported_ir_format(2));
}

#[test]
fn v2_golden_is_consistent() {
    let g = golden("ir_format_2", "full.json");
    let ir: Ir = serde_json::from_slice(&g).unwrap();
    assert_eq!(ir.required_ir_format(), 2);
    assert!(ir.has_consistent_format());
}
