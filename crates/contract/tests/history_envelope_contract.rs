//! `HistoryEnvelope` の直列化の形・拒否・往復の適合試験（TASK-309 2/3）。
//! 操作 `E`・写し `S` には core に依存しない試験用の代役を使う。

use std::collections::BTreeMap;

use fandhe_workshop_contract::history::{
    HistoryEnvelope, HistoryOp, HISTORY_CONTRACT_VERSION, HISTORY_FORMAT_NAME,
};
use fandhe_workshop_contract::history_version::HistoryVersions;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubOp {
    Input(String),
    Tick(u32),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StubSnapshot {
    log: Vec<String>,
    generation: u64,
}

type Env = HistoryEnvelope<StubOp, StubSnapshot>;

fn versions() -> HistoryVersions {
    HistoryVersions {
        engine: "0.1.0".into(),
        ir_format: 1,
        vocab: "1.0".into(),
        content: "0.1.0".into(),
        content_hash: "0".repeat(64),
        refs: BTreeMap::new(),
    }
}

fn sample(ops: Vec<HistoryOp<StubOp>>, log: Vec<String>) -> Env {
    HistoryEnvelope::new(
        "p".into(),
        versions(),
        "main".into(),
        ops,
        StubSnapshot { log, generation: 1 },
    )
}

fn valid_value() -> Value {
    serde_json::to_value(sample(vec![], vec![])).unwrap()
}

#[test]
fn new_fills_format_and_version() {
    let e = sample(vec![], vec![]);
    assert_eq!(e.format, HISTORY_FORMAT_NAME);
    assert_eq!(e.history_format, HISTORY_CONTRACT_VERSION);
}

#[test]
fn minimal_envelope_has_exact_key_order() {
    let s = serde_json::to_string(&sample(vec![], vec![])).unwrap();
    let h = "0".repeat(64);
    let expected = format!(
        r#"{{"format":"fandhe-workshop-history","history_format":1,"pack_id":"p","versions":{{"engine":"0.1.0","ir_format":1,"vocab":"1.0","content":"0.1.0","content_hash":"{h}","refs":{{}}}},"start":"main","ops":[],"snapshot":{{"log":[],"generation":1}}}}"#
    );
    assert_eq!(s, expected);
}

#[test]
fn roundtrip_keeps_ops_and_log_order() {
    let ops = vec![
        HistoryOp::Draft("a".into()),
        HistoryOp::Engine(StubOp::Tick(1)),
        HistoryOp::Submit("b".into()),
        HistoryOp::Reload,
        HistoryOp::Engine(StubOp::Input("c".into())),
        HistoryOp::Draft("d".into()),
    ];
    let log: Vec<String> = ["e5", "e1", "e4", "e2", "e3", "e0"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let e = sample(ops, log);
    let s = serde_json::to_string(&e).unwrap();
    let back: Env = serde_json::from_str(&s).unwrap();
    assert_eq!(back, e);
    assert_eq!(serde_json::to_string(&back).unwrap(), s);
}

#[test]
fn missing_field_is_rejected() {
    for key in [
        "format",
        "history_format",
        "pack_id",
        "versions",
        "start",
        "ops",
        "snapshot",
    ] {
        let mut v = valid_value();
        v.as_object_mut().unwrap().remove(key);
        assert!(serde_json::from_value::<Env>(v).is_err(), "{key}");
    }
}

#[test]
fn ops_only_envelope_is_rejected() {
    let v = json!({"ops": [{"submit": "x"}]});
    assert!(serde_json::from_value::<Env>(v).is_err());
}

#[test]
fn unknown_field_and_type_mismatch_are_rejected() {
    let mut v = valid_value();
    v["extra"] = Value::Null;
    assert!(serde_json::from_value::<Env>(v).is_err());

    let mut v = valid_value();
    v["history_format"] = json!("1");
    assert!(serde_json::from_value::<Env>(v).is_err());

    let mut v = valid_value();
    v["ops"] = json!({});
    assert!(serde_json::from_value::<Env>(v).is_err());

    let mut v = valid_value();
    v["ops"] = json!([{"fast_forward": true}]);
    assert!(serde_json::from_value::<Env>(v).is_err());
}

#[test]
fn format_and_version_values_are_not_checked_here() {
    // 値の照合は TASK-310・TASK-174 の責務。型は受けるだけ。
    let mut v = valid_value();
    v["format"] = json!("other");
    v["history_format"] = json!(2);
    let e: Env = serde_json::from_value(v).unwrap();
    assert_eq!(e.format, "other");
    assert_eq!(e.history_format, 2);
}

#[test]
fn user_strings_are_kept_verbatim() {
    let nasty = "<script>alert(\"x\")</script>'日本語";
    let e: Env = HistoryEnvelope::new(
        nasty.to_string(),
        versions(),
        nasty.to_string(),
        vec![HistoryOp::Draft(nasty.to_string())],
        StubSnapshot {
            log: vec![],
            generation: 0,
        },
    );
    let back: Env = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
    assert_eq!(back.pack_id, nasty);
    assert_eq!(back.start, nasty);
    assert_eq!(back.ops, vec![HistoryOp::Draft(nasty.to_string())]);
}
