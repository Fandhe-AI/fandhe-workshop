//! 左の成果物ビューの型・識別子が教材固有の語を含まないことの検査（TASK-467 1/2。W-22）。
//!
//! 固有の語の一覧の正は `docs/vocabulary/vocabulary.md` の付録 A で、ここでは読むだけにする
//! （正を 2 つにしないため）。解釈の規則は `crates/pack/tests/vocabulary_extension_points.rs` と同じである。

use std::fs;
use std::path::Path;

use fandhe_workshop_web::artifact_view::{
    RowKind, Variant, ViewKind, ACTION_ATTR, HIDDEN_ATTR, PAYLOAD_ATTR, POINTABLE_TABINDEX,
    POINT_ACTION, ROW_ATTR, TABINDEX_ATTR, VIEW_ATTR, VIEW_LIST_TESTID,
};
use fandhe_workshop_web::card_view::{
    CardState, CARD_DETAIL_TESTID, CARD_LABEL_TESTID, CARD_TAGS_TESTID, TAGS_SEPARATOR,
};
use fandhe_workshop_web::check_view::{
    CheckState, CHECK_DETAIL_TESTID, CHECK_LABEL_TESTID, DETAIL_CLASS, ICON_CLASS,
};

#[derive(Clone, Copy, PartialEq)]
enum Match {
    Substring,
    Word,
}

struct Term {
    word: String,
    kind: Match,
}

fn read(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(rel);
    fs::read_to_string(&path).expect("読めること")
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().trim_matches('`').trim().to_string())
        .collect()
}

fn forbidden_terms() -> Vec<Term> {
    let doc = read("../../docs/vocabulary/vocabulary.md");
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
        let c = cells(line);
        if c.len() != 3 || c[0] == "出所" || c[0].starts_with("---") {
            continue;
        }
        let kind = match c[2].as_str() {
            "部分一致" => Match::Substring,
            "語の一致" => Match::Word,
            other => panic!("未知の照合の値: {other}"),
        };
        terms.push(Term {
            word: c[1].to_lowercase(),
            kind,
        });
    }
    terms
}

/// 識別子を snake_case と CamelCase の語に分けて小文字にする。
fn words_of(ident: &str) -> Vec<String> {
    let mut words = Vec::new();
    for part in ident.split('_') {
        let mut cur = String::new();
        let chars: Vec<char> = part.chars().collect();
        for (i, ch) in chars.iter().enumerate() {
            let boundary = ch.is_uppercase()
                && i > 0
                && (chars[i - 1].is_lowercase()
                    || chars.get(i + 1).is_some_and(|n| n.is_lowercase()));
            if boundary && !cur.is_empty() {
                words.push(cur.to_lowercase());
                cur.clear();
            }
            cur.push(*ch);
        }
        if !cur.is_empty() {
            words.push(cur.to_lowercase());
        }
    }
    words
}

fn violations(ident: &str, terms: &[Term]) -> Vec<String> {
    let lower = ident.to_lowercase();
    let words = words_of(ident);
    terms
        .iter()
        .filter(|t| match t.kind {
            Match::Substring => lower.contains(&t.word),
            Match::Word => words.contains(&t.word),
        })
        .map(|t| t.word.clone())
        .collect()
}

/// コメント行を除いたソースから識別子を取り出す。
fn identifiers(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let code = line.trim_start();
        if code.starts_with("//") {
            continue;
        }
        let mut cur = String::new();
        for ch in code.chars().chain(std::iter::once(' ')) {
            if ch.is_alphanumeric() || ch == '_' {
                cur.push(ch);
            } else if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        }
    }
    out
}

#[test]
fn 付録_a_を読めている() {
    assert!(!forbidden_terms().is_empty());
}

#[test]
fn 検査は固有の語を検出できる() {
    let terms = forbidden_terms();
    for bad in ["GithubCard", "pr_list", "issue_row"] {
        assert!(!violations(bad, &terms).is_empty(), "{bad}");
    }
}

#[test]
fn 型と識別子に固有の語が無い() {
    let terms = forbidden_terms();
    let mut found = Vec::new();
    for file in [
        "src/artifact_view.rs",
        "tests/artifact_view_sample.rs",
        "src/card_view.rs",
        "src/check_view.rs",
    ] {
        let src = read(file);
        for id in identifiers(&src) {
            let v = violations(&id, &terms);
            if !v.is_empty() {
                found.push(format!("{file} {id}: {v:?}"));
            }
        }
    }
    assert!(found.is_empty(), "固有の語: {found:?}");
}

#[test]
fn 綴りに固有の語が無い() {
    let terms = forbidden_terms();
    let spellings = ViewKind::ALL
        .iter()
        .map(|k| k.as_str())
        .chain(ViewKind::ALL.iter().map(|k| k.list_field()))
        .chain(RowKind::ALL.iter().map(|k| k.as_str()))
        .chain(Variant::ALL.iter().map(|v| v.as_str()))
        .chain(CardState::ALL.iter().map(|c| c.text()))
        .chain(
            CheckState::ALL
                .iter()
                .flat_map(|c| [c.text(), c.label(), c.glyph()]),
        )
        .chain([
            CHECK_LABEL_TESTID,
            CHECK_DETAIL_TESTID,
            ICON_CLASS,
            DETAIL_CLASS,
        ])
        .chain([
            TAGS_SEPARATOR,
            CARD_LABEL_TESTID,
            CARD_DETAIL_TESTID,
            CARD_TAGS_TESTID,
        ])
        .chain([
            VIEW_LIST_TESTID,
            VIEW_ATTR,
            POINT_ACTION,
            ACTION_ATTR,
            PAYLOAD_ATTR,
            ROW_ATTR,
            TABINDEX_ATTR,
            POINTABLE_TABINDEX,
            HIDDEN_ATTR,
        ]);
    for s in spellings {
        assert!(violations(s, &terms).is_empty(), "{s}");
    }
}
