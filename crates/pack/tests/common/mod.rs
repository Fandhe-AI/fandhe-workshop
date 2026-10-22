//! 結合試験で共有する部品（一時ディレクトリ・試験データの場所・最小パック）。
//!
//! `tests/compile.rs`・`tests/compare.rs`・`tests/format_b.rs` が `mod common;` で取り込む。ファイルごとに使う部品が
//! 異なるため、未使用の警告だけをここで抑える。

#![allow(dead_code, reason = "試験ファイルごとに使う共通部品が異なるため")]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use fandhe_workshop_pack::validate::ANSWER_LABEL_REQUIRED;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

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

pub fn files_of(dir: &Path, base: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).expect("read_dir").flatten() {
        let p = e.path();
        if p.is_dir() {
            files_of(&p, base, out);
        } else {
            out.push(p.strip_prefix(base).expect("prefix").to_path_buf());
        }
    }
}

/// ファイルを辞書順の逆で作って写す（ディレクトリの列挙順を変える）。
pub fn reversed_copy(from: &Path, tag: &str) -> Tmp {
    let t = Tmp::new(tag);
    let mut files = Vec::new();
    files_of(from, from, &mut files);
    files.sort();
    files.reverse();
    for rel in files {
        let dst = t.0.join(&rel);
        fs::create_dir_all(dst.parent().expect("parent")).expect("mkdir");
        fs::copy(from.join(&rel), dst).expect("copy");
    }
    t
}

pub const WORKSHOP: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";
pub const CHAPTER: &str = concat!(
    "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n",
    "  - {id: a, title: A, body: a.md, next: [{to: b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n",
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

/// `validate` の診断から、確認 `answer` の `label` の欠け（`E-STRUCTURE`・[`ANSWER_LABEL_REQUIRED`]）だけを除く。
///
/// `label` の必須化（PACK-37・D-67）の前に作られた教材パック（spec の写し・IR の凍結物を持つ `fill-pack`）は
/// 編集できず、`label` の無い選択肢を持つ。それらを「`validate` が 0 件」「特定のコードだけ」と見る試験が、
/// この 1 種の診断だけを除いて残りを見るために使う。除いた件数も返し、除外が空振りしていないことを呼び出し側が確かめられる。
pub fn without_answer_label_required(diags: Vec<Diagnostic>) -> (Vec<Diagnostic>, usize) {
    let total = diags.len();
    let rest: Vec<Diagnostic> = diags
        .into_iter()
        .filter(|d| !(d.code() == DiagCode::Structure && d.message() == ANSWER_LABEL_REQUIRED))
        .collect();
    let removed = total - rest.len();
    (rest, removed)
}

/// `validate` の診断から、置換後の `do` の隠す語の漏れ（`E-GUIDANCE-LEAK`。PACK-52・TASK-71 3/3）だけを除く。
///
/// spec の写し（`module-reuse-rounds` など）は、引数を渡さない章が既定値の skill 名を `body_levels` を宣言した Step へ
/// 差し込む形のまま（R-5 が記録した真の漏れ）で、編集できない。「`validate` が 0 件」と見る試験が、この 1 種の診断だけを
/// 除いて残りを見るために使う。除いた件数も返し、除外が空振りしていないことを呼び出し側が確かめられる。
pub fn without_expanded_do_leak(diags: Vec<Diagnostic>) -> (Vec<Diagnostic>, usize) {
    let total = diags.len();
    let rest: Vec<Diagnostic> = diags
        .into_iter()
        .filter(|d| {
            !(d.code() == DiagCode::GuidanceLeak
                && d.message()
                    .ends_with("appears in do after argument substitution"))
        })
        .collect();
    let removed = total - rest.len();
    (rest, removed)
}
