//! 入力欄の上限（SAFE-21。TASK-158）の native 試験。境界値は contract の `BOUNDARY_SAMPLES` から取る。
//!
//! `judge23_` で始まる試験は、TASK-280（JUDGE-23）のキーと貼り付けの側の 5 つの操作を、ブラウザが
//! `SetDraft` に渡す値の列として流す手順の試験である。変異 `JUDGE23-1`・`JUDGE23-2` の拠り所になる。

use fandhe_workshop_contract::limits::{
    BoundarySample, LimitId, BOUNDARY_SAMPLES, INPUT_TOO_LONG_MESSAGE, MAX_INPUT_CODE_POINTS,
};
use fandhe_workshop_web::input_limit::{clamp_draft, clip_paste, notice_for};

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

// ---- JUDGE-24（TASK-283）: 上限を超える貼り付けは貼った分だけを切る（D-60） ----

fn doc(before: usize, after: usize) -> String {
    format!("{}{}", "a".repeat(before), "b".repeat(after))
}

#[test]
fn judge24_収まる貼り付けは介入しない() {
    let d = doc(10, 10);
    assert_eq!(clip_paste(&d, 10, 10, "xxxxx"), None);
}

#[test]
fn judge24_途中への貼り付けは貼った分だけを切り_後ろの既存の文字を残す() {
    let d = doc(10_000, 9_990);
    let plan = clip_paste(&d, 10_000, 10_000, &"x".repeat(20)).expect("介入するはず");
    let (v, over) = clamp_draft(&plan.value);
    assert!(over);
    assert_eq!(
        v,
        format!(
            "{}{}{}",
            "a".repeat(10_000),
            "x".repeat(10),
            "b".repeat(9_990)
        )
    );
    assert_eq!(plan.caret_utf16, 10_010);
}

#[test]
fn judge24_上限ちょうどで途中に貼ると値は変わらず_超過の印が立ち_キャレットは残る() {
    let d = doc(10_000, 10_000);
    let plan = clip_paste(&d, 10_000, 10_000, "xyz").expect("介入するはず");
    let (v, over) = clamp_draft(&plan.value);
    assert_eq!(v, d);
    assert!(over);
    assert_eq!(plan.caret_utf16, 10_000);
}

#[test]
fn judge24_選択範囲の置き換えは選択した分だけ入る量が増える() {
    let d = doc(10_000, 9_995);
    // 5 文字選ぶと room は 5 + 5 = 10。
    let plan = clip_paste(&d, 9_998, 10_003, &"x".repeat(30)).expect("介入するはず");
    let (v, over) = clamp_draft(&plan.value);
    assert!(over);
    assert_eq!(cp(v), MAX_INPUT_CODE_POINTS);
    assert_eq!(v.matches('x').count(), 10);
    assert_eq!(plan.caret_utf16, 9_998 + 10);
}

#[test]
fn judge24_後ろが空なら介入せず_末尾で切る安全網に任せる() {
    let d = doc(10_000, 0);
    assert_eq!(clip_paste(&d, 10_000, 10_000, &"x".repeat(20_000)), None);
}

#[test]
fn judge24_サロゲートペアは貼った側を割らない() {
    let d = doc(10, 19_989);
    // room = 1。先頭の 1 文字（ペア）だけ残る。
    let plan = clip_paste(&d, 10, 10, "😀😀😀").expect("介入するはず");
    let (v, over) = clamp_draft(&plan.value);
    assert!(over);
    assert_eq!(cp(v), MAX_INPUT_CODE_POINTS);
    assert_eq!(v.matches('😀').count(), 1);
    assert_eq!(plan.caret_utf16, 12);
}

#[test]
fn judge24_既存の側のペアの途中を指す位置は介入しない() {
    let d = format!("a😀{}", "b".repeat(19_990));
    assert_eq!(clip_paste(&d, 2, 2, &"x".repeat(100)), None);
}

#[test]
fn judge24_改行は_lf_として数える() {
    let d = doc(10, 19_985);
    // room = 5。CRLF は 1 文字、CR も 1 文字。
    let plan = clip_paste(&d, 10, 10, "1\r\n2\r3\n4xyz").expect("介入するはず");
    let (v, _) = clamp_draft(&plan.value);
    assert_eq!(&v[10..15], "1\n2\n3");
    assert_eq!(plan.caret_utf16, 15);
}

#[test]
fn judge24_不正な位置は介入しない() {
    let d = doc(5, 19_990);
    let big = "x".repeat(100);
    assert_eq!(clip_paste(&d, 6, 5, &big), None);
    assert_eq!(clip_paste(&d, 5, 20_000, &big), None);
    assert_eq!(clip_paste(&d, 20_000, 20_000, &big), None);
}

#[test]
fn judge24_選択の外だけで上限を超えるなら介入しない() {
    let d = doc(10_000, 10_001);
    assert_eq!(clip_paste(&d, 10_000, 10_000, "x"), None);
}

#[test]
fn judge24_巨大な貼り付けでも結果は正しい() {
    let d = doc(100, 19_000);
    let plan = clip_paste(&d, 100, 100, &"あ".repeat(200_000)).expect("介入するはず");
    let (v, over) = clamp_draft(&plan.value);
    assert!(over);
    assert_eq!(cp(v), MAX_INPUT_CODE_POINTS);
    assert_eq!(v.matches('あ').count(), 900);
    assert!(v.ends_with(&"b".repeat(19_000)));
}
