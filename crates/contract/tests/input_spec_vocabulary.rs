//! 入力方式の語と、語彙の文書（`docs/vocabulary/vocabulary.md` 7 節）の一致の検査（TASK-44）。
//!
//! 文書を読むだけで作業ツリーへは書き込まない。`wait`・`cancel` は別タスク（EP-1・EP-2）が型を足すため除く。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::input_spec::InputKind;

fn input_terms() -> BTreeSet<String> {
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
        if cells[0] == "input" {
            out.insert(cells[1].clone());
        }
    }
    out
}

#[test]
fn input_kinds_match_the_vocabulary_document_except_wait_and_cancel() {
    let mut doc = input_terms();
    assert!(doc.remove("wait") && doc.remove("cancel"));
    let ours: BTreeSet<String> = InputKind::ALL
        .iter()
        .map(|k| k.as_str().to_string())
        .collect();
    assert_eq!(ours, doc);
}
