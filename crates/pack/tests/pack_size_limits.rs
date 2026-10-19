//! 教材パック全体と Markdown 1 ファイルの大きさの上限の試験（SAFE-22・D-25、TASK-159 1/2）。
//!
//! 上限の値と理由コードの正は `contract::limits`。`source::load` の事前走査は、ファイルの大きさ（metadata）
//! だけを数えて中身を読まずに拒否する。本ファイルはその境界（ちょうど・+1・桁違いの超過）を固定する。
//! 深さ・シンボリックリンクは #442（TASK-159 2/2）、3 コマンドへの適用は TASK-410 の範囲で、ここでは扱わない。
//!
//! - 巨大なファイルは疎ファイルで作る（論理サイズだけを持ち、ディスクと時間を使わない）
//! - 「中身を読まない」ことは、`workshop.yaml` を構文として壊しても `E-PARSE` が出ないことで確かめる
//! - 参照されない `.md` を使い、本文の読み込み時の判定（`load_body`）ではなく事前走査だけが捉える形にする

mod common;

use std::time::{Duration, Instant};

use common::{assert_no_secret, dir_total_bytes, minimal, Tmp, SECRET};

use fandhe_workshop_contract::limits::{MAX_MARKDOWN_BYTES, MAX_PACK_BYTES};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(|x| x.code()).collect()
}

/// 合計がちょうど `total` になるよう、参照されない疎ファイルを足した最小パック。
/// `broken` なら `workshop.yaml` を構文として壊す（中身を読めば `E-PARSE` になる）。
fn pack_with_total(total: u64, broken: bool) -> Tmp {
    let t = minimal();
    if broken {
        // 壊す変更で大きさが変わるため、パディングの計算より前に行う。
        break_workshop(&t);
    }
    let used = dir_total_bytes(&t.0);
    t.put_sparse("pad.bin", total - used);
    assert_eq!(dir_total_bytes(&t.0), total);
    t
}

fn break_workshop(t: &Tmp) {
    t.put("workshop.yaml", format!("pack: [{SECRET}\n").as_bytes());
}

#[test]
fn limits_match_spec_values() {
    assert_eq!(MAX_PACK_BYTES, 20_971_520);
    assert_eq!(MAX_MARKDOWN_BYTES, 524_288);
}

#[test]
fn pack_total_exactly_at_limit_loads() {
    let t = pack_with_total(MAX_PACK_BYTES, false);
    assert!(load(&t.0).is_ok());
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn pack_total_one_byte_over_is_rejected_without_reading() {
    let t = pack_with_total(MAX_PACK_BYTES + 1, true);
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitPackSize]);
    assert_no_secret(&d);
}

#[test]
fn pack_total_200mb_is_rejected_quickly() {
    let t = minimal();
    t.put_sparse("pad.bin", 209_715_200);
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitPackSize]);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

#[test]
fn markdown_exactly_at_limit_loads_even_when_unreferenced() {
    let t = minimal();
    t.put(
        "chapters/c1/unused.md",
        &vec![b'a'; MAX_MARKDOWN_BYTES as usize],
    );
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn unreferenced_markdown_one_byte_over_is_rejected_by_prescan() {
    let t = minimal();
    let mut body = vec![b'a'; MAX_MARKDOWN_BYTES as usize + 1];
    body[..SECRET.len()].copy_from_slice(SECRET.as_bytes());
    t.put("chapters/c1/unused.md", &body);
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitMdSize]);
    assert_eq!(d[0].file(), "chapters/c1/unused.md");
    assert_no_secret(&d);
}

#[test]
fn markdown_5mb_is_rejected_quickly_without_reading() {
    let t = minimal();
    t.put_sparse("chapters/c1/unused.md", 5_242_880);
    break_workshop(&t);
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitMdSize]);
    assert_no_secret(&d);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}
