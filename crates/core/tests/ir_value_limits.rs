//! IR の配列・連想配列の要素数と文字列の長さの上限（SAFE-25・D-45・TASK-162）の境界の試験。
//!
//! 土台は最小の IR（`min-3step.json`）で、`serde_json::Value` を書き換えて境界を作る。ちょうど（通す）・
//! 1 超過・10 倍（断る）と、判定の順（型の段が先、値の上限が参照の整合より先、大きさ・深さが最初）を固定する。
//! 呼び出し元は `cargo test` と変異注入（`SAFE25-1`・`SAFE25-2`）。判定の実体は `core::value_limits`。

mod common;

use std::time::{Duration, Instant};

use fandhe_workshop_contract::limits::{
    MAX_COLLECTION_ELEMENTS as N, MAX_IR_BYTES, MAX_STRING_CHARS as S,
};
use fandhe_workshop_core::load_ir;
use serde_json::{json, Value};

fn base() -> Value {
    let path = common::contract_sample::testdata_dir()
        .join("engine-event-consistency/test-data/min-3step.json");
    serde_json::from_slice(&std::fs::read(path).expect("read")).expect("json")
}

fn code(v: &Value) -> Result<(), &'static str> {
    let bytes = serde_json::to_vec(v).expect("to_vec");
    assert!(bytes.len() as u64 <= MAX_IR_BYTES);
    load_ir(&bytes).map(|_| ()).map_err(|e| e.reason_code())
}

fn lo(n: usize) -> Value {
    let mut v = base();
    v["nodes"][0]["step"]["lo"] = Value::Array((0..n).map(|_| Value::from("x")).collect());
    v
}

fn title(n: usize, c: &str) -> Value {
    let mut v = base();
    v["pack"]["title"] = Value::from(c.repeat(n));
    v
}

fn commands(n: usize) -> Value {
    let mut v = base();
    let m = (0..n)
        .map(|i| (format!("c{i:06}"), Value::from("builtin")))
        .collect();
    v["commands"] = Value::Object(m);
    v
}

#[test]
fn v_a_array_elements() {
    assert_eq!(code(&lo(N)), Ok(()));
    assert_eq!(code(&lo(N + 1)), Err("too_many_elements"));
    assert_eq!(code(&lo(N * 10)), Err("too_many_elements"));
}

#[test]
fn v_m_map_keys() {
    assert_eq!(code(&commands(N)), Ok(()));
    assert_eq!(code(&commands(N + 1)), Err("too_many_elements"));
}

#[test]
fn v_s_string_chars() {
    assert_eq!(code(&title(S, "a")), Ok(()));
    assert_eq!(code(&title(S + 1, "a")), Err("string_too_long"));
    assert_eq!(code(&title(S * 10, "a")), Err("string_too_long"));
}

#[test]
fn v_u_counts_code_points_not_bytes() {
    assert_eq!(code(&title(S, "あ")), Ok(()));
    assert_eq!(code(&title(S + 1, "あ")), Err("string_too_long"));
}

#[test]
fn v_k_map_key_length() {
    let mut v = base();
    let mut m = v["commands"].as_object().expect("object").clone();
    m.insert("k".repeat(S + 1), Value::from("builtin"));
    v["commands"] = Value::Object(m);
    assert_eq!(code(&v), Err("string_too_long"));
}

#[test]
fn v_o1_type_mismatch_wins() {
    let mut v = lo(N + 1);
    v["format_version"] = json!("one");
    assert_eq!(code(&v), Err("malformed"));
}

#[test]
fn v_o2_value_limit_wins_over_references() {
    let mut v = lo(N + 1);
    v["starts"][0]["node"] = json!("main/nowhere");
    assert_eq!(code(&v), Err("too_many_elements"));
    let mut v = lo(N);
    v["starts"][0]["node"] = json!("main/nowhere");
    assert_eq!(code(&v), Err("malformed"));
}

#[test]
fn v_o3_size_and_depth_come_first() {
    let big = vec![b' '; MAX_IR_BYTES as usize + 1];
    assert_eq!(load_ir(&big).unwrap_err().reason_code(), "too_large");
    let deep = format!("{}{}", "[".repeat(65), "]".repeat(65));
    assert_eq!(
        load_ir(deep.as_bytes()).unwrap_err().reason_code(),
        "too_deep"
    );
}

#[test]
fn v_c_error_carries_no_input() {
    let mut v = title(S + 1, "SECRET");
    v["pack"]["title"] = Value::from("SECRET".repeat(S));
    let e = load_ir(&serde_json::to_vec(&v).expect("to_vec")).unwrap_err();
    assert!(!e.to_string().contains("SECRET"));
    assert_eq!(e.to_string(), "IR rejected: string_too_long");
}

#[test]
fn v_t_refusal_is_fast() {
    let bytes = serde_json::to_vec(&lo(N + 1)).expect("to_vec");
    let t = Instant::now();
    assert!(load_ir(&bytes).is_err());
    assert!(t.elapsed() < Duration::from_secs(5));
}
