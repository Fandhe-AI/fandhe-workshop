//! 層 1 の試験の索引（`docs/testing.md` 1.1。BUILD-7、TASK-83 1/3）と実在の照合。
//! 1.2 の性質テストの索引（TASK-83 2/3）も照合する。3 性質 × 2 生成（固定・誘導）がそろい、各行のファイルと関数が実在し、
//! ファイルの本文にケース数 `1_000` があることを確かめる（ファイル単位の確認で、関数ごとではない）。
//! 1.3 の変異注入の索引（TASK-83 3/3）も照合する。PoC の 27 件がちょうど 1 行ずつあり、`native` の行の定義ファイル・
//! 試験ファイル・関数が実在し、`browser` の行の試験ファイルと試験名の接頭辞が実在することを確かめる。
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

// ---- 1.2 性質テストの索引（TASK-83 2/3） ----

const PROPERTIES: [&str; 3] = [
    "古い進行による状態変化 0",
    "実体の間の混ざり 0",
    "復元の不一致 0",
];
const GENERATIONS: [&str; 2] = ["固定", "誘導"];

struct PRow {
    property: String,
    generation: String,
    file: String,
    fns: Vec<String>,
    /// 「網羅の確認」の列。`なし` のときは空。
    coverage: Vec<String>,
}

fn split_fns(cell: &str) -> Vec<String> {
    cell.split('・')
        .map(strip)
        .filter(|f| !f.is_empty() && f != "なし")
        .collect()
}

