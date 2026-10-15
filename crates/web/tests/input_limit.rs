//! 入力欄の上限（SAFE-21。TASK-158）の native 試験。境界値は contract の `BOUNDARY_SAMPLES` から取る。

use fandhe_workshop_contract::limits::{
    BoundarySample, LimitId, BOUNDARY_SAMPLES, INPUT_TOO_LONG_MESSAGE, MAX_INPUT_CODE_POINTS,
};
use fandhe_workshop_web::input_limit::{clamp_draft, notice_for};

fn cp(s: &str) -> usize {
    s.chars().count()
}

fn sample() -> &'static BoundarySample {
    BOUNDARY_SAMPLES
        .iter()
        .find(|s| s.limit == LimitId::InputCodePoints)
        .expect("InputCodePoints の標本が無い")
}

#[test]
fn 境界値の標本はコードポイントの上限と一致する() {
    assert_eq!(sample().exact, MAX_INPUT_CODE_POINTS as u64);
}

#[test]
fn 上限ちょうどは通り_超過の印は立たない() {
    let v = "あ".repeat(sample().exact as usize);
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, v);
    assert!(!over);
}

#[test]
fn 一文字超過は先頭の上限までになり_超過の印が立つ() {
    let v = "あ".repeat(sample().over_by_one as usize);
    let (out, over) = clamp_draft(&v);
    assert_eq!(cp(out), MAX_INPUT_CODE_POINTS);
    assert!(over);
}

#[test]
fn 十倍の貼り付けも上限になる() {
    let v = "あ".repeat(sample().tenfold as usize);
    let (out, over) = clamp_draft(&v);
    assert_eq!(cp(out), MAX_INPUT_CODE_POINTS);
    assert!(over);
}

#[test]
fn サロゲートペアの文字は一文字と数える() {
    let v = "😀".repeat(MAX_INPUT_CODE_POINTS);
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, v);
    assert!(!over);
    let v = format!("{}😀", "あ".repeat(MAX_INPUT_CODE_POINTS - 1));
    assert_eq!(v.encode_utf16().count(), MAX_INPUT_CODE_POINTS + 1);
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, v);
    assert!(!over);
}

#[test]
fn サロゲートペアの超過は文字の境界で切る() {
    let v = "😀".repeat(MAX_INPUT_CODE_POINTS + 1);
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, "😀".repeat(MAX_INPUT_CODE_POINTS));
    assert!(over);
}

#[test]
fn 空_ascii_結合文字の扱い() {
    assert_eq!(clamp_draft(""), ("", false));
    assert_eq!(clamp_draft("abc"), ("abc", false));
    // 結合文字は 2 コードポイントと数える。
    let v = "e\u{301}".repeat(MAX_INPUT_CODE_POINTS / 2 + 1);
    let (out, over) = clamp_draft(&v);
    assert_eq!(cp(out), MAX_INPUT_CODE_POINTS);
    assert!(over);
}

#[test]
fn 案内文は固定の定数と一致し_超過でなければ空() {
    assert_eq!(notice_for(true), INPUT_TOO_LONG_MESSAGE);
    assert_eq!(notice_for(false), "");
}
