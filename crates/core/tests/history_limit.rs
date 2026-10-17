//! 保存の前の上限の判定（`history_limit`、Issue #1414、SAVE-19・SAFE-41・SAVE-24）の試験。
//!
//! 境界（20,000 件と 20,001 件）、判定の順、読み込みの判定との一致、上限を超えても記録が止まらないことを確かめる。
//! 上限の値は契約（`MAX_HISTORY_OPS`・`MAX_HISTORY_BYTES`）から取り、ここで打ち直さない。
//! 履歴は受講者のデータのため、失敗の文には長さだけを出し、中身を出さない。規則は推論で、記録は `docs/testing.md` の 5.82。

mod common;

use common::history_sample::load;
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::limits::{LimitReason, MAX_HISTORY_BYTES, MAX_HISTORY_OPS};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_limit::{check_save, ops_within_limit};
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::ops_record::OpsRecorder;
use proptest::prelude::*;

fn bytes_limit() -> usize {
    usize::try_from(MAX_HISTORY_BYTES).expect("limit fits usize")
}

#[test]
fn ops_boundary_at_and_over_limit() {
    assert_eq!(MAX_HISTORY_OPS, 20_000);
    assert!(ops_within_limit(MAX_HISTORY_OPS));
    assert!(!ops_within_limit(MAX_HISTORY_OPS + 1));
    assert_eq!(check_save(0, MAX_HISTORY_OPS), Ok(()));
    assert_eq!(
        check_save(0, MAX_HISTORY_OPS + 1),
        Err(LimitReason::TooManyOps)
    );
}

#[test]
fn bytes_boundary_at_and_over_limit() {
    assert_eq!(check_save(bytes_limit(), 0), Ok(()));
    assert_eq!(check_save(bytes_limit() + 1, 0), Err(LimitReason::TooLarge));
}

#[test]
fn size_is_checked_before_ops() {
    assert_eq!(
        check_save(bytes_limit() + 1, MAX_HISTORY_OPS + 1),
        Err(LimitReason::TooLarge)
    );
    assert!(LimitReason::TooLarge.stage() < LimitReason::TooManyOps.stage());
}

#[test]
fn save_gate_agrees_with_load_gate() {
    let base = load("two-rounds");
    let first = base.ops.first().expect("sample has ops").clone();
    for (count, loadable) in [(MAX_HISTORY_OPS, true), (MAX_HISTORY_OPS + 1, false)] {
        let mut env = base.clone();
        env.ops = vec![first.clone(); count];
        let bytes = export_bytes(&env).expect("history must export");
        assert!(
            bytes.len() < bytes_limit(),
            "precondition: only the ops count is under test (bytes={})",
            bytes.len()
        );
        let value = parse_history_json(&bytes).expect("stages 1-3 must pass");
        let loaded = parse_history_envelope(value);
        let save = check_save(bytes.len(), env.ops.len());
        assert_eq!(loaded.is_ok(), loadable, "load gate at ops={count}");
        assert_eq!(save.is_ok(), loadable, "save gate at ops={count}");
        // 保存してよい包みは、必ず読み込める。
        assert_eq!(save.is_ok(), loaded.is_ok());
    }
}

#[test]
fn recorder_keeps_recording_past_limit() {
    let mut rec = OpsRecorder::new();
    for _ in 0..=MAX_HISTORY_OPS {
        rec.reload();
    }
    assert_eq!(rec.len(), MAX_HISTORY_OPS + 1);
    rec.draft("last".to_owned());
    assert_eq!(rec.len(), MAX_HISTORY_OPS + 2);
    assert!(
        matches!(
            rec.ops().last(),
            Some(fandhe_workshop_contract::history::HistoryOp::Draft(_))
        ),
        "the newest op is kept at the tail"
    );
}

#[test]
fn property_ops_boundary() {
    let lengths = prop_oneof![
        (MAX_HISTORY_OPS - 2)..=(MAX_HISTORY_OPS + 2),
        0..=(MAX_HISTORY_OPS * 10),
    ];
    check("history-limit-ops-rule", DEFAULT_CASES, lengths, |n| {
        prop_assert_eq!(check_save(0, n).is_ok(), n <= MAX_HISTORY_OPS);
        prop_assert_eq!(ops_within_limit(n), n <= MAX_HISTORY_OPS);
        Ok(())
    });
}