/// 「### 1.2 層 1」の節の表の行を読む。
fn property_rows(doc: &str) -> Vec<PRow> {
    let mut in_section = false;
    let mut out = Vec::new();
    for line in doc.lines() {
        if line.starts_with("### ") || line.starts_with("## ") {
            in_section = line.starts_with("### 1.2 層 1");
            continue;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        if cells.len() < 5 {
            continue;
        }
        let file = strip(cells[2]);
        if !file.ends_with(".rs") {
            continue;
        }
        out.push(PRow {
            property: strip(cells[0]),
            generation: strip(cells[1]),
            file,
            fns: split_fns(cells[3]),
            coverage: split_fns(cells[4]),
        });
    }
    out
}

/// 性質ごとに、固定と誘導の行がそろっているかを説明する文字列で返す（余計な名前も指摘する）。
fn property_gaps(rows: &[PRow]) -> Vec<String> {
    let mut out = Vec::new();
    for r in rows {
        if !PROPERTIES.contains(&r.property.as_str()) {
            out.push(format!("unknown property: {}", r.property));
        }
        if !GENERATIONS.contains(&r.generation.as_str()) {
            out.push(format!("unknown generation: {}", r.generation));
        }
        if r.fns.is_empty() {
            out.push(format!(
                "[{}/{}] {}: no test fns",
                r.property, r.generation, r.file
            ));
        }
    }
    for p in PROPERTIES {
        for g in GENERATIONS {
            if !rows.iter().any(|r| r.property == p && r.generation == g) {
                out.push(format!("missing row: {p} / {g}"));
            }
        }
    }
    out
}

/// 行ごとに、ファイル・関数（網羅の確認を含む）の実在と、本文のケース数 `1_000` を確かめる。
fn property_missing(rows: &[PRow], base: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for r in rows {
        let tag = format!("[{}/{}] {}", r.property, r.generation, r.file);
        let p = Path::new(&r.file);
        let ok_shape = !p.is_absolute()
            && r.file.starts_with("crates/")
            && r.file.contains("/tests/")
            && !r.file.contains("..");
        if !ok_shape {
            out.push(format!("{tag}: path shape"));
            continue;
        }
        let Ok(body) = fs::read_to_string(base.join(p)) else {
            out.push(format!("{tag}: file not found"));
            continue;
        };
        if !body.contains("1_000") {
            out.push(format!("{tag}: no 1_000 cases"));
        }
        for f in r.fns.iter().chain(&r.coverage) {
            if !body.contains(&format!("fn {f}(")) {
                out.push(format!("{tag}: fn {f} not found"));
            }
        }
    }
    out
}

#[test]
fn layer1_property_index_has_three_properties_by_two_generations() {
    let rows = property_rows(&doc());
    let gaps = property_gaps(&rows);
    assert!(gaps.is_empty(), "性質の索引に過不足がある: {gaps:#?}");
}

#[test]
fn layer1_property_index_entries_exist_with_1000_cases() {
    let m = property_missing(&property_rows(&doc()), &root());
    assert!(m.is_empty(), "性質の索引に実在しない項目がある: {m:#?}");
}

#[test]
fn layer1_property_index_checker_rejects_gaps() {
    let mk = |p: &str, g: &str, file: &str, f: &str| PRow {
        property: p.into(),
        generation: g.into(),
        file: file.into(),
        fns: vec![f.into()],
        coverage: vec![],
    };
    let me = "crates/core/tests/layer1_index.rs";
    let full: Vec<PRow> = PROPERTIES
        .iter()
        .flat_map(|p| GENERATIONS.iter().map(move |g| (*p, *g)))
        .map(|(p, g)| mk(p, g, me, "x"))
        .collect();
    assert!(property_gaps(&full).is_empty());
    // 生成の欠け
    let missing_guided: Vec<PRow> = full
        .into_iter()
        .filter(|r| r.generation != "誘導" || r.property != PROPERTIES[1])
        .collect();
    assert_eq!(property_gaps(&missing_guided).len(), 1);
    // 余計な名前
    assert!(property_gaps(&[mk("別の性質", "固定", me, "x")]).len() >= 2);

    // 存在しない関数・ファイル・不正なパス
    let ok = mk(
        PROPERTIES[0],
        "固定",
        me,
        "layer1_property_index_checker_rejects_gaps",
    );
    // このファイルは `1_000` を含む（本文の定数の例として下に書いている: 1_000）。
    assert!(property_missing(&[ok], &root()).is_empty());
    let bad_fn = mk(PROPERTIES[0], "固定", me, "no_such_fn");
    assert_eq!(property_missing(&[bad_fn], &root()).len(), 1);
    let bad_cov = PRow {
        coverage: vec!["no_such_cov".into()],
        ..mk(PROPERTIES[0], "固定", me, "doc")
    };
    assert_eq!(property_missing(&[bad_cov], &root()).len(), 1);
    let no_file = mk(
        PROPERTIES[0],
        "固定",
        "crates/core/tests/no_such_file.rs",
        "x",
    );
    assert_eq!(property_missing(&[no_file], &root()).len(), 1);
    // `1_000` の無いファイル
    let no_cases = mk(
        PROPERTIES[0],
        "固定",
        "crates/core/tests/tick_progress.rs",
        "matching_tick_advances_once",
    );
    assert_eq!(property_missing(&[no_cases], &root()).len(), 1);
    for bad in ["/etc/passwd", "../x.rs", "crates/core/src/lib.rs"] {
        assert_eq!(
            property_missing(&[mk(PROPERTIES[0], "固定", bad, "x")], &root()).len(),
            1,
            "{bad}"
        );
    }
}

// ---- 1.3 変異注入の索引（TASK-83 3/3） ----

/// PoC ごとの変異名。1.3 の表は、この 27 件がちょうど 1 行ずつあることを求める。
const POC_MUTATIONS: [(&str, &[&str]); 4] = [
    ("PoC-1", &["M1", "M2", "M3", "M4", "M5", "M6", "M7"]),
    ("PoC-4", &["X1", "X2", "X3", "X4", "X5", "X6", "X7", "X8"]),
    ("PoC-6", &["X-1", "X-2", "X-3", "X-4", "X-5", "X-6"]),
    ("PoC-2", &["M1", "M2", "M3", "M4", "M5", "M6"]),
];
const CRATES: [&str; 4] = ["contract", "core", "pack", "web"];

struct MRow {
    poc: String,
    name: String,
    id: String,
    krate: String,
    target: String,
    fns: Vec<String>,
    route: String,
}

/// 「### 1.3 層 1」の節の表の行を読む。
fn mutation_rows(doc: &str) -> Vec<MRow> {
    let mut in_section = false;
    let mut out = Vec::new();
    for line in doc.lines() {
        if line.starts_with("### ") || line.starts_with("## ") {
            in_section = line.starts_with("### 1.3 層 1");
            continue;
        }
        if !in_section || !line.trim_start().starts_with('|') {
            continue;
        }
        let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
        if cells.len() < 7 {
            continue;
        }
        let route = strip(cells[6]);
        if route != "native" && route != "browser" {
            continue; // 見出し行と区切り行
        }
        out.push(MRow {
            poc: strip(cells[0]),
            name: strip(cells[1]),
            id: strip(cells[2]),
            krate: strip(cells[3]),
            target: strip(cells[4]),
            fns: split_fns(cells[5]),
            route,
        });
    }
    out
}

/// 27 件の過不足と重複を説明する文字列で返す。
fn mutation_gaps(rows: &[MRow]) -> Vec<String> {
    let mut out = Vec::new();
    for (poc, names) in POC_MUTATIONS {
        for n in names {
            let c = rows.iter().filter(|r| r.poc == poc && r.name == *n).count();
            if c != 1 {
                out.push(format!("{poc} {n}: {c} rows"));
            }
        }
    }
    for r in rows {
        let known = POC_MUTATIONS
            .iter()
            .any(|(p, ns)| *p == r.poc && ns.contains(&r.name.as_str()));
        if !known {
            out.push(format!("unknown mutation: {} {}", r.poc, r.name));
        }
        if r.fns.is_empty() {
            out.push(format!("{} {}: no expected fns", r.poc, r.name));
        }
    }
    out
}

fn is_word(s: &str, extra: &[char]) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || extra.contains(&c))
}

