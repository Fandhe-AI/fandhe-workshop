//! 解釈の前の入れ子の深さの検査（TASK-157、SAFE-20・SAFE-42）の試験。

use std::time::{Duration, Instant};

use fandhe_workshop_contract::limits::{LimitReason, LoadStage, MAX_JSON_DEPTH};
use fandhe_workshop_core::json_depth::{check_json_depth, json_depth};

fn nested_arrays(n: usize) -> Vec<u8> {
    let mut v = vec![b'['; n];
    v.extend(vec![b']'; n]);
    v
}

/// オブジェクトと配列を交互に入れ子にした n 段の JSON。
fn mixed(n: usize) -> String {
    let mut open = String::new();
    let mut close = Vec::new();
    for i in 0..n {
        if i % 2 == 0 {
            open.push_str("{\"a\":");
            close.push('}');
        } else {
            open.push('[');
            close.push(']');
        }
    }
    open.push('1');
    close.reverse();
    open.extend(close);
    open
}

/// 上限ちょうどは深さでは断らない（R2）。
#[test]
fn exact_limit_is_not_rejected_by_depth() {
    let b = nested_arrays(MAX_JSON_DEPTH);
    assert_eq!(check_json_depth(&b), Ok(()));
    assert_eq!(json_depth(&b), MAX_JSON_DEPTH);
}

/// 上限を 1 段超えると too_deep（R1）。
#[test]
fn one_over_limit_is_too_deep() {
    assert_eq!(
        check_json_depth(&nested_arrays(MAX_JSON_DEPTH + 1)),
        Err(LimitReason::TooDeep)
    );
}

/// 65 段は serde_json の解釈だけでは通る。深さの検査が解釈の前に断ることの証拠（R1）。
#[test]
fn too_deep_is_decided_before_parse() {
    let b = nested_arrays(MAX_JSON_DEPTH + 1);
    assert!(serde_json::from_slice::<serde_json::Value>(&b).is_ok());
    assert_eq!(check_json_depth(&b), Err(LimitReason::TooDeep));
}

/// 10,000 段でも 1 秒未満で理由が出る（R3）。
#[test]
fn nested_10000_is_too_deep_within_one_second() {
    let b = nested_arrays(10_000);
    let t = Instant::now();
    let r = check_json_depth(&b);
    assert!(t.elapsed() < Duration::from_secs(1));
    assert_eq!(r, Err(LimitReason::TooDeep));
}

/// 文字列の中の括弧は数えない。エスケープされた引用符の後も文字列の中として扱う。
#[test]
fn brackets_inside_strings_are_not_counted() {
    let s = format!("{{\"a\":\"{}\"}}", "[".repeat(10_000));
    assert_eq!(json_depth(s.as_bytes()), 1);
    let e = format!("{{\"a\":\"\\\"{}\"}}", "[".repeat(10_000));
    assert_eq!(json_depth(e.as_bytes()), 1);
    // エスケープされた `\` の直後の引用符は文字列を閉じる。
    let c = "{\"a\":\"\\\\\",\"b\":[[1]]}";
    assert_eq!(json_depth(c.as_bytes()), 3);
}

/// 閉じていない・余分な閉じ括弧の入力でも止まり、桁あふれしない。
#[test]
fn unclosed_and_stray_brackets_do_not_hang_or_underflow() {
    assert_eq!(json_depth(&vec![b'['; 100_000]), 100_000);
    assert_eq!(json_depth(b"]]]][["), 2);
    assert_eq!(json_depth(b"]]]]["), 1);
}

/// オブジェクトも配列も段として数える。
#[test]
fn object_and_array_mix_counts_levels() {
    let ok = mixed(MAX_JSON_DEPTH);
    assert_eq!(json_depth(ok.as_bytes()), MAX_JSON_DEPTH);
    assert_eq!(check_json_depth(ok.as_bytes()), Ok(()));
    let over = mixed(MAX_JSON_DEPTH + 1);
    assert_eq!(check_json_depth(over.as_bytes()), Err(LimitReason::TooDeep));
}

/// 日本語・絵文字は段数に影響しない。
#[test]
fn multibyte_utf8_does_not_affect_depth() {
    let s = "{\"題\":[\"日本語[[[🎉\",{\"x\":\"]]]\"}]}";
    assert_eq!(json_depth(s.as_bytes()), 3);
}

/// 返る理由は contract の値である。
#[test]
fn reason_code_is_contract_value() {
    let r = check_json_depth(&nested_arrays(MAX_JSON_DEPTH + 1)).unwrap_err();
    assert_eq!(r.as_str(), "too_deep");
    assert_eq!(r.stage(), LoadStage::Depth);
}
