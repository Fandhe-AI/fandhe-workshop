//! core・contract のソースに、時刻・乱数・大域状態・反復順が不定の集合の源が無いことの静的な検査（TASK-218 1/3。EVT-22・EVT-16）。
//!
//! 「同じ入力列から同じ結果」を `determinism.rs` の実行で確かめても、たまたま一致しただけの非決定性は見逃す。
//! 源になりうる語をソースから 0 件に固定し、例外は 1 件だけ（件数つき）にする。新しい源が入ると失敗する（fail-closed）。
//! 依存の推移的な閉包は `make check-deps` の領分で、ここでは `[dependencies]` の直接の依存だけを見る。
//! 照合はコメントを除き（文字列中の `//` はコメントと見なさない）、識別子の境界で行う。マクロ・文字列の中身は見分けない（限界は `docs/testing.md` 5.277）。

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

/// 行コメントとブロックコメントを除く。文字列・raw 文字列・文字リテラルの中の `//` は
/// コメントの開始と見なさず、中身はそのまま残す（後続の実コードを検査から落とさないため）。
fn strip_line_comments(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < c.len() {
        let ch = c[i];
        if ch == '/' && c.get(i + 1) == Some(&'/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
        } else if ch == '/' && c.get(i + 1) == Some(&'*') {
            let mut depth = 1;
            i += 2;
            while i < c.len() && depth > 0 {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth -= 1;
                    i += 2;
                } else {
                    if c[i] == '\n' {
                        out.push('\n');
                    }
                    i += 1;
                }
            }
        } else if ch == 'r' && (i == 0 || !is_id(c[i - 1])) && raw_open(&c, i).is_some() {
            let (hashes, body) = raw_open(&c, i).expect("checked");
            out.extend(&c[i..body]);
            i = body;
            while i < c.len() {
                if c[i] == '"' && (1..=hashes).all(|k| c.get(i + k) == Some(&'#')) {
                    out.extend(&c[i..=i + hashes]);
                    i += hashes + 1;
                    break;
                }
                out.push(c[i]);
                i += 1;
            }
        } else if ch == '"' {
            out.push(ch);
            i += 1;
            while i < c.len() {
                if c[i] == '\\' && i + 1 < c.len() {
                    out.push(c[i]);
                    out.push(c[i + 1]);
                    i += 2;
                    continue;
                }
                out.push(c[i]);
                i += 1;
                if c[i - 1] == '"' {
                    break;
                }
            }
        } else if ch == '\'' && c.get(i + 1) == Some(&'\\') {
            // バックスラッシュと次の 1 文字を組で消費し、その後の最初の引用符で閉じる（`'\\'` の閉じ引用符を誤らない）。
            out.push(ch);
            out.push(c[i + 1]);
            i += 2;
            if i < c.len() {
                out.push(c[i]);
                i += 1;
            }
            while i < c.len() {
                out.push(c[i]);
                i += 1;
                if c[i - 1] == '\'' {
                    break;
                }
            }
        } else if ch == '\'' && c.get(i + 2) == Some(&'\'') {
            out.extend(&c[i..i + 3]);
            i += 3;
        } else {
            out.push(ch);
            i += 1;
        }
    }
    out
}

