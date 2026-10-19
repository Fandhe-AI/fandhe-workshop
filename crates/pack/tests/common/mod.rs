//! 結合試験で共有する部品（一時ディレクトリ・試験データの場所・最小パック）。
//!
//! `tests/compile.rs`・`tests/compare.rs`・`tests/format_b.rs` が `mod common;` で取り込む。ファイルごとに使う部品が
//! 異なるため、未使用の警告だけをここで抑える。

#![allow(dead_code, reason = "試験ファイルごとに使う共通部品が異なるため")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_workshop_pack::Diagnostic;

pub const SECRET: &str = "SECRET-BODY-MARKER-4711";

pub fn testdata(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata")
        .join(rel)
}

pub fn ch1() -> PathBuf {
    testdata("input-field-ime/test-data/ch1-pack")
}

/// 一意な一時ディレクトリ。Drop で消す。
pub struct Tmp(pub PathBuf);

impl Tmp {
    pub fn new(tag: &str) -> Tmp {
        static N: AtomicUsize = AtomicUsize::new(0);
        let p = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "pack-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).expect("create tmp dir");
        Tmp(p)
    }

    pub fn put(&self, rel: &str, content: &[u8]) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::write(p, content).expect("write");
    }

    /// 論理サイズだけを持つ疎ファイルを置く。巨大な上限超過を、ディスクと時間を使わずに作るための部品。
    pub fn put_sparse(&self, rel: &str, len: u64) {
        let p = self.0.join(rel);
        fs::create_dir_all(p.parent().expect("parent")).expect("mkdir");
        fs::File::create(p)
            .expect("create")
            .set_len(len)
            .expect("set_len");
    }
}

/// 通常ファイルの論理サイズの合計（リンクは辿らない）。
pub fn dir_total_bytes(dir: &Path) -> u64 {
    let mut total = 0;
    for e in fs::read_dir(dir).expect("read_dir").flatten() {
        let md = fs::symlink_metadata(e.path()).expect("metadata");
        if md.is_dir() {
            total += dir_total_bytes(&e.path());
        } else if md.is_file() {
            total += md.len();
        }
    }
    total
}

impl Drop for Tmp {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("mkdir");
    for e in fs::read_dir(from).expect("read_dir").flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            fs::copy(&src, &dst).expect("copy");
        }
    }
}

pub const WORKSHOP: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";
pub const CHAPTER: &str = concat!(
    "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
    "  - {id: a, title: A, body: a.md, next: [{to: b}]}\n",
    "  - {id: b, title: B, final: true}\n",
);

/// 正しい最小パック（形式 B）。
pub fn minimal() -> Tmp {
    let t = Tmp::new("min");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    t
}

pub fn assert_no_secret(d: &[Diagnostic]) {
    for x in d {
        let s = x.to_string();
        assert!(!s.contains(SECRET), "diagnostic leaks body: {s}");
    }
}
