//! 案内レベル別の本文の選択と濃い側への倒れ（TASK-292、GUIDE-1）の試験。
//!
//! 3 水準 × 版の有無の 12 通り、本文の中身が水準の定義に合うこと（目印の語。試験データ
//! `tests/data/body_level_1/ir.json`）、鍵 `A`・未知の鍵・木なしの fail-closed、任意の鍵の組での全域性を確かめる。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::doc::{Block, Doc, Inline};
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::body_level::{body_doc_at, select_body};
use fandhe_workshop_core::load_ir;
use proptest::prelude::*;
use serde_json::{json, Value};
use GuidanceLevel::{A, B, C};

const SKILL: &str = "/sample-skill";
const PLACE: &str = "作業場所: 実装リポジトリ";

fn doc_json(t: &str) -> Value {
    json!({"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":t}]}]})
}

fn node(a: Option<&str>, levels: &[(&str, &str)]) -> Node {
    let mut v = json!({"uid":"main/s","scope":"main","unit":"u","step":{"id":"s","title":"t"}});
    if let Some(a) = a {
        v["body_doc"] = doc_json(a);
    }
    let m: serde_json::Map<String, Value> = levels
        .iter()
        .map(|(k, t)| ((*k).to_string(), doc_json(t)))
        .collect();
    if !m.is_empty() {
        v["body_levels_doc"] = m.into();
    }
    serde_json::from_value(v).unwrap()
}

fn inline_text(i: &Inline, out: &mut String) {
    match i {
        Inline::Text { text } | Inline::Code { text } => out.push_str(text),
        Inline::Emph { children } | Inline::Strong { children } | Inline::Link { children, .. } => {
            children.iter().for_each(|c| inline_text(c, out));
        }
        _ => {}
    }
}

fn block_text(b: &Block, out: &mut String) {
    match b {
        Block::Heading { children, .. } | Block::Paragraph { children } => {
            children.iter().for_each(|c| inline_text(c, out));
        }
        Block::Quote { children } => children.iter().for_each(|c| block_text(c, out)),
        Block::List { items, .. } => items.iter().flatten().for_each(|c| block_text(c, out)),
        Block::Code { text, .. } => out.push_str(text),
        _ => {}
    }
}

fn plain(d: &Doc) -> String {
    let mut s = String::new();
    d.blocks.iter().for_each(|b| block_text(b, &mut s));
    s
}

fn picked(n: &Node, l: GuidanceLevel) -> Option<(GuidanceLevel, String)> {
    select_body(n, l).map(|s| (s.level, plain(s.doc)))
}

#[test]
fn twelve_combinations_pick_the_expected_body_and_fall_toward_the_dense_side() {
    type Levels<'a> = &'a [(&'a str, &'a str)];
    let cases: [(Levels, [GuidanceLevel; 3]); 4] = [
        (&[], [A, A, A]),
        (&[("B", "b")], [A, B, B]),
        (&[("C", "c")], [A, A, C]),
        (&[("B", "b"), ("C", "c")], [A, B, C]),
    ];
    for (levels, want) in cases {
        let n = node(Some("a"), levels);
        for (l, w) in GuidanceLevel::ALL.into_iter().zip(want) {
            let text = match w {
                A => "a",
                B => "b",
                C => "c",
            };
            assert_eq!(picked(&n, l), Some((w, text.to_string())), "{levels:?} {l}");
            assert_eq!(body_doc_at(&n, l), select_body(&n, l).map(|s| s.doc));
        }
    }
}

fn fixture_ir() -> Ir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/body_level_1/ir.json");
    load_ir(&std::fs::read(p).unwrap()).unwrap()
}

fn by_step<'a>(ir: &'a Ir, id: &str) -> &'a Node {
    ir.nodes.iter().find(|n| n.step.id == id).unwrap()
}

#[test]
fn selected_body_matches_the_level_definition() {
    let ir = fixture_ir();
    let full = by_step(&ir, "full");
    let a = picked(full, A).unwrap().1;
    let b = picked(full, B).unwrap().1;
    let c = picked(full, C).unwrap().1;
    assert!(a.contains(SKILL), "{a}");
    assert!(b.contains(PLACE) && !b.contains(SKILL), "{b}");
    assert!(!c.contains(PLACE) && !c.contains(SKILL), "{c}");
}

#[test]
fn missing_versions_fall_to_the_dense_side_in_the_fixture() {
    let ir = fixture_ir();
    // 倒れた結果は濃い側の定義に合う（薄い側の定義は満たさない）。
    let no_b = by_step(&ir, "no-b");
    let (lv, text) = picked(no_b, B).unwrap();
    assert_eq!(lv, A);
    assert!(text.contains(SKILL));
    assert_eq!(picked(no_b, C).unwrap().0, C);
    let no_c = by_step(&ir, "no-c");
    let (lv, text) = picked(no_c, C).unwrap();
    assert_eq!(lv, B);
    assert!(text.contains(PLACE) && !text.contains(SKILL));
    let only_a = by_step(&ir, "only-a");
    for l in GuidanceLevel::ALL {
        assert_eq!(picked(only_a, l).unwrap().0, A);
    }
}

#[test]
fn fail_closed_on_odd_irs() {
    // 鍵 A だけで body_doc なし: 全水準で何も出さない。
    let n = node(None, &[("A", "x")]);
    for l in GuidanceLevel::ALL {
        assert_eq!(picked(&n, l), None);
    }
    // 鍵 A と body_doc の両方: A は body_doc。
    let n = node(Some("a"), &[("A", "x")]);
    assert_eq!(picked(&n, A), Some((A, "a".to_string())));
    // 未知の鍵は読まれない。
    let n = node(Some("a"), &[("D", "x"), ("b", "y")]);
    for l in GuidanceLevel::ALL {
        assert_eq!(picked(&n, l), Some((A, "a".to_string())));
    }
    // body_doc なしで C 版だけ。
    let n = node(None, &[("C", "c")]);
    assert_eq!(picked(&n, A), None);
    assert_eq!(picked(&n, B), None);
    assert_eq!(picked(&n, C), Some((C, "c".to_string())));
    // 木を持たない版 1 の Node（本文のパスだけ）は何も出さない。
    let n: Node = serde_json::from_value(json!({
        "uid":"main/s","scope":"main","unit":"u",
        "step":{"id":"s","title":"t","body":"a.md","body_levels":{"B":"a.b.md"}}
    }))
    .unwrap();
    for l in GuidanceLevel::ALL {
        assert_eq!(picked(&n, l), None);
    }
}

#[test]
fn total_over_arbitrary_key_sets() {
    let keys = prop::sample::subsequence(vec!["A", "B", "C", "D", "b", ""], 0..=6);
    check(
        "body-level-total",
        DEFAULT_CASES,
        (keys, any::<bool>()),
        |(keys, has_a)| {
            let levels: BTreeMap<&str, String> =
                keys.iter().map(|k| (*k, format!("lv-{k}"))).collect();
            let pairs: Vec<(&str, &str)> = levels.iter().map(|(k, v)| (*k, v.as_str())).collect();
            let n = node(has_a.then_some("a"), &pairs);
            for l in GuidanceLevel::ALL {
                if let Some(s) = select_body(&n, l) {
                    prop_assert!(l.fallback_chain().contains(&s.level));
                    prop_assert!(!plain(s.doc).starts_with("lv-A"));
                    if s.level == A {
                        prop_assert_eq!(plain(s.doc), "a");
                    }
                }
            }
            Ok(())
        },
    );
}
