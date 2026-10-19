//! 履歴の形式の文書（`docs/history-format.md`）と型の一致の検査（TASK-188 2/2）。
//!
//! 正は型と rustdoc で、文書は一覧である。文書だけが古くなるのを防ぐため、冒頭の契約の版と、
//! 2 節（包み）・3 節（4 軸の版）の表の欄名の並びを、型の直列化の順と照合する。他の記述は確かめない。
//! 呼び出し元は `cargo test`（`make test`）のみである。

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use fandhe_workshop_contract::history::{HistoryEnvelope, HISTORY_CONTRACT_VERSION};
use fandhe_workshop_contract::history_version::{
    HistoryVersions, HISTORY_VERSION_CONTRACT_VERSION,
};

fn doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/history-format.md");
    fs::read_to_string(&path).expect("履歴の形式の文書を読めること")
}

/// 指定した見出しで始まる節の本文（次の `## ` の前まで）。
fn section(doc: &str, heading: &str) -> String {
    let mut lines = doc.lines().skip_while(|l| !l.starts_with(heading));
    assert!(lines.next().is_some(), "節が見つかること: {heading}");
    lines
        .take_while(|l| !l.starts_with("## "))
        .collect::<Vec<_>>()
        .join("\n")
}

/// 節の表の本体行の指定列から欄名を取る（`・` 区切りは分割し、バッククォートを外す）。
fn column_names(section: &str, col: usize) -> Vec<String> {
    let mut out = Vec::new();
    for line in section.lines().filter(|l| l.starts_with('|')) {
        let cells: Vec<&str> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        let cell = cells[col];
        if cell.starts_with('`') {
            out.extend(cell.split('・').map(|n| n.trim_matches('`').to_string()));
        }
    }
    out
}

/// 直列化した JSON 文字列の中での、指定したキーの出現順。
fn key_order(json: &str, keys: &[String]) -> Vec<String> {
    let mut found: Vec<(usize, String)> = keys
        .iter()
        .map(|k| {
            let pos = json
                .find(&format!("\"{k}\":"))
                .unwrap_or_else(|| panic!("キーが直列化に現れること: {k}"));
            (pos, k.clone())
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, k)| k).collect()
}

fn versions() -> HistoryVersions {
    HistoryVersions {
        engine: String::new(),
        ir_format: 1,
        vocab: String::new(),
        content: String::new(),
        content_hash: String::new(),
        refs: BTreeMap::new(),
    }
}

#[test]
fn doc_header_versions_match_constants() {
    let d = doc();
    let value = |label: &str| -> u32 {
        d.lines()
            .find_map(|l| l.strip_prefix(label))
            .unwrap_or_else(|| panic!("冒頭の囲みに {label} があること"))
            .trim()
            .parse()
            .expect("版が整数であること")
    };
    assert_eq!(
        value("対象の契約の版（HISTORY_CONTRACT_VERSION）:"),
        HISTORY_CONTRACT_VERSION
    );
    assert_eq!(
        value("対象の契約の版（HISTORY_VERSION_CONTRACT_VERSION）:"),
        HISTORY_VERSION_CONTRACT_VERSION
    );
}

#[test]
fn doc_envelope_fields_match_serialization_order() {
    let env: HistoryEnvelope<String, String> = HistoryEnvelope {
        format: String::new(),
        history_format: 1,
        pack_id: String::new(),
        versions: versions(),
        start: String::new(),
        ops: Vec::new(),
        snapshot: String::new(),
    };
    let json = serde_json::to_string(&env).expect("直列化できること");
    let names = column_names(&section(&doc(), "## 2."), 0);
    assert_eq!(names.len(), 7, "包みの欄は 7 つ");
    assert_eq!(
        names,
        key_order(&json, &names),
        "2 節の欄の並びが直列化の順と一致すること"
    );
}

#[test]
fn doc_version_fields_match_serialization_order() {
    let json = serde_json::to_string(&versions()).expect("直列化できること");
    let sec = section(&doc(), "## 3.");
    let rows = sec
        .lines()
        .filter(|l| l.starts_with('|') && l.contains("| `"))
        .count();
    assert_eq!(rows, 4, "軸の行はちょうど 4 行");
    let names = column_names(&sec, 1);
    assert_eq!(names.len(), 6, "版の欄は 6 つ");
    assert_eq!(
        names,
        key_order(&json, &names),
        "3 節の欄の並びが直列化の順と一致すること"
    );
}
