//! IR・履歴の読み込みの判定の順序（大きさ、入れ子の深さ、JSON の解釈、型・内容）の試験（TASK-157、SAFE-20）。
//!
//! 順序の正は contract の `LOAD_STAGE_ORDER`。両方の入口で同じ順序になることを確かめる。

mod common;

use std::time::{Duration, Instant};

use common::json_input::{nested, pad_to};
use fandhe_workshop_contract::limits::{
    LimitReason, LoadEntry, LoadStage, LOAD_STAGE_ORDER, MAX_HISTORY_BYTES, MAX_IR_BYTES,
    MAX_JSON_DEPTH,
};
use fandhe_workshop_core::load_gate::{load_json, parse_json_value, LoadError};
use serde::Deserialize;

const ENTRIES: [LoadEntry; 2] = [LoadEntry::Ir, LoadEntry::History];

#[derive(Debug, Deserialize)]
struct Probe {
    #[allow(dead_code, reason = "型の段の検査のためだけに読む")]
    v: u32,
}

fn limit(entry: LoadEntry) -> usize {
    let n = match entry {
        LoadEntry::Ir => MAX_IR_BYTES,
        LoadEntry::History => MAX_HISTORY_BYTES,
    };
    usize::try_from(n).expect("limit fits usize")
}

fn run(entry: LoadEntry, bytes: &[u8]) -> Result<Probe, LoadError> {
    load_json::<Probe>(entry, bytes)
}

#[test]
fn size_is_decided_before_depth() {
    for e in ENTRIES {
        let b = pad_to(&nested(MAX_JSON_DEPTH + 1, true), limit(e) + 1);
        let err = run(e, &b).unwrap_err();
        assert_eq!(err, LoadError::Limit(LimitReason::TooLarge), "{e:?}");
        assert_eq!(err.stage(), LoadStage::Size);
    }
}

#[test]
fn size_is_decided_before_parse() {
    for e in ENTRIES {
        let b = pad_to(b"{not json", limit(e) + 1);
        assert_eq!(
            run(e, &b).unwrap_err(),
            LoadError::Limit(LimitReason::TooLarge)
        );
    }
}

#[test]
fn size_exact_is_not_rejected_by_size() {
    for e in ENTRIES {
        let b = pad_to(br#"{"v":1}"#, limit(e));
        assert_eq!(b.len(), limit(e));
        assert!(run(e, &b).is_ok(), "{e:?}");
    }
}

#[test]
fn depth_is_decided_before_parse() {
    for e in ENTRIES {
        let b = nested(MAX_JSON_DEPTH + 1, false);
        assert_eq!(
            run(e, &b).unwrap_err(),
            LoadError::Limit(LimitReason::TooDeep)
        );
    }
}

#[test]
fn deep_valid_json_is_too_deep() {
    let b = nested(MAX_JSON_DEPTH + 1, true);
    assert!(serde_json::from_slice::<serde_json::Value>(&b).is_ok());
    for e in ENTRIES {
        let err = run(e, &b).unwrap_err();
        assert_eq!(err, LoadError::Limit(LimitReason::TooDeep));
        assert_eq!(err.stage(), LoadStage::Depth);
    }
}

#[test]
fn depth_exact_is_not_rejected_by_depth() {
    for e in ENTRIES {
        let b = nested(MAX_JSON_DEPTH, true);
        let err = run(e, &b).unwrap_err();
        assert_eq!(err, LoadError::Malformed);
        assert_eq!(err.stage(), LoadStage::TypeAndContent);
        assert!(parse_json_value(e, &b).is_ok());
    }
}

#[test]
fn depth_check_precedes_serde_recursion_limit() {
    for e in ENTRIES {
        for n in [129, 10_000] {
            let b = nested(n, true);
            let t0 = Instant::now();
            let r = run(e, &b);
            let elapsed = t0.elapsed();
            assert_eq!(
                r.unwrap_err(),
                LoadError::Limit(LimitReason::TooDeep),
                "{e:?} {n}"
            );
            assert!(
                elapsed < Duration::from_secs(1),
                "{n} levels took {elapsed:?}"
            );
        }
    }
}

#[test]
fn parse_is_decided_before_type() {
    for e in ENTRIES {
        // 先頭の値は Probe の型に合わず、かつ構文も壊れている。
        assert_eq!(run(e, br#"{"v":"x","#).unwrap_err(), LoadError::NotJson);
    }
}

#[test]
fn type_is_the_last_stage() {
    for e in ENTRIES {
        let err = run(e, br#"{"v":"x"}"#).unwrap_err();
        assert_eq!(err, LoadError::Malformed);
        assert_eq!(err.stage(), LoadStage::TypeAndContent);
    }
}

#[test]
fn valid_input_passes_all_stages() {
    for e in ENTRIES {
        assert!(run(e, br#"{"v":1}"#).is_ok());
    }
}

/// 違反の組み合わせごとに、返る段は違反した段のうち契約の順で最も前のものになる。
#[test]
fn rejecting_stage_is_the_first_violated_in_contract_order() {
    for e in ENTRIES {
        // 解釈が違反なら型は評価されない。
        for size in [false, true] {
            for deep in [false, true] {
                for broken in [false, true] {
                    let mut b = if deep {
                        nested(MAX_JSON_DEPTH + 1, !broken)
                    } else if broken {
                        b"{\"v\":".to_vec()
                    } else {
                        br#"{"v":"x"}"#.to_vec()
                    };
                    if size {
                        b = pad_to(&b, limit(e) + 1);
                    }
                    let violated = |s: LoadStage| match s {
                        LoadStage::Size => size,
                        LoadStage::Depth => deep,
                        LoadStage::JsonParse => broken && !deep,
                        LoadStage::TypeAndContent => !broken && !deep,
                    };
                    let expected = LOAD_STAGE_ORDER.into_iter().find(|s| violated(*s));
                    let got = run(e, &b).unwrap_err().stage();
                    assert_eq!(
                        Some(got),
                        expected,
                        "{e:?} size={size} deep={deep} broken={broken}"
                    );
                }
            }
        }
    }
}

#[test]
fn rejection_does_not_leak_input() {
    for e in ENTRIES {
        let secret = br#"{"v":"SECRET-TOKEN-XYZ"}"#;
        let s = format!("{:?}", run(e, secret).unwrap_err());
        assert!(!s.contains("SECRET"), "{s}");
    }
}

/// 関門の 3 段目と同じ呼び方で、`serde_json` 自身の再帰の上限が残っていること（深さの検査の後ろの保険）。
#[test]
fn serde_json_own_recursion_limit_still_holds() {
    let b = nested(10_000, true);
    let err = serde_json::from_slice::<serde_json::Value>(&b).unwrap_err();
    assert!(err.to_string().contains("recursion limit"), "{err}");
}

#[test]
fn serde_json_recursion_boundary() {
    let parse = |n: usize| serde_json::from_slice::<serde_json::Value>(&nested(n, true));
    assert!(parse(127).is_ok());
    assert!(parse(129).is_err());
}
