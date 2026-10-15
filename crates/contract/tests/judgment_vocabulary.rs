//! 判定の型の語と、語彙の文書（`docs/vocabulary/vocabulary.md` 7 節）の一致の検査（TASK-256）。
//!
//! 文書を読むだけで作業ツリーへは書き込まない。変異注入は `docs/` を写さないため、適合試験とは別の試験の対象にする。
//! 表の読み方は pack の `tests/vocabulary_terms.rs` と同じ方式で、contract は pack に依存できないため小さく写す。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::judgment::*;

fn vocab_rows(kind: &str) -> BTreeSet<String> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/vocabulary/vocabulary.md");
    let doc = std::fs::read_to_string(p).expect("語彙の文書を読めること");
    let strip = |c: &str| c.trim().trim_matches('`').trim().to_string();
    let mut inside = false;
    let mut header_seen = false;
    let mut out = BTreeSet::new();
    for line in doc.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with("## 7. ");
            header_seen = false;
            continue;
        }
        if !inside || !line.starts_with('|') {
            continue;
        }
        if !header_seen {
            header_seen = true;
            continue;
        }
        if line.starts_with("|---") {
            continue;
        }
        let cells: Vec<String> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(strip)
            .collect();
        if cells[0] == kind {
            out.insert(cells[1].clone());
        }
    }
    out
}

#[test]
fn terms_match_the_vocabulary_document() {
    let rejection = vocab_rows("rejection");
    let cats: BTreeSet<String> = Category::ALL
        .iter()
        .map(|c| c.as_str().to_string())
        .collect();
    assert_eq!(rejection, cats);

    let result = vocab_rows("result");
    let expected: BTreeSet<String> = ["accepted", "rejected", "clarified"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(result, expected);
    let kinds: BTreeSet<String> = JudgmentKind::ALL
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();
    assert!(result.is_subset(&kinds));
}
