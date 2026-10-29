//! `ops` の件数の上限（20,000 件。D-44・TASK-156。SAVE-19・SAVE-41）の境界・桁違い・所要時間の試験。
//!
//! 判定の実体は `history_load` の段 5（件数）で、型の変換の後・段 7 以降の前に置かれる。本試験は、
//! 上限のすぐ上と 10 倍が速く `too_many_ops` で断られること、上限ちょうどが件数では断られず
//! 段 7・8 の検査へ進むことを固定する。1 秒の表明は計時が機械に依存するため native では 10 倍の拒否だけが持ち、
//! 実測を `eprintln!` で残す（ブラウザの試験 `ops-limit` が全体を持つ）。
//! 試料は最小の形の op で作る。10 倍でも段 1 の大きさ（10MB）に掛からず、件数の判定に届くようにするためである。

mod common;

use std::time::{Duration, Instant};

use common::contract_sample::{load as load_sample, testdata_dir};
use common::history_sample::read_bytes;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::limits::{MAX_HISTORY_BYTES, MAX_HISTORY_OPS};
use fandhe_workshop_contract::load_reason::{LoadReason, HISTORY_LOAD_ORDER};
use fandhe_workshop_core::history_load::{load_history, parse_history_envelope};
use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::version_record::record_versions;
use serde_json::{json, Value};

fn ops_json(n: usize) -> Value {
    Value::Array((0..n).map(|_| json!({"draft": "x"})).collect())
}

fn value_with_ops(n: usize) -> Value {
    let mut v: Value = serde_json::from_slice(&read_bytes("two-rounds")).expect("parse");
    v["ops"] = ops_json(n);
    v
}

fn bytes_with_ops(n: usize) -> Vec<u8> {
    serde_json::to_vec(&value_with_ops(n)).expect("to_vec")
}

fn open(bytes: &[u8]) -> Opened {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let ir = load_ir(&std::fs::read(path).expect("read min-3step")).expect("load_ir");
    let versions = record_versions(&ir, &content_hash(&ir).expect("hash")).expect("versions");
    load_history(
        bytes,
        &ir,
        &versions,
        &OpeningCaps::current(),
        load_sample().initial,
    )
}

fn reason(o: &Opened) -> Option<LoadReason> {
    match o {
        Opened::ViewOnly { code } | Opened::Refused { code } => Some(*code),
        _ => None,
    }
}

fn order_pos(r: LoadReason) -> usize {
    HISTORY_LOAD_ORDER
        .iter()
        .position(|x| *x == r)
        .expect("reason must be in the order")
}

#[test]
fn limit_value_matches_decision() {
    // D-44（TASK-10 の決定）。値を変えるときは決定の記録と同じ変更で更新する。
    assert_eq!(MAX_HISTORY_OPS, 20_000);
}

#[test]
fn over_by_one_is_too_many_ops() {
    let b = bytes_with_ops(MAX_HISTORY_OPS + 1);
    assert_eq!(reason(&open(&b)), Some(LoadReason::TooManyOps));
}

#[test]
fn tenfold_is_too_many_ops_within_a_second() {
    let b = bytes_with_ops(MAX_HISTORY_OPS * 10);
    // 段 1 の大きさに先に掛からないこと（件数の判定に届く試料であること）。
    assert!(
        (b.len() as u64) < MAX_HISTORY_BYTES,
        "sample too large: {}",
        b.len()
    );
    let t = Instant::now();
    let o = open(&b);
    let d = t.elapsed();
    eprintln!("ops-limit: tenfold bytes={} elapsed={d:?}", b.len());
    assert_eq!(reason(&o), Some(LoadReason::TooManyOps));
    assert!(d < Duration::from_secs(1), "{d:?}");
}

/// 上限ちょうどは件数では断られず、段 7・8 の検査まで進む。合成の 20,000 件は写しと合わないため、
/// 開くのではなく「段 5 を越えた理由で断られる」ことを観察する。
#[test]
fn at_limit_reaches_replay_stage() {
    let o = open(&bytes_with_ops(MAX_HISTORY_OPS));
    let r = reason(&o).expect("a synthetic history does not open");
    eprintln!("ops-limit: at-limit reason={r:?}");
    assert!(
        order_pos(r) > order_pos(LoadReason::TooManyOps),
        "{r:?} must come after too_many_ops"
    );
}

#[test]
fn count_rule_holds_near_limit() {
    for n in (MAX_HISTORY_OPS - 2)..=(MAX_HISTORY_OPS + 2) {
        let r = parse_history_envelope(value_with_ops(n));
        assert_eq!(
            r.err() == Some(LoadReason::TooManyOps),
            n > MAX_HISTORY_OPS,
            "n={n}"
        );
    }
}
