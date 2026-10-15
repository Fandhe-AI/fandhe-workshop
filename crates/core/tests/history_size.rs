//! 履歴ファイルの大きさの上限（SAFE-18・TASK-155）の境界と、中身を読まないことの試験。
//!
//! 上限の値は契約（`MAX_HISTORY_BYTES`）から取り、ここで打ち直さない。

mod common;

use std::time::{Duration, Instant};

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::limits::{
    rejection_message, BoundarySample, LimitId, LimitReason, LoadEntry, LoadStage,
    BOUNDARY_SAMPLES, LOAD_STAGE_ORDER, MAX_HISTORY_BYTES,
};
use fandhe_workshop_core::history_size::{check_history_bytes, check_history_size};
use proptest::prelude::*;

fn sample() -> BoundarySample {
    BOUNDARY_SAMPLES
        .iter()
        .copied()
        .find(|s| s.limit == LimitId::HistoryBytes)
        .expect("history bytes sample exists")
}

#[test]
fn exact_limit_is_not_rejected_by_size() {
    let s = sample();
    assert_eq!(s.exact, 10_485_760);
    assert_eq!(s.exact, MAX_HISTORY_BYTES);
    assert_eq!(check_history_size(s.exact), Ok(()));
}

#[test]
fn over_by_one_is_rejected_as_too_large() {
    let s = sample();
    assert_eq!(s.over_by_one, 10_485_761);
    assert_eq!(
        check_history_size(s.over_by_one),
        Err(LimitReason::TooLarge)
    );
}

#[test]
fn tenfold_is_rejected_as_too_large() {
    let s = sample();
    assert_eq!(s.tenfold, 104_857_600);
    assert_eq!(check_history_size(s.tenfold), Err(LimitReason::TooLarge));
}

#[test]
fn empty_history_is_not_rejected_by_size() {
    assert_eq!(check_history_size(0), Ok(()));
    assert_eq!(check_history_bytes(&[]), Ok(()));
}

#[test]
fn bytes_are_judged_by_length_without_reading_content() {
    // JSON として壊れた中身でも、判定は長さだけで決まる（解釈の段の理由にならない）。
    let exact = vec![b'{'; usize::try_from(MAX_HISTORY_BYTES).expect("fits usize")];
    assert_eq!(check_history_bytes(&exact), Ok(()));
    assert_eq!(
        check_history_bytes(&exact),
        check_history_size(MAX_HISTORY_BYTES)
    );
    let mut over = exact;
    over.push(b'{');
    assert_eq!(check_history_bytes(&over), Err(LimitReason::TooLarge));
    assert_eq!(
        check_history_bytes(&over),
        check_history_size(MAX_HISTORY_BYTES + 1)
    );
}

#[test]
fn size_is_the_first_stage() {
    assert_eq!(LimitReason::TooLarge.stage(), LoadStage::Size);
    assert_eq!(LOAD_STAGE_ORDER[0], LoadStage::Size);
}

#[test]
fn rejection_message_is_fixed_contract_text() {
    let message = rejection_message(LoadEntry::History, LimitReason::TooLarge)
        .expect("history too_large has a message");
    assert!(
        message.contains("履歴ファイルが 10MB の上限を超えています"),
        "{message}"
    );
    let s = sample();
    for measured in [s.over_by_one, s.tenfold] {
        assert!(!message.contains(&measured.to_string()), "{message}");
    }
}

#[test]
fn rejection_is_fast() {
    let start = Instant::now();
    let result = check_history_size(sample().tenfold);
    let elapsed = start.elapsed();
    assert_eq!(result, Err(LimitReason::TooLarge));
    assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
}

#[test]
fn limit_neighborhood_is_judged_exactly() {
    for len in (MAX_HISTORY_BYTES - 2)..=(MAX_HISTORY_BYTES + 2) {
        let expected = if len <= MAX_HISTORY_BYTES {
            Ok(())
        } else {
            Err(LimitReason::TooLarge)
        };
        assert_eq!(check_history_size(len), expected, "len={len}");
    }
}

#[test]
fn size_rule_holds_for_any_length() {
    // 一様な u64 は境界に当たらないので、境界付近と上限の 10 倍付近を混ぜる。
    let lengths = prop_oneof![
        any::<u64>(),
        (MAX_HISTORY_BYTES - 2)..=(MAX_HISTORY_BYTES + 2),
        0..=(MAX_HISTORY_BYTES * 10),
    ];
    check("history-size-rule", DEFAULT_CASES, lengths, |len| {
        prop_assert_eq!(check_history_size(len).is_ok(), len <= MAX_HISTORY_BYTES);
        Ok(())
    });
}