/// `c[i]` が raw 文字列の開始（`r"`・`r#"` など）なら、`#` の数と本体の開始位置を返す。
fn raw_open(c: &[char], i: usize) -> Option<(usize, usize)> {
    let mut j = i + 1;
    while c.get(j) == Some(&'#') {
        j += 1;
    }
    (c.get(j) == Some(&'"')).then_some((j - i - 1, j + 1))
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

/// 直接の依存の実際の crate 名を返す。`[dependencies]`・`[build-dependencies]`・`[target.*.dependencies]` を見て、
/// `package = "x"` の改名は x で、`workspace = true` は workspace 側の定義（`package` を含む）で解決する。
fn dependency_crates(manifest: &toml::Table, workspace: Option<&toml::Table>) -> Vec<String> {
    let ws_deps = workspace
        .and_then(|w| w.get("workspace"))
        .and_then(|w| w.get("dependencies"))
        .and_then(|d| d.as_table());
    let mut tables: Vec<&toml::Table> = Vec::new();
    for key in ["dependencies", "build-dependencies"] {
        tables.extend(manifest.get(key).and_then(|d| d.as_table()));
    }
    if let Some(targets) = manifest.get("target").and_then(|t| t.as_table()) {
        for t in targets.values().filter_map(|t| t.as_table()) {
            for key in ["dependencies", "build-dependencies"] {
                tables.extend(t.get(key).and_then(|d| d.as_table()));
            }
        }
    }
    let package_of = |v: &toml::Value| -> Option<String> {
        v.get("package")
            .and_then(|p| p.as_str())
            .map(str::to_string)
    };
    let mut out = Vec::new();
    for table in tables {
        for (key, value) in table {
            out.push(key.clone());
            if let Some(p) = package_of(value) {
                out.push(p);
            }
            let inherits = value.get("workspace").and_then(|w| w.as_bool()) == Some(true);
            if inherits {
                if let Some(def) = ws_deps.and_then(|d| d.get(key)) {
                    if let Some(p) = package_of(def) {
                        out.push(p);
                    }
                }
            }
        }
    }
    out
}

#[test]
fn core_and_contract_have_no_time_or_random_dependency() {
    let root_text =
        std::fs::read_to_string(crates_dir().parent().expect("repo root").join("Cargo.toml"))
            .expect("read workspace Cargo.toml");
    let root: toml::Table = toml::from_str(&root_text).expect("parse workspace Cargo.toml");
    for name in ["core", "contract"] {
        let text = std::fs::read_to_string(crates_dir().join(name).join("Cargo.toml"))
            .expect("read Cargo.toml");
        let value: toml::Table = toml::from_str(&text).expect("parse Cargo.toml");
        assert!(
            value.get("dependencies").is_some_and(|d| d.is_table()),
            "dependencies table"
        );
        let deps = dependency_crates(&value, Some(&root));
        for c in FORBIDDEN_CRATES {
            assert!(!deps.iter().any(|d| d == c), "{name} depends on {c}");
        }
    }
}

#[test]
fn dependency_detector_sees_renames_targets_and_workspace_entries() {
    let parse = |s: &str| s.parse::<toml::Table>().expect("toml");
    let renamed = parse("[dependencies]\nclock = { package = \"time\", version = \"0.3\" }");
    assert!(dependency_crates(&renamed, None).contains(&"time".to_string()));
    let target = parse("[target.'cfg(unix)'.dependencies]\nrand = \"0.8\"");
    assert!(dependency_crates(&target, None).contains(&"rand".to_string()));
    let member = parse("[dependencies]\nclock.workspace = true");
    let ws = parse("[workspace.dependencies]\nclock = { package = \"chrono\", version = \"0.4\" }");
    assert!(dependency_crates(&member, Some(&ws)).contains(&"chrono".to_string()));
    let plain = parse("[dependencies]\nserde = \"1\"");
    assert!(!dependency_crates(&plain, None).iter().any(|d| d == "time"));
}

#[test]
fn detector_flags_synthetic_sources_and_ignores_comments() {
    let none: &[(&str, &str, usize)] = &[];
    assert!(!violations("x.rs", "let t = std::time::Instant::now();", none).is_empty());
    assert!(!violations("x.rs", "use std::collections::HashMap;", none).is_empty());
    assert!(violations("x.rs", "// SystemTime is not used\n/// Instant", none).is_empty());
    assert!(violations("x.rs", "let instantiate = 1;", none).is_empty());
    // 文字列・raw 文字列・文字リテラルの中の `//` は後続の実コードを隠さない。
    let url = "let url = \"https://example.com\"; let t = std::time::Instant::now();";
    assert!(!violations("x.rs", url, none).is_empty());
    let raw = "let u = r#\"a//b\"#; let m = HashMap::new();";
    assert!(!violations("x.rs", raw, none).is_empty());
    let ch = "let c = '\"'; let u = \"//\"; let t = SystemTime::now();";
    assert!(!violations("x.rs", ch, none).is_empty());
    // 本物の行コメント・ブロックコメントは従来どおり除く。
    // `'\\'` の閉じ引用符をエスケープ中と誤らない（後ろの本物のコメントは除かれ、後ろの実コードは残る）。
    assert!(violations("x.rs", "let c = '\\\\'; // Instant", none).is_empty());
    assert!(!violations("x.rs", "let c = '\\\\'; let t = Instant::now();", none).is_empty());
    assert!(violations("x.rs", "let s = \"a\"; // Instant\n/* HashMap */", none).is_empty());
    let one = [("x.rs", "HashSet", 2)];
    assert!(violations("x.rs", "let a: HashSet<u8>;", &one).is_empty());
    assert!(!violations(
        "x.rs",
        "let a: HashSet<u8>; let b: HashSet<u8>; let c: HashSet<u8>;",
        &one
    )
    .is_empty());
}