/// 行ごとに、定義ファイル・試験ファイル・関数（`native`）または試験ファイル・試験名の接頭辞（`browser`）の実在を確かめる。
/// 文書のセルは信頼できない入力として、許可リストと文字種を満たすものだけからパスを組む。
fn mutation_missing(rows: &[MRow], base: &Path) -> Vec<String> {
    let mut out = Vec::new();
    for r in rows {
        let tag = format!("{} {}", r.poc, r.name);
        if r.route == "browser" {
            let file = r.target.trim_matches('`');
            let stem = file
                .strip_prefix("tools/browser/")
                .and_then(|f| f.strip_suffix(".test.mjs"));
            let Some(stem) = stem.filter(|s| is_word(s, &['-'])) else {
                out.push(format!("{tag}: browser path shape"));
                continue;
            };
            let Ok(body) = fs::read_to_string(base.join(format!("tools/browser/{stem}.test.mjs")))
            else {
                out.push(format!("{tag}: browser test not found"));
                continue;
            };
            for f in &r.fns {
                if !body.contains(f.as_str()) {
                    out.push(format!("{tag}: test name {f} not found"));
                }
            }
            continue;
        }
        if !CRATES.contains(&r.krate.as_str())
            || !is_word(&r.id, &['-'])
            || !is_word(&r.target, &[])
        {
            out.push(format!("{tag}: id/crate/target shape"));
            continue;
        }
        let def = base.join(format!(
            "crates/{}/tests/mutations/{}.mutation",
            r.krate, r.id
        ));
        let Ok(def_body) = fs::read_to_string(&def) else {
            out.push(format!("{tag}: definition {} not found", r.id));
            continue;
        };
        if !def_body.contains(&format!("id: {}\n", r.id))
            || !def_body.contains(&format!("test: {}\n", r.target))
        {
            out.push(format!("{tag}: definition id/test mismatch"));
        }
        let Ok(body) =
            fs::read_to_string(base.join(format!("crates/{}/tests/{}.rs", r.krate, r.target)))
        else {
            out.push(format!("{tag}: test file not found"));
            continue;
        };
        for f in &r.fns {
            if !body.contains(&format!("fn {f}(")) {
                out.push(format!("{tag}: fn {f} not found"));
            }
        }
    }
    out
}

