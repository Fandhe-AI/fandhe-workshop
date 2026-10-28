//! 層 1 の試験の索引（`docs/testing.md` 1.1。BUILD-7、TASK-83 1/3）と実在の照合。
//!
//! 索引の表が挙げる試験ファイルと試験関数が実在することを確かめ、改名・削除で索引だけが古くなるのを防ぐ。
//! 区分（進行管理・分岐・拒否の分類・状態の復元・履歴の変換）の過不足も確かめる。
//! 文書の内容は信頼できない入力として扱い、パスは `crates/<crate>/tests/*.rs` の形に限る。
//! 呼び出し元は `cargo test`（`make test`）のみである。変異注入の対象にはしない
//! （`scripts/check-mutations.sh` の一時コピーに `docs/` が無いため）。

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

const CATEGORIES: [&str; 5] = ["進行管理", "分岐", "拒否の分類", "状態の復元", "履歴の変換"];

struct Row {
    category: String,
    file: String,
    fns: Vec<String>,
}

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn strip(cell: &str) -> String {
    cell.trim().trim_matches('`').trim().to_string()
}

/// 「### 1.1 層 1」の節の表の行を読む。
fn rows(doc: &str) -> Vec<Row> {
    let mut in_section = false;
    let mut out = Vec::new();
    for line in doc.lines() {
        if line.starts_with("### ") || line.starts_with("## ") {
            in_section = line.starts_with("### 1.1 層 1");
            continue;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        if cells.len() < 3 {
            continue;
        }
        let file = strip(cells[1]);
        if !file.ends_with(".rs") {
            continue; // 見出し行と区切り行
        }
        out.push(Row {
            category: strip(cells[0]),
            file,
            fns: cells[2]
                .split('・')
                .map(strip)
                .filter(|f| !f.is_empty())
                .collect(),
        });
    }
    out
}

/// 行ごとに、ファイルと関数の実在を確かめ、見つからないものを説明する文字列で返す。
fn missing(rows: &[Row], base: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for r in rows {
        let p = Path::new(&r.file);
        let ok_shape = !p.is_absolute()
            && r.file.starts_with("crates/")
            && r.file.contains("/tests/")
            && !r.file.contains("..");
        if !ok_shape {
            out.push(format!("[{}] {}: path shape", r.category, r.file));
            continue;
        }
        let Ok(body) = fs::read_to_string(base.join(p)) else {
            out.push(format!("[{}] {}: file not found", r.category, r.file));
            continue;
        };
        for f in &r.fns {
            if !body.contains(&format!("fn {f}(")) {
                out.push(format!("[{}] {}: fn {f} not found", r.category, r.file));
            }
        }
    }
    out
}

fn doc() -> String {
    fs::read_to_string(root().join("docs/testing.md")).expect("試験の手順の文書を読めること")
}

#[test]
fn layer1_index_has_all_five_categories() {
    let rows = rows(&doc());
    let found: BTreeSet<&str> = rows.iter().map(|r| r.category.as_str()).collect();
    let want: BTreeSet<&str> = CATEGORIES.into_iter().collect();
    assert_eq!(found, want);
    assert!(rows.iter().all(|r| !r.fns.is_empty()));
}

#[test]
fn layer1_index_entries_exist() {
    let m = missing(&rows(&doc()), &root());
    assert!(m.is_empty(), "索引に実在しない項目がある: {m:#?}");
}

#[test]
fn layer1_index_checker_rejects_missing_names() {
    let mk = |file: &str, f: &str| Row {
        category: "分岐".into(),
        file: file.into(),
        fns: vec![f.into()],
    };
    let ok = "crates/core/tests/layer1_index.rs";
    assert!(missing(&[mk(ok, "layer1_index_entries_exist")], &root()).is_empty());
    assert_eq!(missing(&[mk(ok, "no_such_fn")], &root()).len(), 1);
    assert_eq!(
        missing(&[mk("crates/core/tests/no_such_file.rs", "x")], &root()).len(),
        1
    );
    for bad in ["/etc/passwd", "../x.rs", "crates/core/src/lib.rs"] {
        assert_eq!(missing(&[mk(bad, "x")], &root()).len(), 1, "{bad}");
    }
}
