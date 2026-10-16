//! 履歴の読み込みの段 1〜3（大きさ・入れ子の深さ・JSON の段。`truncated` の判定を含む）の試験（TASK-331 1/3、SAVE-19・SAVE-20）。
//!
//! 順序の正は contract の `HISTORY_LOAD_ORDER`。2 つの段に同時に違反する入力で、前の段の理由が返ることを固定する。
//! 100MB の実バッファは作らず、長さだけを見る判定は `history_size` の試験に任せる（本試験は 1 バイト超過と 2 倍で確かめる）。
//! 画面の状態（DOM・`localStorage`）は core では観測できない。ブラウザでの確認は後続で扱う（未作成）。

mod common;

use std::time::{Duration, Instant};

use common::history_sample::read_bytes;
use common::json_input::{nested, pad_to};
use common::mem_store::MemStore;
use fandhe_workshop_contract::limits::{LoadStage, MAX_HISTORY_BYTES, MAX_JSON_DEPTH};
use fandhe_workshop_contract::load_reason::{HistoryLoadStage, LoadReason, HISTORY_LOAD_ORDER};
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::history_load::parse_history_json;

const LIMIT: usize = MAX_HISTORY_BYTES as usize;

fn reason(bytes: &[u8]) -> LoadReason {
    parse_history_json(bytes).unwrap_err()
}

fn order_pos(r: LoadReason) -> usize {
    HISTORY_LOAD_ORDER
        .iter()
        .position(|x| *x == r)
        .expect("reason must be in the order")
}

/// `{"a":"あ` を「あ」（3 バイト）の `n` バイト目で切る。
fn cut_multibyte(n: usize) -> Vec<u8> {
    let full = "{\"a\":\"あ\"}".as_bytes().to_vec();
    full[..6 + n].to_vec()
}

#[test]
fn exact_limit_valid_history_passes_size() {
    let body = read_bytes("two-rounds");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let padded = pad_to(&body, LIMIT);
    assert_eq!(padded.len(), LIMIT);
    assert_eq!(parse_history_json(&padded), Ok(v));
}

#[test]
fn over_by_one_is_too_large() {
    let padded = pad_to(&read_bytes("two-rounds"), LIMIT + 1);
    assert_eq!(reason(&padded), LoadReason::TooLarge);
}

#[test]
fn double_is_too_large() {
    let padded = vec![b' '; LIMIT * 2];
    assert_eq!(reason(&padded), LoadReason::TooLarge);
}

#[test]
fn depth_exact_passes_depth() {
    assert!(parse_history_json(&nested(MAX_JSON_DEPTH, true)).is_ok());
}

#[test]
fn depth_over_is_too_deep() {
    assert_eq!(
        reason(&nested(MAX_JSON_DEPTH + 1, true)),
        LoadReason::TooDeep
    );
}

#[test]
fn ten_thousand_levels_is_too_deep_within_a_second() {
    let b = nested(10_000, true);
    let t = Instant::now();
    assert_eq!(reason(&b), LoadReason::TooDeep);
    assert!(t.elapsed() < Duration::from_secs(1));
}

#[test]
fn cut_at_char_boundary_is_truncated() {
    let b = read_bytes("two-rounds");
    let cut = &b[..b.len() * 55 / 100];
    assert_eq!(reason(cut), LoadReason::Truncated);
}

#[test]
fn cut_inside_multibyte_char_is_truncated() {
    assert_eq!(reason(&cut_multibyte(1)), LoadReason::Truncated);
    assert_eq!(reason(&cut_multibyte(2)), LoadReason::Truncated);
}

#[test]
fn invalid_byte_in_middle_is_not_json() {
    assert_eq!(reason(b"{\"a\":\"\xff\"}"), LoadReason::NotJson);
}

#[test]
fn syntax_error_is_not_json() {
    assert_eq!(reason(b"{\"a\":1,}"), LoadReason::NotJson);
    assert_eq!(reason(b"{\"a\":1} x"), LoadReason::NotJson);
}

#[test]
fn empty_input_is_truncated() {
    assert_eq!(reason(b""), LoadReason::Truncated);
    assert_eq!(reason(b"  \n "), LoadReason::Truncated);
}

#[test]
fn valid_history_passes_three_stages() {
    for name in common::history_sample::SAMPLES {
        assert!(parse_history_json(&read_bytes(name)).is_ok(), "{name}");
    }
}

