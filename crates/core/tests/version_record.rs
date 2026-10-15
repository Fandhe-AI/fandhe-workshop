//! 履歴の 4 軸の版の記録と欠落の拒否の契約適合試験（TASK-173 2/2、VER-1）。
//!
//! 理由コードの文字列は検査しない（TASK-310）。分類 `ReasonKind::MissingVersion` だけを確かめる。
//! データは `testdata/` と contract の golden から読み、作業ツリーは書き換えない。

use std::fs;
use std::path::{Path, PathBuf};

use fandhe_workshop_contract::history_version::{
    ContentHash, HistoryVersions, ReasonKind, Version3, VersionAxis,
};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_core::load_gate::{parse_json_value, LoadError};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::version_record::{
    check_versions_present, record_versions, record_versions_with, MissingVersion, RecordError,
    ENGINE_VERSION,
};
use serde_json::{json, Value};

const HASH: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn hash() -> ContentHash {
    ContentHash::parse(HASH).unwrap()
}

fn base_ir() -> Ir {
    let bytes = fs::read(root().join("testdata/input-field-ime/test-data/ch1-pack.json")).unwrap();
    load_ir(&bytes).unwrap()
}

fn engine(s: &str) -> Version3 {
    Version3::parse(s).unwrap()
}

#[test]
fn t1_engine_version_is_parsable() {
    assert!(Version3::parse(ENGINE_VERSION).is_some());
}

#[test]
fn t2_default_vocab_record() {
    let ir = base_ir();
    let v = record_versions(&ir, &hash()).unwrap();
    assert_eq!(v.engine, ENGINE_VERSION);
    assert_eq!(v.vocab, "1.0");
    assert_eq!(v.ir_format, 1);
    assert_eq!(v.content, ir.pack.version);
    assert!(v.refs.is_empty());
    assert_eq!(v.content_hash, HASH);
    assert!(v.parse().is_ok());
}

#[test]
fn t3_four_axes_present_in_order() {
    let v = record_versions(&base_ir(), &hash()).unwrap();
    let json = serde_json::to_value(&v).unwrap();
    let keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    // serde_json の既定は辞書順のため、直列化した文字列の出現順で確かめる。
    let text = serde_json::to_string(&v).unwrap();
    let order = [
        "engine",
        "ir_format",
        "vocab",
        "content",
        "content_hash",
        "refs",
    ];
    let mut last = 0;
    for k in order {
        let at = text.find(&format!("\"{k}\":")).unwrap();
        assert!(at >= last, "{k} out of order");
        last = at;
    }
    assert_eq!(keys.len(), 6);
}

#[test]
fn t4_matches_contract_golden() {
    let mut ir = base_ir();
    ir.pack.version = "2.0.1".to_owned();
    ir.pack.vocab = Some(json!("1.3"));
    ir.refs = vec![
        json!({"name": "repo-b", "commit": "fedcba9876543210fedcba9876543210fedcba98", "link": "x"}),
        json!({"name": "repo-a", "commit": "0123456789abcdef0123456789abcdef01234567"}),
    ];
    let v = record_versions_with(&ir, engine("1.2.3"), &hash()).unwrap();
    let mut got = serde_json::to_vec_pretty(&v).unwrap();
    got.push(b'\n');
    let want =
        fs::read(root().join("crates/contract/tests/data/history_versions_1/full.json")).unwrap();
    assert_eq!(got, want);
}

#[test]
fn t5_vocab_change_keeps_ir_format() {
    let mut a = base_ir();
    let mut b = base_ir();
    a.pack.vocab = Some(json!("1.0"));
    b.pack.vocab = Some(json!("1.2"));
    let va = record_versions_with(&a, engine("0.0.0"), &hash()).unwrap();
    let vb = record_versions_with(&b, engine("0.0.0"), &hash()).unwrap();
    assert_eq!(va.ir_format, vb.ir_format);
    let diffs = va.diff(&vb);
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].axis, VersionAxis::Vocab);
}

