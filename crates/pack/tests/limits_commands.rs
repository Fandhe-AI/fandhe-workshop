//! 教材パックの上限の検査が、3 つのコマンドの入口で読み込みの前に働くことの試験（AUTHOR-10・TASK-410）。
//!
//! 上限の値と理由コードの正は `contract::limits`、事前走査の実装は `source::check_limits`（TASK-159）。
//! `validate`・`test`・`compile` はどれも `source::load` を通るため、入口は次の 3 つで確かめる。
//!
//! - `source::load`（3 コマンド共通の読み込み。`test` の入口はこれを前提にする）
//! - `validate::validate_dir`
//! - `compile::compile_dir`
//!
//! 確かめた範囲の限界: `validate`・`test` の CLI 本体は未実装（TASK-403・TASK-404）で、CLI から起動したときの
//! 終了コードと診断の行は、それらの実装後に同じ教材パックで確かめる。CLI の層は `cli.rs` の `compile` だけを見る。
//!
//! - 「中身を読まない」ことは、`workshop.yaml` を構文として壊しても `E-PARSE` が出ないことと、
//!   `SECRET` が診断に現れないことで確かめる
//! - `.md` 以外の本文の上限超過は、事前走査ではなく本文の読み込みの時点で素の `E-LIMIT` になる。
//!   `.md` の判定は事前走査と同じく大文字小文字を区別する

mod common;

use std::time::{Duration, Instant};

use common::{assert_no_secret, dir_total_bytes, minimal, Tmp, CHAPTER, SECRET};

use fandhe_workshop_contract::limits::{MAX_MARKDOWN_BYTES, MAX_PACK_BYTES};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(|x| x.code()).collect()
}

fn break_workshop(t: &Tmp) {
    t.put("workshop.yaml", format!("pack: [{SECRET}\n").as_bytes());
}

fn secret_first(len: usize) -> Vec<u8> {
    let mut b = vec![b'a'; len];
    b[..SECRET.len()].copy_from_slice(SECRET.as_bytes());
    b
}

/// 3 つの入口すべてで、診断のコードの並びが `want` と一致し、`E-PARSE` もそれ以外も出ないことを確かめる。
fn assert_on_every_entry(t: &Tmp, want: &[DiagCode]) -> Vec<Diagnostic> {
    let a = load(&t.0).expect_err("load must reject");
    let b = validate_dir(&t.0);
    let c = compile_dir(&t.0).expect_err("compile must reject");
    for d in [&a, &b, &c] {
        assert_eq!(codes(d), want);
        assert_no_secret(d);
    }
    a
}

fn nested(n: usize) -> String {
    let mut p = String::from("deep");
    for _ in 1..n {
        p.push_str("/d");
    }
    p
}

#[test]
fn author10_pack_size_over_on_every_entry() {
    let t = minimal();
    break_workshop(&t);
    let used = dir_total_bytes(&t.0);
    t.put_sparse("pad.bin", MAX_PACK_BYTES + 1 - used);
    assert_on_every_entry(&t, &[DiagCode::LimitPackSize]);
}

#[test]
fn author10_md_size_over_on_every_entry() {
    let t = minimal();
    break_workshop(&t);
    t.put(
        "chapters/c1/unused.md",
        &secret_first(MAX_MARKDOWN_BYTES as usize + 1),
    );
    assert_on_every_entry(&t, &[DiagCode::LimitMdSize]);
}

#[test]
fn author10_depth_over_on_every_entry() {
    let t = minimal();
    break_workshop(&t);
    t.put(&format!("{}/x.txt", nested(33)), b"x");
    assert_on_every_entry(&t, &[DiagCode::LimitDepth]);
}

#[cfg(unix)]
#[test]
fn author10_symlink_on_every_entry() {
    let t = minimal();
    break_workshop(&t);
    symlink("/", t.0.join("link")).expect("symlink");
    assert_on_every_entry(&t, &[DiagCode::LimitSymlink]);
}

#[test]
fn author10_multiple_limits_listed_and_nothing_read() {
    let t = minimal();
    break_workshop(&t);
    t.put(
        "chapters/c1/unused.md",
        &secret_first(MAX_MARKDOWN_BYTES as usize + 1),
    );
    t.put(&format!("{}/x.txt", nested(33)), b"x");
    #[cfg(unix)]
    symlink("/", t.0.join("link")).expect("symlink");
    for d in [
        load(&t.0).expect_err("load"),
        validate_dir(&t.0),
        compile_dir(&t.0).expect_err("compile"),
    ] {
        assert!(!d.is_empty());
        for x in &d {
            assert!(
                matches!(
                    x.code(),
                    DiagCode::LimitMdSize | DiagCode::LimitDepth | DiagCode::LimitSymlink
                ),
                "unexpected code: {x}"
            );
        }
        let cs = codes(&d);
        assert!(cs.contains(&DiagCode::LimitMdSize));
        assert!(cs.contains(&DiagCode::LimitDepth));
        #[cfg(unix)]
        assert!(cs.contains(&DiagCode::LimitSymlink));
        assert_no_secret(&d);
    }
}

#[test]
fn author10_exactly_at_limits_loads_on_every_entry() {
    let t = minimal();
    t.put(
        "chapters/c1/unused.md",
        &vec![b'a'; MAX_MARKDOWN_BYTES as usize],
    );
    let used = dir_total_bytes(&t.0);
    t.put_sparse("pad.bin", MAX_PACK_BYTES - used);
    assert_eq!(dir_total_bytes(&t.0), MAX_PACK_BYTES);
    assert!(load(&t.0).is_ok());
    assert!(validate_dir(&t.0).is_empty());
    assert!(compile_dir(&t.0).is_ok());
}

/// 本文が `file` を指す最小パック。`levels` なら `body_levels` で指す。
fn pack_with_body(file: &str, levels: bool) -> Tmp {
    let t = minimal();
    let chapter = if levels {
        CHAPTER.replace(
            "body: a.md",
            &format!("body: a.md, body_levels: {{B: {file}}}"),
        )
    } else {
        CHAPTER.replace("body: a.md", &format!("body: {file}"))
    };
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t
}

#[test]
fn author10_non_md_body_over_is_e_limit() {
    for levels in [false, true] {
        let t = pack_with_body("b.txt", levels);
        t.put(
            "chapters/c1/b.txt",
            &secret_first(MAX_MARKDOWN_BYTES as usize + 1),
        );
        let d = assert_on_every_entry(&t, &[DiagCode::Limit]);
        assert_eq!(d[0].file(), "chapters/c1/b.txt");
    }
}

#[test]
fn author10_non_md_body_exactly_at_limit_loads() {
    let t = pack_with_body("b.txt", false);
    t.put(
        "chapters/c1/b.txt",
        &vec![b'a'; MAX_MARKDOWN_BYTES as usize],
    );
    assert!(validate_dir(&t.0).is_empty());
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn author10_non_md_body_huge_is_rejected_quickly() {
    let t = pack_with_body("b.txt", false);
    t.put_sparse("chapters/c1/b.txt", 5_242_880);
    let start = Instant::now();
    let d = load(&t.0).expect_err("must be rejected");
    let elapsed = start.elapsed();
    assert_eq!(codes(&d), [DiagCode::Limit]);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

#[test]
fn author10_md_body_over_stays_md_size() {
    let t = minimal();
    t.put(
        "chapters/c1/a.md",
        &secret_first(MAX_MARKDOWN_BYTES as usize + 1),
    );
    assert_on_every_entry(&t, &[DiagCode::LimitMdSize]);
}
