//! 構造の合わない巨大な入力の拒否（SAFE-24・TASK-161）の試験。
//!
//! 100 万・500 万要素の配列、巨大な文字列、巨大な連想配列は、大きさ・深さの段（段 1・2）を通る大きさに収め、
//! 形の段で理由つきに断られることを固定する。IR は `malformed`、履歴は `missing_version`（版の欄が無い）である。
//! 呼び出し元は `cargo test` と変異注入（`SAFE24-1`）で、ブラウザの試験 `shape-mismatch` より速く失敗を示すために置く。
//! 時間の表明は debug ビルドかつ並列実行の CI でも落ちない 5 秒にし（1 秒は CI の M で超えた）。1 秒の予算と実測はブラウザの試験が持つ。
//! 形の合う対照（`step.lo` が 100,000 要素）は、将来の要素数の上限（SAFE-25）が入っても開き続ける最大の値である。

mod common;

use std::fmt::Write as _;
use std::time::{Duration, Instant};

use fandhe_workshop_contract::limits::{MAX_HISTORY_BYTES, MAX_IR_BYTES};
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::load_ir;
use serde_json::Value;

const BUDGET: Duration = Duration::from_secs(5);

/// `[0,0,...]` を `n` 要素で作る。
fn array_of(n: usize) -> Vec<u8> {
    let mut v = Vec::with_capacity(2 * n + 1);
    v.push(b'[');
    for i in 0..n {
        if i > 0 {
            v.push(b',');
        }
        v.push(b'0');
    }
    v.push(b']');
    v
}

/// 全体がちょうど `total` バイトの JSON の文字列。
fn string_of(total: usize) -> Vec<u8> {
    let mut v = vec![b'a'; total];
    v[0] = b'"';
    v[total - 1] = b'"';
    v
}

/// `{"k0000000":0,...}` を `n` キーで作る（`format_version`・`versions` を持たない）。
fn map_of(n: usize) -> Vec<u8> {
    let mut s = String::with_capacity(n * 13 + 2);
    s.push('{');
    for i in 0..n {
        if i > 0 {
            s.push(',');
        }
        write!(s, "\"k{i:07}\":0").expect("write to String");
    }
    s.push('}');
    s.into_bytes()
}

fn shapes(string_total: usize, map_keys: usize) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("A1", array_of(1_000_000)),
        ("A5", array_of(5_000_000)),
        ("S", string_of(string_total)),
        ("M", map_of(map_keys)),
    ]
}

#[test]
fn ir_shapes_are_malformed_within_budget() {
    let max = usize::try_from(MAX_IR_BYTES).expect("usize");
    for (id, bytes) in shapes(max, 1_000_000) {
        assert!(bytes.len() <= max, "{id}: within the IR size limit");
        let t = Instant::now();
        let err = load_ir(&bytes).expect_err(id);
        assert!(t.elapsed() < BUDGET, "{id}: refused within the budget");
        assert_eq!(err.reason_code(), "malformed", "{id}");
    }
}

#[test]
fn history_shapes_are_missing_version_within_budget() {
    let max = usize::try_from(MAX_HISTORY_BYTES).expect("usize");
    for (id, bytes) in shapes(max, 800_000) {
        assert!(bytes.len() <= max, "{id}: within the history size limit");
        let t = Instant::now();
        let r = parse_history_json(&bytes).and_then(parse_history_envelope);
        assert!(t.elapsed() < BUDGET, "{id}: refused within the budget");
        assert_eq!(r.err(), Some(LoadReason::MissingVersion), "{id}");
    }
}

#[test]
fn matching_ir_with_100000_element_array_loads() {
    let path = common::contract_sample::testdata_dir()
        .join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json");
    let lo: Vec<Value> = (0..100_000).map(|_| Value::from("x")).collect();
    v["nodes"][0]["step"]["lo"] = Value::Array(lo);
    let bytes = serde_json::to_vec(&v).expect("to_vec");
    assert!(bytes.len() as u64 <= MAX_IR_BYTES);
    assert!(load_ir(&bytes).is_ok());
}