#[test]
fn t6_fail_closed() {
    let rec = |ir: &Ir| record_versions_with(ir, engine("0.0.0"), &hash());
    for vocab in [
        json!({"v": 1}),
        json!(1),
        json!("1"),
        json!("01.0"),
        json!(null),
    ] {
        let mut ir = base_ir();
        ir.pack.vocab = Some(vocab);
        assert_eq!(rec(&ir), Err(RecordError::Vocab));
    }
    let mut ir = base_ir();
    ir.pack.version = "1.0".to_owned();
    assert_eq!(rec(&ir), Err(RecordError::Content));

    let mut ir = base_ir();
    ir.pack.ir_format = 2;
    assert_eq!(rec(&ir), Err(RecordError::IrFormatMismatch));

    let c = "0123456789abcdef0123456789abcdef01234567";
    for (refs, want) in [
        (vec![json!("x")], RecordError::Ref { index: 0 }),
        (vec![json!({"name": "r"})], RecordError::Ref { index: 0 }),
        (
            vec![json!({"name": 1, "commit": c})],
            RecordError::Ref { index: 0 },
        ),
        (
            vec![
                json!({"name": "r", "commit": c}),
                json!({"name": "r", "commit": c}),
            ],
            RecordError::DuplicateRef { index: 1 },
        ),
    ] {
        let mut ir = base_ir();
        ir.refs = refs;
        let err = rec(&ir).unwrap_err();
        assert_eq!(err, want);
        assert!(!err.to_string().contains(c));
    }
}

#[test]
fn t7_missing_versions_is_rejected() {
    let path = root()
        .join("testdata/save-restore-versions/test-data-addendum/histories/b-5-no-versions.json");
    let v: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let err = check_versions_present(&v).unwrap_err();
    assert_eq!(err, MissingVersion);
    assert_eq!(err.kind(), ReasonKind::MissingVersion);
    for v in [
        json!({"versions": null}),
        json!({"versions": []}),
        json!({"versions": "1"}),
        json!([]),
        json!({}),
    ] {
        assert_eq!(check_versions_present(&v), Err(MissingVersion));
    }
}

#[test]
fn t8_malformed_versions_table_is_not_missing() {
    let no_engine = json!({"versions": {
        "ir_format": 1, "vocab": "1.0", "content": "1.0.0", "content_hash": HASH, "refs": {}
    }});
    let unknown = json!({"versions": {
        "engine": "0.0.0", "ir_format": 1, "vocab": "1.0", "content": "1.0.0",
        "content_hash": HASH, "refs": {}, "extra": 1
    }});
    for v in [no_engine, unknown] {
        assert_eq!(check_versions_present(&v), Ok(()));
        assert!(serde_json::from_value::<HistoryVersions>(v["versions"].clone()).is_err());
    }
}

/// 解釈 → 欠落 → 型 → 書式の順に通した結果。
fn load_versions(bytes: &[u8]) -> Result<HistoryVersions, String> {
    let v = parse_json_value(LoadEntry::History, bytes).map_err(|e| format!("{e:?}"))?;
    check_versions_present(&v).map_err(|_| "missing".to_owned())?;
    let hv: HistoryVersions =
        serde_json::from_value(v["versions"].clone()).map_err(|_| "malformed".to_owned())?;
    hv.parse().map_err(|_| "unreadable".to_owned())?;
    Ok(hv)
}

#[test]
fn t9_stage_order_over_poc_histories() {
    let dir = root().join("testdata/save-restore-versions/test-data-addendum/histories");
    let mut seen = 0;
    for e in fs::read_dir(dir).unwrap().flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let bytes = fs::read(e.path()).unwrap();
        let got = load_versions(&bytes);
        match name.as_str() {
            "b-2-truncated.json" | "b-3-not-json.json" => {
                assert_eq!(
                    got.unwrap_err(),
                    format!("{:?}", LoadError::NotJson),
                    "{name}"
                );
            }
            "b-5-no-versions.json" => assert_eq!(got.unwrap_err(), "missing"),
            _ => assert!(got.is_ok(), "{name}: {got:?}"),
        }
        seen += 1;
    }
    assert!(seen >= 10);
}

#[test]
fn t10_round_trip() {
    let v = record_versions_with(&base_ir(), engine("0.0.0"), &hash()).unwrap();
    let bytes = serde_json::to_vec(&json!({ "versions": v })).unwrap();
    assert_eq!(load_versions(&bytes).unwrap(), v);
}

/// TASK-180: 仕様で求めたハッシュを版の記録に渡すと、サンプルの期待と一致し、版の書式の検査を通る。
#[test]
fn record_versions_accepts_the_spec_content_hash() {
    let path = root().join("crates/contract/tests/data/ir_format_1/minimal.json");
    let ir: Ir = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let h = fandhe_workshop_contract::content_hash::content_hash(&ir).unwrap();
    assert_eq!(
        h.as_str(),
        "87ba748bbe525761ca30428d0834dcafbf6cfa3962157da260ce8840bfb51834"
    );
    let v = record_versions_with(&ir, engine("1.2.3"), &h).unwrap();
    assert_eq!(v.content_hash, h.as_str());
    assert_eq!(v.parse().unwrap().content_hash, h);
}
