//! ディレクトリの深さとシンボリックリンクの上限の試験（SAFE-43・D-44、TASK-159 2/2）。
//!
//! 上限の値と理由コードの正は `contract::limits`。`source::load` の事前走査は、メタデータだけを見て
//! リンクを辿らず、中身を読まずに拒否する。本ファイルはその境界を固定する。
//!
//! - 段数は根を 0 段として数える。32 段は通し、33 段は `E-LIMIT-DEPTH` で拒否する
//! - シンボリックリンクは種類（ファイル・ディレクトリ・宛先なし・循環）を問わず `E-LIMIT-SYMLINK` で拒否する
//! - 「中身を読まない」ことは、`workshop.yaml` を構文として壊しても `E-PARSE` が出ないことと、
//!   走査されない側・リンクの先にある `SECRET` や上限超過が診断に現れないことで確かめる
//! - ファイルへのリンクを本文として参照する場合は `compile.rs` の `neg_symlink_is_rejected_without_following` が扱う
//! - 3 コマンドの入口への適用は `limits_commands.rs`（TASK-410）で扱う

mod common;

use std::time::{Duration, Instant};

use common::{assert_no_secret, minimal, Tmp, SECRET};

use fandhe_workshop_contract::limits::{PackLimitCode, MAX_PACK_DIR_DEPTH};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::source::load;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(|x| x.code()).collect()
}

/// `n` 段の入れ子の相対パス。`deep` が 1 段目になる。
fn nested(n: usize) -> String {
    let mut p = String::from("deep");
    for _ in 1..n {
        p.push_str("/d");
    }
    p
}

fn break_workshop(t: &Tmp) {
    t.put("workshop.yaml", format!("pack: [{SECRET}\n").as_bytes());
}

#[test]
fn limits_match_decision_d44() {
    assert_eq!(MAX_PACK_DIR_DEPTH, 32);
    assert_eq!(PackLimitCode::DirDepth.as_str(), "E-LIMIT-DEPTH");
    assert_eq!(PackLimitCode::Symlink.as_str(), "E-LIMIT-SYMLINK");
    assert!(PackLimitCode::DirDepth.message().contains("32"));
    assert!(PackLimitCode::Symlink.limit().is_none());
}

#[test]
fn dir_exactly_32_levels_loads() {
    let t = minimal();
    t.put(&format!("{}/leaf.txt", nested(32)), b"x");
    assert!(load(&t.0).is_ok());
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn dir_33_levels_is_rejected_without_reading() {
    let t = minimal();
    break_workshop(&t);
    let deep = nested(33);
    t.put_sparse(&format!("{deep}/big.md"), 5_242_880);
    #[cfg(unix)]
    symlink("nowhere", t.0.join(&deep).join("link")).expect("symlink");
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitDepth]);
    assert_eq!(d[0].file(), deep);
    assert_no_secret(&d);
}

#[test]
fn far_deeper_dirs_are_rejected_quickly() {
    let t = minimal();
    t.put(&format!("{}/x.txt", nested(200)), b"x");
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitDepth]);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

/// パックの外に、`SECRET` 入りの `x.md` と巨大な疎ファイルを置いた一時ディレクトリ。
#[cfg(unix)]
fn outside() -> Tmp {
    let o = Tmp::new("outside");
    o.put("x.md", format!("# x\n{SECRET}\n").as_bytes());
    o.put_sparse("huge.bin", 209_715_200);
    o
}

#[cfg(unix)]
#[test]
fn dir_symlink_to_outside_is_rejected_without_following() {
    let o = outside();
    let t = minimal();
    symlink(&o.0, t.0.join("chapters/c1/ext")).expect("symlink");
    break_workshop(&t);
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_eq!(d[0].file(), "chapters/c1/ext");
    assert_no_secret(&d);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

#[cfg(unix)]
#[test]
fn dangling_symlink_is_rejected_as_symlink() {
    let t = minimal();
    symlink("nowhere", t.0.join("chapters/c1/dangling")).expect("symlink");
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_eq!(d[0].file(), "chapters/c1/dangling");
}

#[cfg(unix)]
#[test]
fn symlink_cycle_to_pack_root_is_rejected_quickly() {
    let t = minimal();
    break_workshop(&t);
    symlink(&t.0, t.0.join("loop")).expect("symlink");
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}

#[cfg(unix)]
#[test]
fn chapters_dir_itself_as_symlink_is_rejected() {
    let o = Tmp::new("chapters-copy");
    o.put("c1/chapter.yaml", common::CHAPTER.as_bytes());
    o.put("c1/a.md", b"# a\n");
    let t = Tmp::new("chapters-link");
    t.put("workshop.yaml", common::WORKSHOP.as_bytes());
    symlink(&o.0, t.0.join("chapters")).expect("symlink");
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_eq!(d[0].file(), "chapters");
}

#[cfg(unix)]
#[test]
fn symlink_named_workshop_yaml_is_rejected_before_parse() {
    let o = Tmp::new("ws-outside");
    o.put("workshop.yaml", common::WORKSHOP.as_bytes());
    let t = Tmp::new("ws-link");
    t.put("chapters/c1/chapter.yaml", common::CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    symlink(o.0.join("workshop.yaml"), t.0.join("workshop.yaml")).expect("symlink");
    let d = load(&t.0).expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_eq!(d[0].file(), "workshop.yaml");
}

#[cfg(unix)]
#[test]
fn symlink_to_huge_file_is_rejected_quickly_without_reading() {
    let o = outside();
    let t = minimal();
    std::fs::remove_file(t.0.join("chapters/c1/a.md")).expect("rm");
    symlink(o.0.join("huge.bin"), t.0.join("chapters/c1/a.md")).expect("symlink");
    break_workshop(&t);
    let start = Instant::now();
    let r = load(&t.0);
    let elapsed = start.elapsed();
    let d = r.expect_err("must be rejected");
    assert_eq!(codes(&d), [DiagCode::LimitSymlink]);
    assert_no_secret(&d);
    assert!(elapsed < Duration::from_secs(1), "{elapsed:?}");
}
