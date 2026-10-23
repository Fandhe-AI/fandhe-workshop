//! core・contract のソースに、時刻・乱数・大域状態・反復順が不定の集合の源が無いことの静的な検査（TASK-218 1/3。EVT-22・EVT-16）。
//!
//! 「同じ入力列から同じ結果」を `determinism.rs` の実行で確かめても、たまたま一致しただけの非決定性は見逃す。
//! 源になりうる語をソースから 0 件に固定し、例外は 1 件だけ（件数つき）にする。新しい源が入ると失敗する（fail-closed）。
//! 依存の推移的な閉包は `make check-deps` の領分で、ここでは `[dependencies]` の直接の依存だけを見る。
//! 照合は行コメントを除き、識別子の境界で行う。マクロ・文字列の中身は見分けない（限界は `docs/testing.md` 5.277）。

use std::path::{Path, PathBuf};

/// 禁止の語。
const FORBIDDEN: &[&str] = &[
    "std::time",
    "SystemTime",
    "Instant",
    "rand::",
    "getrandom",
    "thread_rng",
    "RandomState",
    "std::env",
    "thread_local",
    "static mut",
    "AtomicBool",
    "AtomicUsize",
    "AtomicU8",
    "AtomicU16",
    "AtomicU32",
    "AtomicU64",
    "AtomicIsize",
    "AtomicI8",
    "AtomicI16",
    "AtomicI32",
    "AtomicI64",
    "OnceLock",
    "LazyLock",
    "HashMap",
    "HashSet",
];

/// 例外（ファイルの末尾の相対パス, 語, 件数）。`revise_targets` の重複除去の所属判定だけで使い、出力順は `Vec` の順。1 行の型注釈と生成で 2 出現。
const EXCEPTIONS: &[(&str, &str, usize)] = &[("core/src/select_judge.rs", "HashSet", 2)];

/// 依存に置いてはならない時刻・乱数の代表的な crate。
const FORBIDDEN_CRATES: &[&str] = &[
    "rand",
    "getrandom",
    "fastrand",
    "chrono",
    "time",
    "uuid",
    "instant",
    "web-time",
    "js-sys",
];

fn is_id(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn strip_line_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 識別子の境界で `needle` の出現数を数える。
fn count_word(hay: &str, needle: &str) -> usize {
    let mut n = 0;
    let mut from = 0;
    while let Some(p) = hay[from..].find(needle) {
        let s = from + p;
        let e = s + needle.len();
        let before_ok =
            !needle.starts_with(is_id) || !hay[..s].chars().next_back().is_some_and(is_id);
        let after_ok = !needle.ends_with(is_id) || !hay[e..].chars().next().is_some_and(is_id);
        if before_ok && after_ok {
            n += 1;
        }
        from = e;
    }
    n
}

/// 1 ファイルの違反を返す。`name` は例外の照合に使うパスの末尾。
fn violations(name: &str, src: &str, exceptions: &[(&str, &str, usize)]) -> Vec<String> {
    let code = strip_line_comments(src);
    let mut out = Vec::new();
    for word in FORBIDDEN {
        let n = count_word(&code, word);
        let allowed = exceptions
            .iter()
            .find(|(f, w, _)| name.ends_with(f) && w == word)
            .map_or(0, |(_, _, c)| *c);
        if n > allowed {
            out.push(format!("{name}: `{word}` x{n} (allowed {allowed})"));
        }
    }
    out
}

fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("read src dir") {
        let path = entry.expect("dir entry").path();
        if path.is_dir() {
            rs_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn crates_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crates dir")
        .to_path_buf()
}

#[test]
fn core_and_contract_sources_have_no_nondeterminism_sources() {
    let mut files = Vec::new();
    rs_files(&crates_dir().join("core/src"), &mut files);
    rs_files(&crates_dir().join("contract/src"), &mut files);
    assert!(files.len() > 10, "sources found: {}", files.len());
    let mut bad = Vec::new();
    let mut used = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).expect("read source");
        let rel = f
            .strip_prefix(crates_dir())
            .expect("under crates")
            .to_string_lossy()
            .replace('\\', "/");
        bad.extend(violations(&rel, &src, EXCEPTIONS));
        used += EXCEPTIONS
            .iter()
            .filter(|(p, w, _)| rel.ends_with(p) && count_word(&strip_line_comments(&src), w) > 0)
            .count();
    }
    assert!(bad.is_empty(), "nondeterminism sources: {bad:?}");
    assert_eq!(used, EXCEPTIONS.len(), "every exception is still needed");
}

#[test]
fn core_and_contract_have_no_time_or_random_dependency() {
    for name in ["core", "contract"] {
        let text = std::fs::read_to_string(crates_dir().join(name).join("Cargo.toml"))
            .expect("read Cargo.toml");
        let value: toml::Table = toml::from_str(&text).expect("parse Cargo.toml");
        let deps = value
            .get("dependencies")
            .and_then(|d| d.as_table())
            .expect("dependencies table");
        for c in FORBIDDEN_CRATES {
            assert!(!deps.contains_key(*c), "{name} depends on {c}");
        }
    }
}

#[test]
fn detector_flags_synthetic_sources_and_ignores_comments() {
    let none: &[(&str, &str, usize)] = &[];
    assert!(!violations("x.rs", "let t = std::time::Instant::now();", none).is_empty());
    assert!(!violations("x.rs", "use std::collections::HashMap;", none).is_empty());
    assert!(violations("x.rs", "// SystemTime is not used\n/// Instant", none).is_empty());
    assert!(violations("x.rs", "let instantiate = 1;", none).is_empty());
    let one = [("x.rs", "HashSet", 2)];
    assert!(violations("x.rs", "let a: HashSet<u8>;", &one).is_empty());
    assert!(!violations(
        "x.rs",
        "let a: HashSet<u8>; let b: HashSet<u8>; let c: HashSet<u8>;",
        &one
    )
    .is_empty());
}
