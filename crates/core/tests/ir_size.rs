//! IR の大きさの上限（SAFE-17・SAFE-45・TASK-154）の境界と、解析の前に長さだけで断ることの試験。
//!
//! 上限の値は契約（`MAX_IR_BYTES`）から取り、ここで打ち直さない。ただし決定 D-44 の値
//! （20,971,520 バイト）は数字で 1 か所だけ固定し、契約の値が黙って変わらないようにする。

mod common;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::limits::{
    rejection_message, BoundarySample, LimitId, LimitReason, LoadEntry, LoadStage,
    BOUNDARY_SAMPLES, MAX_IR_BYTES,
};
use fandhe_workshop_core::ir_load::IrLoadError;
use fandhe_workshop_core::load_gate::check_size;
use fandhe_workshop_core::{check_ir_limits, load_ir};
use proptest::prelude::*;

fn sample() -> BoundarySample {
    BOUNDARY_SAMPLES
        .iter()
        .copied()
        .find(|s| s.limit == LimitId::IrBytes)
        .expect("ir bytes sample exists")
}

/// 有効な最小の教材を、末尾の空白で `len` バイトに埋める。
fn valid_ir_padded_to(len: u64) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut bytes = std::fs::read(path).expect("min-3step.json");
    let len = usize::try_from(len).expect("fits usize");
    assert!(bytes.len() < len, "base IR is smaller than the limit");
    bytes.resize(len, b' ');
    bytes
}

#[test]
fn exact_limit_matches_decision() {
    let s = sample();
    assert_eq!(s.exact, 20_971_520);
    assert_eq!(s.exact, MAX_IR_BYTES);
    assert_eq!(s.over_by_one, 20_971_521);
    assert_eq!(s.tenfold, 209_715_200);
}

#[test]
fn exact_limit_is_not_rejected_by_size() {
    assert_eq!(check_size(LoadEntry::Ir, sample().exact), Ok(()));
}

#[test]
fn over_by_one_is_rejected_as_too_large() {
    assert_eq!(
        check_size(LoadEntry::Ir, sample().over_by_one),
        Err(LimitReason::TooLarge)
    );
}

#[test]
fn tenfold_is_rejected_as_too_large() {
    assert_eq!(
        check_size(LoadEntry::Ir, sample().tenfold),
        Err(LimitReason::TooLarge)
    );
}

#[test]
fn valid_ir_padded_to_exact_limit_loads() {
    let bytes = valid_ir_padded_to(sample().exact);
    assert_eq!(bytes.len() as u64, MAX_IR_BYTES);
    assert_eq!(check_ir_limits(&bytes), Ok(()));
    assert!(load_ir(&bytes).is_ok());
}

#[test]
fn valid_ir_padded_over_by_one_is_too_large() {
    let bytes = valid_ir_padded_to(sample().over_by_one);
    assert_eq!(check_ir_limits(&bytes), Err(LimitReason::TooLarge));
    let err = load_ir(&bytes).unwrap_err();
    assert_eq!(err, IrLoadError::Limit(LimitReason::TooLarge));
    assert_eq!(err.reason_code(), "too_large");
}

#[test]
fn rejection_message_is_fixed_contract_text() {
    let message = rejection_message(LoadEntry::Ir, LimitReason::TooLarge)
        .expect("ir too_large has a message");
    assert!(
        message.contains("ファイルが上限の大きさを超えています"),
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
    let by_length = check_size(LoadEntry::Ir, sample().tenfold);
    let bytes = valid_ir_padded_to(sample().over_by_one);
    let by_load = load_ir(&bytes);
    let elapsed = start.elapsed();
    assert_eq!(by_length, Err(LimitReason::TooLarge));
    assert_eq!(by_load.unwrap_err().reason_code(), "too_large");
    assert!(elapsed < Duration::from_secs(1), "took {elapsed:?}");
}

#[test]
fn limit_neighborhood_is_judged_exactly() {
    for len in (MAX_IR_BYTES - 2)..=(MAX_IR_BYTES + 2) {
        let expected = if len <= MAX_IR_BYTES {
            Ok(())
        } else {
            Err(LimitReason::TooLarge)
        };
        assert_eq!(check_size(LoadEntry::Ir, len), expected, "len={len}");
    }
}

#[test]
fn size_is_the_first_stage() {
    assert_eq!(LimitReason::TooLarge.stage(), LoadStage::Size);
}

#[test]
fn size_rule_holds_for_any_length() {
    let lengths = prop_oneof![
        any::<u64>(),
        (MAX_IR_BYTES - 2)..=(MAX_IR_BYTES + 2),
        0..=(MAX_IR_BYTES * 10),
    ];
    check("ir-size-rule", DEFAULT_CASES, lengths, |len| {
        prop_assert_eq!(check_size(LoadEntry::Ir, len).is_ok(), len <= MAX_IR_BYTES);
        Ok(())
    });
}
