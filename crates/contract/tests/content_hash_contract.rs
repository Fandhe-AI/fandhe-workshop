//! `content_hash` の求め方の契約適合試験（TASK-180 1/2、D-51・VER-12）。
//!
//! サンプル（IR とハッシュの対）は `tests/data/content_hash_1/expected.json`。期待値は Rust の実装から
//! 求めず、Python で本文の木を除いた compact の JSON の SHA-256 を独立に計算して固定した。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::content_hash::{content_hash, CONTENT_HASH_SPEC_VERSION};
use fandhe_workshop_contract::history_version::ContentHash;
use fandhe_workshop_contract::ir::Ir;
use serde_json::Value;

fn data_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data")
}

/// `tests/data` の下の相対パスだけを受ける（絶対パス・`..` は拒否する）。
fn read(rel: &str) -> Vec<u8> {
    let p = Path::new(rel);
    assert!(
        p.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe sample path: {rel}"
    );
    std::fs::read(data_dir().join(p)).unwrap_or_else(|_| panic!("cannot read {rel}"))
}

fn ir(rel: &str) -> Ir {
    serde_json::from_slice(&read(rel)).unwrap()
}

fn hash(ir: &Ir) -> String {
    content_hash(ir).unwrap().as_str().to_owned()
}

fn expected() -> Vec<(String, String)> {
    let v: Value = serde_json::from_slice(&read("content_hash_1/expected.json")).unwrap();
    assert_eq!(v["spec_version"], CONTENT_HASH_SPEC_VERSION);
    v["samples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| {
            (
                s["ir"].as_str().unwrap().to_owned(),
                s["content_hash"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn strip_bodies(ir: &mut Ir) {
    for n in &mut ir.nodes {
        n.body_doc = None;
        n.body_levels_doc.clear();
    }
}

#[test]
fn spec_version_is_1() {
    assert_eq!(CONTENT_HASH_SPEC_VERSION, 1);
}

#[test]
fn samples_match_expected_and_are_well_formed() {
    let samples = expected();
    assert!(samples.len() >= 6);
    for (rel, want) in samples {
        assert!(
            ContentHash::parse(&want).is_some(),
            "{rel}: bad expectation"
        );
        assert_eq!(hash(&ir(&rel)), want, "{rel}");
    }
}

#[test]
fn body_only_edit_keeps_the_hash() {
    let before = ir("ir_format_2/full.json");
    let after = ir("content_hash_1/body-edited.json");
    assert_ne!(before, after, "the sample must really differ in the bodies");
    assert!(after.nodes.iter().any(|n| n.body_doc.is_some()));
    assert_eq!(hash(&before), hash(&after));
}

#[test]
fn hash_equals_the_hash_of_the_ir_without_body_trees() {
    let full = ir("ir_format_2/full.json");
    assert!(full.nodes.iter().any(|n| n.body_doc.is_some()));
    assert!(full.nodes.iter().any(|n| !n.body_levels_doc.is_empty()));
    let mut stripped = full.clone();
    strip_bodies(&mut stripped);
    assert_eq!(stripped.format_version, full.format_version);
    assert_eq!(hash(&full), hash(&stripped));
}

#[test]
fn adding_a_field_in_a_value_area_changes_the_hash() {
    let before = ir("ir_format_2/full.json");
    let after = ir("content_hash_1/value-field-added.json");
    assert_ne!(before, after);
    assert_ne!(hash(&before), hash(&after));
}

#[test]
fn changing_typed_fields_changes_the_hash() {
    let base = ir("ir_format_2/full.json");
    let mut a = base.clone();
    a.pack.version.push_str("-x");
    let mut b = base.clone();
    b.nodes[0].step.title.push('x');
    let h0 = hash(&base);
    assert_ne!(hash(&a), h0);
    assert_ne!(hash(&b), h0);
    assert_ne!(hash(&a), hash(&b));
}

/// 未決（D-51 の残り）: 練習データの内容の木は暫定でハッシュに含める。決定が変わったら仕様の版を上げて直す。
#[test]
fn fixture_docs_are_hashed_pending_decision() {
    let before = ir("ir_format_2/full.json");
    let after = ir("content_hash_1/fixture-doc-edited.json");
    assert_ne!(before.fixture_docs, after.fixture_docs);
    assert_ne!(hash(&before), hash(&after));
}

#[test]
fn hash_does_not_depend_on_notation_or_key_order() {
    let rel = "ir_format_2/full.json";
    let want = hash(&ir(rel));
    let value: Value = serde_json::from_slice(&read(rel)).unwrap();
    // 整形と compact のどちらから読んでも同じ。
    let pretty: Ir = serde_json::from_slice(&serde_json::to_vec_pretty(&value).unwrap()).unwrap();
    let compact: Ir = serde_json::from_slice(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(hash(&pretty), want);
    assert_eq!(hash(&compact), want);
    // `Value` の領域のキーの順を入れ替えた入力でも同じ。`preserve_order` が有効になると失敗する。
    let reversed = r#"{"b":1,"a":2}"#;
    let mut flipped = ir(rel);
    let mut sorted = ir(rel);
    flipped
        .commands
        .insert("k".into(), serde_json::from_str(reversed).unwrap());
    sorted.commands.insert(
        "k".into(),
        serde_json::from_str(r#"{"a":2,"b":1}"#).unwrap(),
    );
    assert_eq!(hash(&flipped), hash(&sorted));
    // 同じ入力で 2 回求めると一致し、入力は変わらない。
    let input = ir(rel);
    let snapshot = input.clone();
    assert_eq!(hash(&input), hash(&input));
    assert_eq!(input, snapshot);
}

#[test]
fn format_versions_are_distinguished() {
    let v1 = ir("ir_format_1/minimal.json");
    let mut as_v2 = v1.clone();
    as_v2.format_version = 2;
    as_v2.pack.ir_format = 2;
    assert_ne!(hash(&v1), hash(&as_v2));
}
