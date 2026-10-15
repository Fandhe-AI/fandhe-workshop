//! 本文の木（`Doc`）の読み込みの検査（SAFE-12・SAFE-20。TASK-150 2/3）の試験。
//!
//! 変換器を通さずに手作りした木（T-05・06・07・10・11・12・15・16。試験データは spec の写し）を、読み込みの関門
//! （`load_json::<Doc>`）と `load_ir` に通し、語彙の外の種別・未知の項目・木の版の違いが型・内容の段で、
//! 深い入れ子が深さの段で拒否されることを固定する。
//! 試験では `unwrap`・`expect` を使ってよい（失敗はそのまま試験の失敗になる）。

use std::path::PathBuf;

use fandhe_workshop_contract::doc::Doc;
use fandhe_workshop_contract::limits::{LoadEntry, LoadStage};
use fandhe_workshop_core::load_gate::{load_json, LoadError};
use fandhe_workshop_core::{load_ir, IrLoadError};
use serde_json::Value;

fn testdata_text(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/untrusted-input-safety")
        .join(rel);
    std::fs::read_to_string(path).expect("read testdata")
}

/// 試験データの `gen.kind == "quote_nest"` と同じ形。
fn quote_nest(n: usize) -> String {
    let open = "{\"t\":\"quote\",\"children\":[".repeat(n);
    let close = "]}".repeat(n);
    format!("{{\"version\":1,\"blocks\":[{open}{close}]}}")
}

fn case(id: &str) -> String {
    for rel in [
        "test-data-addendum/tree-cases.json",
        "test-data-addendum-2/tree-cases-2.json",
    ] {
        let v: Value = serde_json::from_str(&testdata_text(rel)).expect("parse testdata");
        for c in v["cases"].as_array().expect("cases") {
            if c["id"] == id {
                return match c.get("tree") {
                    Some(tree) => tree.to_string(),
                    None => quote_nest(c["gen"]["n"].as_u64().expect("n") as usize),
                };
            }
        }
    }
    panic!("case {id} not found");
}

fn gate_stage(json: &str) -> Option<LoadStage> {
    match load_json::<Doc>(LoadEntry::Ir, json.as_bytes()) {
        Ok(_) => None,
        Err(e) => Some(e.stage()),
    }
}

#[test]
fn 語彙の外の種別と未知の項目と木の版の違いは型の段で拒否される() {
    for id in ["T-05", "T-06", "T-07", "T-10"] {
        let err = load_json::<Doc>(LoadEntry::Ir, case(id).as_bytes()).expect_err(id);
        assert!(matches!(err, LoadError::Malformed), "{id}: {err:?}");
        assert_eq!(err.stage(), LoadStage::TypeAndContent, "{id}");
    }
}

#[test]
fn 深い入れ子は深さの段で拒否される() {
    // 引用 n 段は JSON で 2 + 2n 段。関門の上限（64 段）を超える 50・60・100・10,000 段はすべて深さの段で止まる。
    for id in ["T-11", "T-12", "T-15", "T-16"] {
        assert_eq!(gate_stage(&case(id)), Some(LoadStage::Depth), "{id}");
    }
}

#[test]
fn 関門を通る最も深い引用は読める() {
    // 対照: n = 31 は JSON で 64 段（ちょうど上限）。拒否が深さによるもので、読み込みが常に失敗するわけではない。
    assert_eq!(gate_stage(&quote_nest(31)), None);
    assert_eq!(gate_stage(&quote_nest(32)), Some(LoadStage::Depth));
}

#[test]
fn 版_1_の無害な木は読める() {
    let ok = r#"{"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":"x"}]}]}"#;
    assert_eq!(gate_stage(ok), None);
}

fn full_ir() -> Value {
    let text = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../contract/tests/data/ir_format_2/full.json"),
    )
    .expect("read ir");
    let mut ir: Value = serde_json::from_str(&text).expect("parse ir");
    // 契約の試験データは参照先の節が 1 つ足りない。参照を自身に向けて、読み込める IR にする。
    ir["nodes"][0]["step"]["next"][0]["to"] = Value::from("main/a");
    ir
}

fn load_value(v: &Value) -> Result<(), IrLoadError> {
    load_ir(v.to_string().as_bytes()).map(|_| ())
}

#[test]
fn ir_のどの木の版が違っても_load_ir_が拒否する() {
    // 対照: 元の IR は読める。
    load_value(&full_ir()).expect("baseline IR must load");

    // 木が載る 3 か所（body_doc・body_levels_doc・fixture_docs）を 1 つずつ版 2 に書き換える。
    let mut a = full_ir();
    a["nodes"][0]["body_doc"]["version"] = Value::from(2);
    let mut b = full_ir();
    let level = b["nodes"][0]["body_levels_doc"]
        .as_object_mut()
        .expect("body_levels_doc")
        .values_mut()
        .next()
        .expect("one level");
    level["version"] = Value::from(2);
    let mut c = full_ir();
    let fixture = c["fixture_docs"]
        .as_object_mut()
        .expect("fixture_docs")
        .values_mut()
        .next()
        .expect("one fixture");
    fixture["version"] = Value::from(2);

    for (name, ir) in [("body_doc", a), ("body_levels_doc", b), ("fixture_docs", c)] {
        let err = load_value(&ir).expect_err(name);
        assert!(matches!(err, IrLoadError::Json(_)), "{name}: {err:?}");
    }
}
