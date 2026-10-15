//! 履歴の包みのサンプル（`testdata/contract/history/`、TASK-309 3/3）を、core に依存しない形で読む試験。
//!
//! 操作 `E`・写し `S` を `serde_json::Value` にして、最上位のキーの順・`ops` のタグ・必須欄・未知の欄の拒否を
//! 確かめる。バイト一致と core の型での往復は core 側の `history_envelope_conformance.rs` が担う
//! （`Value` の写像はキーを並べ替えるため、ここではバイト一致を主張しない）。
//! 履歴は未信頼入力として扱う。試験では `unwrap` を使ってよい。

use std::path::{Component, Path, PathBuf};

use fandhe_workshop_contract::history::{
    HistoryEnvelope, EXPORT_FORM, HISTORY_CONTRACT_VERSION, HISTORY_FORMAT_NAME,
};
use serde_json::Value;

const SAMPLES: [&str; 3] = ["empty", "two-rounds", "draft-rejected"];
const TOP_KEYS: [&str; 7] = [
    "format",
    "history_format",
    "pack_id",
    "versions",
    "start",
    "ops",
    "snapshot",
];
const OP_TAGS: [&str; 6] = ["submit", "draft", "reload", "input", "tick", "control"];

type Env = HistoryEnvelope<Value, Value>;

fn read(name: &str) -> String {
    let p = Path::new(name);
    assert!(
        p.components().all(|c| matches!(c, Component::Normal(_))),
        "unsafe sample name: {name}"
    );
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/history")
        .join(format!("{name}.json"));
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn samples_parse_without_core() {
    for name in SAMPLES {
        let env: Env = serde_json::from_str(&read(name)).unwrap();
        assert_eq!(env.format, HISTORY_FORMAT_NAME, "{name}");
        assert_eq!(env.history_format, HISTORY_CONTRACT_VERSION, "{name}");
        env.versions.parse().unwrap();
    }
}

#[test]
fn top_level_keys_follow_the_declared_order() {
    for name in SAMPLES {
        let text = read(name);
        let mut last = 0;
        for key in TOP_KEYS {
            let pos = text
                .find(&format!("\n  \"{key}\":"))
                .unwrap_or_else(|| panic!("{name}: missing {key}"));
            assert!(pos >= last, "{name}: {key} is out of order");
            last = pos;
        }
    }
}

#[test]
fn ops_use_only_the_six_tags() {
    for name in SAMPLES {
        let v: Value = serde_json::from_str(&read(name)).unwrap();
        for op in v["ops"].as_array().unwrap() {
            let tag = match op {
                Value::String(s) => s.as_str(),
                Value::Object(m) => {
                    assert_eq!(m.len(), 1, "{name}: an op has one tag");
                    m.keys().next().unwrap().as_str()
                }
                other => panic!("{name}: unexpected op {other}"),
            };
            assert!(OP_TAGS.contains(&tag), "{name}: {tag}");
        }
    }
}

#[test]
fn ops_only_and_unknown_fields_are_rejected() {
    assert_eq!(format!("{EXPORT_FORM:?}"), "OpsAndSnapshot");
    for name in SAMPLES {
        let mut v: Value = serde_json::from_str(&read(name)).unwrap();
        v.as_object_mut().unwrap().remove("snapshot");
        assert!(serde_json::from_value::<Env>(v).is_err(), "{name}");

        let mut v: Value = serde_json::from_str(&read(name)).unwrap();
        v["extra"] = Value::Null;
        assert!(serde_json::from_value::<Env>(v).is_err(), "{name}");
    }
}