#[test]
fn layer1_mutation_index_covers_all_poc_mutations() {
    let rows = mutation_rows(&doc());
    let gaps = mutation_gaps(&rows);
    assert!(gaps.is_empty(), "変異の索引に過不足がある: {gaps:#?}");
    assert_eq!(rows.len(), 27);
}

#[test]
fn layer1_mutation_index_entries_exist() {
    let m = mutation_missing(&mutation_rows(&doc()), &root());
    assert!(m.is_empty(), "変異の索引に実在しない項目がある: {m:#?}");
}

#[test]
fn layer1_mutation_index_checker_rejects_gaps() {
    let mk = |poc: &str, name: &str, id: &str, krate: &str, target: &str, f: &str| MRow {
        poc: poc.into(),
        name: name.into(),
        id: id.into(),
        krate: krate.into(),
        target: target.into(),
        fns: vec![f.into()],
        route: "native".into(),
    };
    // 実在する定義・試験・関数は通る
    let ok = mk(
        "PoC-1",
        "M1",
        "M1",
        "core",
        "op_sequence_properties",
        "properties_p1_to_p6_hold_after_every_op",
    );
    assert!(mutation_missing(&[ok], &root()).is_empty());
    // 存在しない ID・関数・試験、不正な crate・文字種、test の不一致
    let bad = [
        mk(
            "PoC-1",
            "M1",
            "NOPE-1",
            "core",
            "op_sequence_properties",
            "x",
        ),
        mk(
            "PoC-1",
            "M1",
            "M1",
            "core",
            "op_sequence_properties",
            "no_such_fn",
        ),
        mk("PoC-1", "M1", "M1", "core", "no_such_target", "x"),
        mk("PoC-1", "M1", "M1", "evil", "op_sequence_properties", "x"),
        mk(
            "PoC-1",
            "M1",
            "../M1",
            "core",
            "op_sequence_properties",
            "x",
        ),
        mk("PoC-1", "M1", "M1", "core", "../x", "x"),
        mk("PoC-1", "M1", "M2", "core", "op_sequence_properties", "x"),
    ];
    for b in &bad {
        assert!(
            !mutation_missing(std::slice::from_ref(b), &root()).is_empty(),
            "{}",
            b.id
        );
    }
    // ブラウザの行
    let br = |target: &str, f: &str| MRow {
        route: "browser".into(),
        ..mk("PoC-2", "M2", "-", "web", target, f)
    };
    assert!(mutation_missing(&[br("tools/browser/ime-input.test.mjs", "I-5")], &root()).is_empty());
    for (t, f) in [
        ("tools/browser/ime-input.test.mjs", "Z-99"),
        ("tools/browser/no-such.test.mjs", "I-5"),
        ("tools/browser/../x.test.mjs", "I-5"),
        ("/etc/passwd", "I-5"),
    ] {
        assert!(!mutation_missing(&[br(t, f)], &root()).is_empty(), "{t}");
    }
    // 過不足: 空、重複、未知
    assert_eq!(mutation_gaps(&[]).len(), 27);
    let one = mk("PoC-1", "M1", "M1", "core", "op_sequence_properties", "x");
    let dup = vec![
        mk("PoC-1", "M1", "M1", "core", "op_sequence_properties", "x"),
        one,
        mk("PoC-9", "M1", "M1", "core", "op_sequence_properties", "x"),
    ];
    let g = mutation_gaps(&dup);
    assert!(g.iter().any(|s| s.contains("PoC-1 M1: 2 rows")));
    assert!(g.iter().any(|s| s.contains("unknown mutation")));
}
