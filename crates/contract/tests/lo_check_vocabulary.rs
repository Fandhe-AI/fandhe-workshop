//! 確認の方式の語と、語彙の文書（`docs/vocabulary/vocabulary.md` 7 節）の `confirm` の一致の検査（TASK-291）。
//!
//! 文書を読むだけで作業ツリーへは書き込まない。変異注入は `docs/` を写さないため、適合試験とは別の試験の対象にする。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::lo_check::CheckMethod;

fn confirm_terms() -> BTreeSet<String> {
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
        if cells[0] == "confirm" {
            out.insert(cells[1].clone());
        }
    }
    out
}

#[test]
fn check_methods_plus_none_match_the_confirm_terms() {
    let mut mine: BTreeSet<String> = CheckMethod::ALL
        .iter()
        .map(|m| m.as_str().to_string())
        .collect();
    mine.insert("none".to_string());
    assert_eq!(mine, confirm_terms());
}
