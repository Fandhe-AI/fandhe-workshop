//! 入力欄の上限（SAFE-21。TASK-158）の native 試験。境界値は contract の `BOUNDARY_SAMPLES` から取る。
//!
//! `judge23_` で始まる試験は、TASK-280（JUDGE-23）のキーと貼り付けの側の 5 つの操作を、ブラウザが
//! `SetDraft` に渡す値の列として流す手順の試験である。変異 `JUDGE23-1`・`JUDGE23-2` の拠り所になる。

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

// ---- TASK-280（JUDGE-23）: キーと貼り付けの側の 5 つの操作 ----

/// 欄の末尾へ 1 文字足した値。ブラウザで 1 文字を打ったときに `input` が運ぶ値に当たる。
fn type_char(current: &str, c: char) -> String {
    let mut v = current.to_string();
    v.push(c);
    v
}

/// 欄の末尾の 1 コードポイントを消した値。
fn delete_last(current: &str) -> String {
    let mut v = current.to_string();
    v.pop();
    v
}

fn full() -> String {
    "あ".repeat(MAX_INPUT_CODE_POINTS)
}

#[test]
fn judge23_20000文字の貼り付けは受け付け_案内は出ない() {
    let v = full();
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, v);
    assert!(!over);
    assert_eq!(notice_for(over), "");
}

#[test]
fn judge23_20001文字の貼り付けは受け付けず_理由を示す() {
    let v = "あ".repeat(MAX_INPUT_CODE_POINTS + 1);
    let (out, over) = clamp_draft(&v);
    assert_eq!(out, full());
    assert!(over);
    assert_eq!(notice_for(over), INPUT_TOO_LONG_MESSAGE);
}

#[test]
fn judge23_上限ちょうどで1文字入力しても受け付けず_理由を示す() {
    for c in ['x', '😀'] {
        let v = type_char(&full(), c);
        let (out, over) = clamp_draft(&v);
        assert_eq!(out, full(), "既存の文字が消えてはならない: {c}");
        assert!(over);
        assert_eq!(notice_for(over), INPUT_TOO_LONG_MESSAGE);
    }
}

#[test]
fn judge23_utf16で20001単位でも20000コードポイントは受け付ける() {
    let base = "あ".repeat(MAX_INPUT_CODE_POINTS - 1);
    let pasted = format!("{base}😀");
    assert_eq!(pasted.encode_utf16().count(), MAX_INPUT_CODE_POINTS + 1);
    let (out, over) = clamp_draft(&pasted);
    assert_eq!(out, pasted);
    assert!(!over);
    assert_eq!(notice_for(over), "");

    let typed = type_char(&base, '😀');
    let (out, over) = clamp_draft(&typed);
    assert_eq!(out, typed);
    assert!(!over);
    assert_eq!(notice_for(over), "");
}

#[test]
fn judge23_1文字消すと案内も消える() {
    let over_state = type_char(&full(), 'x');
    let (kept, over) = clamp_draft(&over_state);
    assert!(over, "前提: 超過の状態");
    let after = delete_last(kept);
    let (out, over) = clamp_draft(&after);
    assert_eq!(cp(out), MAX_INPUT_CODE_POINTS - 1);
    assert!(!over);
    assert_eq!(notice_for(over), "");

    // 絵文字で 20,000 コードポイントの状態から消すと、絵文字が丸ごと消える。
    let emoji = format!("{}😀", "あ".repeat(MAX_INPUT_CODE_POINTS - 1));
    let after = delete_last(&emoji);
    let (out, over) = clamp_draft(&after);
    assert_eq!(cp(out), MAX_INPUT_CODE_POINTS - 1);
    assert!(!over);
    assert_eq!(notice_for(over), "");
}

#[test]
fn judge23_5つの操作を順に流すと期待どおりに受け付け_拒否し_案内が消える() {
    // 状態は、直前の `clamp_draft` の出力を次の入力の元にする（glue が切った値を欄へ戻す流れ）。
    let f = full();
    // 1. 20,000 文字の貼り付け
    let (s, over) = clamp_draft(&f);
    assert_eq!((cp(s), over), (MAX_INPUT_CODE_POINTS, false));
    // 3. その状態で 1 文字入力
    let typed = type_char(s, 'x');
    let (s, over) = clamp_draft(&typed);
    assert_eq!((s, over), (f.as_str(), true));
    assert_eq!(notice_for(over), INPUT_TOO_LONG_MESSAGE);
    // 5. 1 文字消す
    let erased = delete_last(s);
    let (s, over) = clamp_draft(&erased);
    assert_eq!((cp(s), over), (MAX_INPUT_CODE_POINTS - 1, false));
    assert_eq!(notice_for(over), "");
    // 4. UTF-16 で 20,001 単位になる貼り付け
    let pasted = format!("{}😀", "あ".repeat(MAX_INPUT_CODE_POINTS - 1));
    let (s, over) = clamp_draft(&pasted);
    assert_eq!((s, over), (pasted.as_str(), false));
    // 2. 20,001 文字の貼り付け
    let big = "あ".repeat(MAX_INPUT_CODE_POINTS + 1);
    let (s, over) = clamp_draft(&big);
    assert_eq!((s, over), (f.as_str(), true));
    assert_eq!(notice_for(over), INPUT_TOO_LONG_MESSAGE);
}
