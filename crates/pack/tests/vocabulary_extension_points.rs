//! 名前つき拡張点 EP-1〜EP-5 の固定と、汎用名の検査（TASK-48、PACK-14）。
//!
//! 正は `docs/vocabulary/vocabulary.md`。この試験は、文書の表が期待どおりの名前・種別・語彙の版を
//! 持つことと、名前が付録 A の固有の語を含まないことを、文書を読んで確かめる。
//! 名前の期待値を変えるのは契約の版上げであり、文書の版・変更履歴と同じ変更で行う。
//! 固有の語の一覧は文書側に置き、試験は読むだけにする（正を 2 つにしないため）。

use std::fs;
use std::path::Path;

/// 期待する公開の一覧（ID、名前、種別、語彙の版）。
const EXPECTED: [(&str, &str, &str, &str); 5] = [
    ("EP-1", "input.wait", "input", "v1"),
    ("EP-2", "input.cancel", "input", "v1"),
    ("EP-3", "condition.param", "condition", "v1"),
    ("EP-4", "condition.clarifying", "condition", "v1"),
    ("EP-5", "view.card_list", "view", "v1"),
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Match {
    Substring,
    Word,
}

struct Term {
    origin: String,
    word: String,
    kind: Match,
}

fn doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/vocabulary/vocabulary.md");
    fs::read_to_string(&path).expect("語彙の文書を読めること")
}

fn table_cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().trim_matches('`').trim().to_string())
        .collect()
}

fn ep_rows(doc: &str) -> Vec<Vec<String>> {
    doc.lines()
        .filter(|l| l.starts_with("| EP-"))
        .map(table_cells)
        .collect()
}

fn forbidden_terms(doc: &str) -> Vec<Term> {
    let mut in_appendix = false;
    let mut terms = Vec::new();
    for line in doc.lines() {
        if line.starts_with("## ") {
            in_appendix = line.starts_with("## 付録 A");
            continue;
        }
        if !in_appendix || !line.starts_with('|') {
            continue;
        }
        let cells = table_cells(line);
        if cells.len() != 3 || cells[0] == "出所" || cells[0].starts_with("---") {
            continue;
        }
        let kind = match cells[2].as_str() {
            "部分一致" => Match::Substring,
            "語の一致" => Match::Word,
            other => panic!("未知の照合の値: {other}"),
        };
        terms.push(Term {
            origin: cells[0].clone(),
            word: cells[1].to_lowercase(),
            kind,
        });
    }
    terms
}

fn violations(name: &str, terms: &[Term]) -> Vec<String> {
    let lower = name.to_lowercase();
    let words: Vec<&str> = lower.split(['.', '_', '-']).collect();
    terms
        .iter()
        .filter(|t| match t.kind {
            Match::Substring => lower.contains(&t.word),
            Match::Word => words.contains(&t.word.as_str()),
        })
        .map(|t| t.word.clone())
        .collect()
}

#[test]
fn document_has_version_block() {
    let d = doc();
    let v = d
        .lines()
        .find_map(|l| l.strip_prefix("語彙の文書の版: "))
        .expect("版の行があること");
    assert!(
        v.trim().parse::<u32>().is_ok_and(|n| n > 0),
        "版が正の整数: {v}"
    );
    assert!(d.lines().any(|l| l == "状態: 固定（契約）"));
}

#[test]
fn extension_points_are_fixed() {
    let rows = ep_rows(&doc());
    assert_eq!(rows.len(), EXPECTED.len(), "EP の行の数");
    for (row, (id, name, kind, ver)) in rows.iter().zip(EXPECTED) {
        assert_eq!(
            (
                row[0].as_str(),
                row[1].as_str(),
                row[2].as_str(),
                row[3].as_str()
            ),
            (id, name, kind, ver)
        );
    }
}

#[test]
fn names_have_generic_shape() {
    for row in ep_rows(&doc()) {
        let name = &row[1];
        let parts: Vec<&str> = name.split('.').collect();
        assert_eq!(parts.len(), 2, "{}: 名前は <種別>.<語>: {name}", row[0]);
        assert_eq!(parts[0], row[2], "{}: 種別の列と名前の先頭が一致", row[0]);
        assert!(
            parts.iter().all(|p| !p.is_empty()
                && p.chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')),
            "{}: 使えない文字: {name}",
            row[0]
        );
    }
}

#[test]
fn forbidden_list_covers_three_origins() {
    let terms = forbidden_terms(&doc());
    assert!(!terms.is_empty());
    for origin in ["掲示板", "Claude Code", "GitHub"] {
        assert!(
            terms.iter().any(|t| t.origin == origin),
            "出所が無い: {origin}"
        );
    }
    for w in ["bbs", "claude", "github"] {
        assert!(
            terms
                .iter()
                .any(|t| t.word == w && t.kind == Match::Substring),
            "部分一致の語が無い: {w}"
        );
    }
}

#[test]
fn extension_point_names_are_generic() {
    let d = doc();
    let terms = forbidden_terms(&d);
    for row in ep_rows(&d) {
        let v = violations(&row[1], &terms);
        assert!(v.is_empty(), "{} {}: 固有の語 {v:?}", row[0], row[1]);
    }
}

#[test]
fn check_rejects_specific_names() {
    let terms = forbidden_terms(&doc());
    for bad in [
        "view.github_card",
        "input.pr",
        "condition.claudecode",
        "view.thread_list",
    ] {
        assert!(!violations(bad, &terms).is_empty(), "検出されない: {bad}");
    }
    for ok in ["input.wait", "view.card_list", "condition.repository_count"] {
        assert!(violations(ok, &terms).is_empty(), "誤検出: {ok}");
    }
}