#[test]
fn size_precedes_depth() {
    let b = pad_to(&nested(MAX_JSON_DEPTH + 1, true), LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn size_precedes_utf8() {
    let b = pad_to(&cut_multibyte(1), LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn size_precedes_parse() {
    let b = pad_to(b"{not json", LIMIT + 1);
    assert_eq!(reason(&b), LoadReason::TooLarge);
}

#[test]
fn depth_precedes_utf8() {
    let mut b = nested(MAX_JSON_DEPTH + 1, true);
    b.push(0xff);
    assert_eq!(reason(&b), LoadReason::TooDeep);
}

#[test]
fn depth_precedes_eof() {
    assert_eq!(
        reason(&nested(MAX_JSON_DEPTH + 1, false)),
        LoadReason::TooDeep
    );
}

#[test]
fn utf8_precedes_parse() {
    let mut b = b"{not json \"".to_vec();
    b.extend_from_slice(&"あ".as_bytes()[..1]);
    assert_eq!(reason(&b), LoadReason::Truncated);
}

#[test]
fn reasons_match_contract_stages() {
    let cases: Vec<(Vec<u8>, HistoryLoadStage, LoadStage)> = vec![
        (
            pad_to(b"[]", LIMIT + 1),
            HistoryLoadStage::Size,
            LoadStage::Size,
        ),
        (
            nested(MAX_JSON_DEPTH + 1, true),
            HistoryLoadStage::Depth,
            LoadStage::Depth,
        ),
        (
            cut_multibyte(1),
            HistoryLoadStage::Json,
            LoadStage::JsonParse,
        ),
        (b"{,".to_vec(), HistoryLoadStage::Json, LoadStage::JsonParse),
    ];
    for (b, st, coarse) in cases {
        let r = reason(&b);
        assert_eq!(r.stage(), st, "{r}");
        assert_eq!(r.stage().coarse(), coarse, "{r}");
    }
}

#[test]
fn rejection_order_follows_history_load_order() {
    let mut deep_and_bad = nested(MAX_JSON_DEPTH + 1, true);
    deep_and_bad.push(0xff);
    // (入力, 返るべき理由, 同時に違反している後の段の理由)
    let cases: Vec<(Vec<u8>, LoadReason, LoadReason)> = vec![
        (
            pad_to(&nested(MAX_JSON_DEPTH + 1, true), LIMIT + 1),
            LoadReason::TooLarge,
            LoadReason::TooDeep,
        ),
        (deep_and_bad, LoadReason::TooDeep, LoadReason::NotJson),
        (
            pad_to(&cut_multibyte(1), LIMIT + 1),
            LoadReason::TooLarge,
            LoadReason::Truncated,
        ),
    ];
    for (b, got, later) in cases {
        assert_eq!(reason(&b), got);
        assert!(
            order_pos(got) < order_pos(later),
            "{got} must precede {later}"
        );
    }
}

#[test]
fn rejection_leaves_store_unchanged() {
    let store = MemStore::default();
    let k1 = HistoryKey::for_pack(DeliveryScope::root(), "pack-a", "start");
    let k2 = HistoryKey::for_pack(DeliveryScope::root(), "pack-b", "start");
    let valid = String::from_utf8(read_bytes("two-rounds")).unwrap();
    store.write(&k1, &valid).unwrap();
    store.write(&k2, &valid[..valid.len() * 55 / 100]).unwrap();
    store.reset_counts();
    let before = store.snapshot();
    let keys_before = store.keys().unwrap();

    // 保存先の経路（文字列）。
    let stored = store.read(&k2).unwrap().unwrap();
    assert_eq!(reason(stored.as_bytes()), LoadReason::Truncated);
    // ファイル由来の経路（バイト列）。
    let rejects: Vec<Vec<u8>> = vec![
        pad_to(b"[]", LIMIT + 1),
        nested(MAX_JSON_DEPTH + 1, true),
        cut_multibyte(2),
        b"{\"a\":1,}".to_vec(),
        Vec::new(),
    ];
    for b in &rejects {
        assert!(parse_history_json(b).is_err());
    }

    assert_eq!(store.snapshot(), before);
    assert_eq!(store.keys().unwrap(), keys_before);
    assert_eq!(store.writes(), 0);
    assert_eq!(store.removes(), 0);
}

#[test]
fn rejection_is_deterministic() {
    for b in [
        cut_multibyte(1),
        nested(MAX_JSON_DEPTH + 1, true),
        b"x".to_vec(),
    ] {
        assert_eq!(reason(&b), reason(&b));
    }
}
