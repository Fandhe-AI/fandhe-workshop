//! 語彙の文書の語の一覧と、`vocab` モジュールの表の一致の検査（TASK-194、PACK-8〜PACK-23）。
//!
//! 正は `docs/vocabulary/vocabulary.md`（7 節の語の一覧、2 節の拡張点、8 節の対応表、9 節の照合表）で、
//! `fandhe_workshop_pack::vocab` の表は実装側の写しである。この試験は、両者が食い違えば失敗する。
//! 文書を読むだけで、作業ツリーへは書き込まない。

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use fandhe_workshop_pack::vocab::{VocabVersion, EXTENSION_POINTS, LATEST, TERMS};

const VERSIONS: [&str; 6] = ["v1", "v1.1", "v1.2", "v1.3", "v1.4", "v1.5"];

fn doc() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/vocabulary/vocabulary.md");
    fs::read_to_string(&path).expect("語彙の文書を読めること")
}

fn cells(line: &str) -> Vec<String> {
    line.trim()
        .trim_matches('|')
        .split('|')
        .map(|c| c.trim().to_string())
        .collect()
}

fn strip(c: &str) -> String {
    c.trim_matches('`').trim().to_string()
}

/// 見出しが `heading` で始まる節の表の本体行（ヘッダと区切りの行を除く）を、セルに分けて返す。
fn section_rows(doc: &str, heading: &str) -> Vec<Vec<String>> {
    let mut inside = false;
    let mut rows = Vec::new();
    let mut header_seen = false;
    for line in doc.lines() {
        if line.starts_with("## ") {
            inside = line.starts_with(heading);
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
        rows.push(cells(line));
    }
    rows
}

/// 7 節の行（種別、語、語彙の版、拡張点）。
fn doc_terms(doc: &str) -> Vec<(String, String, String, String)> {
    section_rows(doc, "## 7. ")
        .iter()
        .map(|r| {
            assert_eq!(r.len(), 6, "7 節の行は 6 列: {r:?}");
            (strip(&r[0]), strip(&r[1]), strip(&r[2]), r[3].clone())
        })
        .collect()
}

fn table_terms() -> Vec<(String, String, String, String)> {
    TERMS
        .iter()
        .map(|t| {
            (
                t.kind.to_string(),
                t.term.to_string(),
                t.version.as_str().to_string(),
                t.extension_point.unwrap_or("—").to_string(),
            )
        })
        .collect()
}

fn ep_rows(doc: &str) -> Vec<Vec<String>> {
    doc.lines()
        .filter(|l| l.starts_with("| EP-"))
        .map(|l| cells(l).iter().map(|c| strip(c)).collect())
        .collect()
}

#[test]
fn terms_match_implementation_table() {
    let doc_set: BTreeSet<_> = doc_terms(&doc()).into_iter().collect();
    let table_set: BTreeSet<_> = table_terms().into_iter().collect();
    let only_doc: Vec<_> = doc_set.difference(&table_set).collect();
    let only_table: Vec<_> = table_set.difference(&doc_set).collect();
    assert!(only_doc.is_empty(), "文書にだけある語: {only_doc:?}");
    assert!(only_table.is_empty(), "表にだけある語: {only_table:?}");
}

#[test]
fn every_term_has_known_version() {
    let terms = doc_terms(&doc());
    assert!(!terms.is_empty());
    for (kind, term, ver, _) in terms {
        assert!(
            VERSIONS.contains(&ver.as_str()),
            "{kind} {term}: 語彙の版が不明: {ver:?}"
        );
    }
}

#[test]
fn terms_are_unique() {
    for (label, terms) in [("文書", doc_terms(&doc())), ("表", table_terms())] {
        let mut seen = BTreeSet::new();
        for (kind, term, _, _) in terms {
            assert!(
                seen.insert((kind.clone(), term.clone())),
                "{label}: 重複 {kind} {term}"
            );
        }
    }
}

#[test]
fn extension_points_match_table() {
    let rows = ep_rows(&doc());
    assert_eq!(rows.len(), 13, "EP の行の数");
    assert_eq!(rows.len(), EXTENSION_POINTS.len());
    for (row, ep) in rows.iter().zip(EXTENSION_POINTS) {
        assert_eq!(
            (
                row[0].as_str(),
                row[1].as_str(),
                row[2].as_str(),
                row[3].as_str()
            ),
            (ep.id, ep.name, ep.kind, ep.version.as_str())
        );
    }
}

#[test]
fn version_table_matches_extension_points() {
    let d = doc();
    let ep_version: BTreeMap<String, String> = ep_rows(&d)
        .into_iter()
        .map(|r| (r[0].clone(), r[3].clone()))
        .collect();
    let mut seen: BTreeMap<String, String> = BTreeMap::new();
    for row in section_rows(&d, "## 8. ") {
        let ver = strip(&row[0]);
        for ep in row[1].split('・') {
            let ep = ep.trim().to_string();
            // 拡張点を持たない版（v1.5）の行は `—`。
            if ep == "—" {
                continue;
            }
            assert!(
                seen.insert(ep.clone(), ver.clone()).is_none(),
                "{ep} が複数の版の行に現れる"
            );
        }
    }
    assert_eq!(seen, ep_version, "8 節の対応と 2 節の語彙の版が一致");
    assert_eq!(seen.len(), 13);
}

#[test]
fn label_is_v13_without_number() {
    let d = doc();
    let label = "confirm.options[].label";
    let row = doc_terms(&d)
        .into_iter()
        .find(|(_, t, _, _)| t == label)
        .expect("label が 7 節にあること");
    assert_eq!((row.2.as_str(), row.3.as_str()), ("v1.3", "—"));
    let v13 = section_rows(&d, "## 8. ")
        .into_iter()
        .find(|r| strip(&r[0]) == "v1.3")
        .expect("v1.3 の行");
    assert!(v13[2].contains(label), "8 節の v1.3 の番号なしの語に label");
    assert!(
        ep_rows(&d).iter().all(|r| r[1] != label),
        "label は拡張点ではない"
    );
}

#[test]
fn revise_candidates_are_v15_without_number() {
    let d = doc();
    let terms = doc_terms(&d);
    let v15 = section_rows(&d, "## 8. ")
        .into_iter()
        .find(|r| strip(&r[0]) == "v1.5")
        .expect("v1.5 の行");
    assert_eq!(strip(&v15[1]), "—", "v1.5 は拡張点を持たない");
    for term in ["input.revise.candidates", "input.revise.candidates[].label"] {
        let row = terms
            .iter()
            .find(|(_, t, _, _)| t == term)
            .unwrap_or_else(|| panic!("{term} が 7 節にあること"));
        assert_eq!((row.2.as_str(), row.3.as_str()), ("v1.5", "—"));
        assert!(v15[2].contains(term), "8 節の v1.5 の番号なしの語に {term}");
        assert!(
            ep_rows(&d).iter().all(|r| r[1] != term),
            "{term} は拡張点ではない"
        );
    }
}

#[test]
fn every_extension_point_appears_in_terms() {
    let d = doc();
    let used: BTreeSet<String> = doc_terms(&d).into_iter().map(|t| t.3).collect();
    for row in ep_rows(&d) {
        assert!(
            used.contains(&row[0]),
            "{} が 7 節の拡張点の列に無い",
            row[0]
        );
    }
}

#[test]
fn crosswalk_terms_exist() {
    let d = doc();
    let known: BTreeSet<String> = doc_terms(&d).into_iter().map(|t| t.1).collect();
    let rows = section_rows(&d, "## 9. ");
    let ids: Vec<String> = rows.iter().map(|r| r[0].clone()).collect();
    let mut want: Vec<String> = (8..=23).map(|n| format!("PACK-{n}")).collect();
    // 語彙 v1.4（TASK-66 1/2）の出所。
    want.push("PACK-38".to_string());
    // 語彙 v1.5（TASK-62 2/2）の出所。
    want.push("PACK-42".to_string());
    assert_eq!(
        ids, want,
        "PACK-8〜PACK-23・PACK-38・PACK-42 の 18 行がそろう"
    );
    for row in &rows {
        for term in row[1].split('`').skip(1).step_by(2) {
            assert!(known.contains(term), "{}: 7 節に無い語 {term}", row[0]);
        }
    }
}

#[test]
fn latest_is_v15() {
    assert_eq!(LATEST, VocabVersion::V1_5);
    let max = TERMS.iter().map(|t| t.version).max().expect("語がある");
    assert_eq!(max, LATEST);
}

#[test]
fn core_vocab_max_matches_latest() {
    // エンジンが読める語彙の上限の正（core の `VOCAB_MAX`）と、文書の最新版（`LATEST`）がずれたら失敗する。
    assert_eq!(
        format!("v{}", fandhe_workshop_core::history_open::VOCAB_MAX),
        LATEST.as_str()
    );
}
