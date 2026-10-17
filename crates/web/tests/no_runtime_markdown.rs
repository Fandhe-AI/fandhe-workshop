//! 実行時に Markdown を解釈するコードが web に 0 件であることの機械的な検査（TASK-476 1/2。ART-7）。
//!
//! 文書のビューは、ビルド時に木へ変えた Markdown（`contract::doc::Block`）を描くだけである（方式 (c)）。
//! 解釈器（`pulldown-cmark`）は pack（ビルド時）にだけ置く。ここでは次の 3 点を確かめる。
//!
//! 1. `crates/web/Cargo.toml` の通常の依存（`[dependencies]` と `[target.*.dependencies]`）に、pack と Markdown の解釈器が無い
//! 2. `Cargo.lock` を web の通常の依存から推移的にたどった集合に `pulldown-cmark` が無い
//!    （Cargo.lock は dev-dependencies も含むため、開始点は manifest の通常の依存に限る）
//! 3. `crates/web/src/**/*.rs` のコード行（コメントと、`#[cfg(test)]` 以降の試験モジュールを除く）に、解釈器と変換器の呼び出しが無い
//!
//! 試作の教材パック（`bbs-tutorial`）の読み取りは、この repo に取り込まれていない（`testdata/README.md`）ため行わない。
//! 試験では `unwrap`・`expect` を使ってよい。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use toml::Table;

const WEB_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
const LOCK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));

/// web の通常の依存に入ってはならない crate。pack は変換器（`markdown::parse`）を持つ。
const BANNED: [&str; 6] = [
    "pulldown-cmark",
    "fandhe-workshop-pack",
    "comrak",
    "markdown",
    "markdown-it",
    "pulldown-cmark-escape",
];

fn parse(src: &str) -> Table {
    src.parse::<Table>().expect("TOML として読めること")
}

/// manifest の通常の依存の名前（`[dependencies]` と `[target.*.dependencies]`）。
fn normal_dependency_names(manifest: &Table) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    if let Some(t) = manifest.get("dependencies").and_then(|v| v.as_table()) {
        names.extend(t.keys().cloned());
    }
    if let Some(targets) = manifest.get("target").and_then(|v| v.as_table()) {
        for spec in targets.values() {
            if let Some(t) = spec.get("dependencies").and_then(|v| v.as_table()) {
                names.extend(t.keys().cloned());
            }
        }
    }
    names
}

#[test]
fn web_の通常の依存に_markdown_の解釈器と_pack_が無い() {
    let names = normal_dependency_names(&parse(WEB_MANIFEST));
    assert!(names.contains("fandhe-workshop-contract"), "{names:?}");
    for banned in BANNED {
        assert!(!names.contains(banned), "{banned} は通常の依存に置かない");
    }
}

#[test]
fn cargo_lock_で通常の依存からたどれる集合に_pulldown_cmark_が無い() {
    let lock = parse(LOCK);
    // 名前 -> 依存の名前。`"name 1.2.3"` の形（同名が複数の版にある場合）は名前だけにする。
    let mut graph: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for p in lock["package"].as_array().expect("package") {
        let name = p["name"].as_str().expect("name").to_owned();
        let deps = p
            .get("dependencies")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|d| d.as_str())
                    .map(|d| d.split(' ').next().unwrap_or(d).to_owned())
                    .collect()
            })
            .unwrap_or_default();
        graph.entry(name).or_default().extend::<Vec<String>>(deps);
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut stack: Vec<String> = normal_dependency_names(&parse(WEB_MANIFEST))
        .into_iter()
        .collect();
    while let Some(n) = stack.pop() {
        if seen.insert(n.clone()) {
            if let Some(next) = graph.get(&n) {
                stack.extend(next.iter().cloned());
            }
        }
    }
    assert!(seen.contains("fandhe-workshop-core"), "走査が空でないこと");
    for banned in BANNED {
        assert!(
            !seen.contains(banned),
            "{banned} が web の通常の依存から到達できる"
        );
    }
}

/// `.rs` を再帰的に集める。
fn rs_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in fs::read_dir(dir).expect("src を読めること").flatten() {
        let p = e.path();
        if p.is_dir() {
            rs_files(&p, out);
        } else if p.extension().is_some_and(|x| x == "rs") {
            out.push(p);
        }
    }
}

#[test]
fn web_のソースのコード行に_markdown_の解釈の呼び出しが無い() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut files = Vec::new();
    rs_files(&src, &mut files);
    assert!(!files.is_empty());
    let banned = ["pulldown_cmark", "fandhe_workshop_pack", "markdown::parse"];
    for f in files {
        let text = fs::read_to_string(&f).expect("ソースを読めること");
        for (i, line) in text.lines().enumerate() {
            let code = line.trim_start();
            // 試験モジュール（末尾に置く慣習）は配信物に入らず、pack を dev-dependency として使ってよい。
            if code == "#[cfg(test)]" {
                break;
            }
            if code.starts_with("//") {
                continue;
            }
            // 行末のコメントは除く。文字列の中の `//` を誤って切る場合は、見落とす側ではなく検出が増える側に倒れる。
            let code = code.split("//").next().unwrap_or(code);
            for b in banned {
                assert!(!code.contains(b), "{}:{}: {b}", f.display(), i + 1);
            }
        }
    }
}
