//! 診断コードの一覧（`docs/diagnostics.md`）と実装の列挙（`DiagCode::ALL`）の一致の試験（TASK-400 1/2・AUTHOR-11）。
//!
//! 一覧は 3 つの区分（2 節の本表、4 節の参照のみ、5 節の本表外）に分かれる。区分を文書の構造から取り出し、
//! 和集合が実装の全件と一致することを確かめる。コードの一覧も件数も試験に書かない（書くと、一覧と実装を同時に
//! 誤っても通るため）。文書は読むだけで、作業ツリーを書き換えない。節は見出しの番号（`## 2.` など）で探す。

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use fandhe_workshop_contract::limits::PackLimitCode;
use fandhe_workshop_pack::DiagCode;

fn doc() -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/diagnostics.md"))
        .expect("診断コードの一覧を読めること")
}

/// `## <number>.` で始まる節の本文（次の `## ` の前まで）。無ければ panic する（fail-closed）。
fn section(doc: &str, number: u32) -> String {
    let prefix = format!("## {number}.");
    let mut lines = doc.lines();
    lines
        .by_ref()
        .find(|l| l.starts_with(&prefix))
        .unwrap_or_else(|| panic!("見出しが無い: {prefix}"));
    lines
        .take_while(|l| !l.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_code_shape(s: &str) -> bool {
    s.strip_prefix("E-").is_some_and(|rest| {
        rest.split('-').all(|seg| {
            !seg.is_empty()
                && seg
                    .chars()
                    .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        })
    })
}

/// バッククォートで囲まれた `E-` 始まりの語（`*` を含むワイルドカードは除く）を初出の順に返す。
/// 節の本文は同じコードを説明で繰り返し述べるため、節の中では重複を除く（本表の重複は表の行で見る）。
fn backticked_codes(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in text.split('`').skip(1).step_by(2) {
        if t.starts_with("E-") && !t.contains('*') && !out.iter().any(|o| o == t) {
            out.push(t.to_string());
        }
    }
    out
}

/// 本表の 1 列目のコード（見出し行・区切り行を除く）。形が崩れていれば panic する。
fn table_codes(sec: &str) -> Vec<String> {
    sec.lines()
        .filter(|l| l.starts_with('|'))
        .skip(2)
        .map(|l| {
            let cell = l.trim_start_matches('|').split('|').next().unwrap().trim();
            let code = cell.trim_matches('`');
            assert!(is_code_shape(code), "本表の 1 列目がコードの形でない: {l}");
            code.to_string()
        })
        .collect()
}

struct Buckets {
    table: Vec<String>,
    reference: Vec<String>,
    excluded: Vec<String>,
}

fn parse_buckets(doc: &str) -> Buckets {
    Buckets {
        table: table_codes(&section(doc, 2)),
        reference: backticked_codes(&section(doc, 4)),
        excluded: backticked_codes(&section(doc, 5)),
    }
}

fn set(v: &[String]) -> BTreeSet<&str> {
    v.iter().map(String::as_str).collect()
}

/// 区分の重複（区分の中の重複、区分どうしの重複）を名前つきで返す。
fn overlaps(b: &Buckets) -> Vec<String> {
    let mut out = Vec::new();
    for (name, v) in [
        ("本表", &b.table),
        ("参照のみ", &b.reference),
        ("本表外", &b.excluded),
    ] {
        let mut own = BTreeSet::new();
        for c in v {
            if !own.insert(c.as_str()) {
                out.push(format!("{name}の中で重複: {c}"));
            }
        }
    }
    let (t, r, e) = (set(&b.table), set(&b.reference), set(&b.excluded));
    for c in t.intersection(&r) {
        out.push(format!("本表と参照のみに重複: {c}"));
    }
    for c in t.intersection(&e) {
        out.push(format!("本表と本表外に重複: {c}"));
    }
    for c in r.intersection(&e) {
        out.push(format!("参照のみと本表外に重複: {c}"));
    }
    out
}

/// 一覧と実装の食い違いを「一覧にだけある」「実装にだけある」の形で返す。空なら一致。
fn compare(b: &Buckets, implemented: &[&str]) -> Vec<String> {
    let listed: BTreeSet<&str> = b
        .table
        .iter()
        .chain(&b.reference)
        .chain(&b.excluded)
        .map(String::as_str)
        .collect();
    let imp: BTreeSet<&str> = implemented.iter().copied().collect();
    let mut out = Vec::new();
    for c in listed.difference(&imp) {
        out.push(format!("一覧にだけある: {c}"));
    }
    for c in imp.difference(&listed) {
        out.push(format!("実装にだけある: {c}"));
    }
    out
}

fn implemented() -> Vec<&'static str> {
    DiagCode::ALL.iter().map(|c| c.as_str()).collect()
}

/// `marker` の後ろに最初に現れる整数。無ければ panic する。
fn number_after(text: &str, marker: &str) -> usize {
    let at = text
        .find(marker)
        .unwrap_or_else(|| panic!("記述が無い: {marker}"));
    let digits: String = text[at + marker.len()..]
        .chars()
        .skip_while(|c| !c.is_ascii_digit())
        .take_while(char::is_ascii_digit)
        .collect();
    digits
        .parse()
        .unwrap_or_else(|_| panic!("数が読めない: {marker}"))
}

#[test]
fn author11_doc_and_impl_codes_match() {
    let diffs = compare(&parse_buckets(&doc()), &implemented());
    assert!(
        diffs.is_empty(),
        "一覧と実装が食い違う:\n{}",
        diffs.join("\n")
    );
}

#[test]
fn author11_buckets_are_disjoint_and_unique() {
    let found = overlaps(&parse_buckets(&doc()));
    assert!(found.is_empty(), "区分が重なる:\n{}", found.join("\n"));
}

#[test]
fn author11_recorded_counts_match() {
    let d = doc();
    let b = parse_buckets(&d);
    let s2 = section(&d, 2);
    let s3 = section(&d, 3);
    assert_eq!(number_after(&s2, "一覧の件数」とする"), b.table.len());
    assert_eq!(number_after(&s3, "本表は"), b.table.len());
    let line = s3
        .lines()
        .find(|l| l.contains("`DiagCode::ALL` は"))
        .expect("実装の列挙との対応の記述");
    assert_eq!(
        number_after(line, "`DiagCode::ALL` は"),
        DiagCode::ALL.len()
    );
    assert_eq!(number_after(line, "本表 "), b.table.len());
    assert_eq!(number_after(line, "本表外 "), b.excluded.len());
    assert_eq!(number_after(line, "上限超過 "), b.reference.len());
}

#[test]
fn author11_limit_codes_follow_contract() {
    let b = parse_buckets(&doc());
    let reference = set(&b.reference);
    let all = implemented();
    let mut contract = BTreeSet::new();
    for c in PackLimitCode::ALL {
        let s = c.as_str();
        assert!(reference.contains(s), "参照のみの区分に無い: {s}");
        assert!(all.contains(&s), "DiagCode::ALL に無い: {s}");
        contract.insert(s);
    }
    // 素の E-LIMIT は PackLimitCode に無い（4 節の記述）。それ以外の差があってはならない。
    let extra: Vec<&str> = reference.difference(&contract).copied().collect();
    assert_eq!(extra, ["E-LIMIT"]);
}

#[test]
fn author11_detects_code_only_in_doc() {
    let d = doc().replace(
        "\n| `E-PARSE` |",
        "\n| `E-FAKE-ONLY-DOC` | x | y | z |\n| `E-PARSE` |",
    );
    let diffs = compare(&parse_buckets(&d), &implemented());
    assert!(diffs.contains(&"一覧にだけある: E-FAKE-ONLY-DOC".to_string()));
}

#[test]
fn author11_detects_code_only_in_impl() {
    let d: String = doc()
        .lines()
        .filter(|l| !l.starts_with("| `E-REF-ELEMENT` |"))
        .collect::<Vec<_>>()
        .join("\n");
    let diffs = compare(&parse_buckets(&d), &implemented());
    assert!(diffs.contains(&"実装にだけある: E-REF-ELEMENT".to_string()));
}

#[test]
fn author11_detects_overlapping_buckets() {
    let d = doc().replace(
        "\n| `E-PARSE` |",
        "\n| `E-PLAY-INCOMPLETE` | x | y | z |\n| `E-PARSE` |",
    );
    let found = overlaps(&parse_buckets(&d));
    assert!(found.contains(&"本表と本表外に重複: E-PLAY-INCOMPLETE".to_string()));
}
