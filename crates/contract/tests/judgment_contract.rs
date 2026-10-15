//! 入力の判定結果・拒否の分類・次の行動・字句化の型の適合試験（TASK-256、版 1）。
//!
//! JUDGE-3（結果の種類が 1 つに定まる）・JUDGE-4（空でない次の行動）・PACK-11（分類の語）と、
//! golden（`tests/data/judgment_1/`）とのバイト一致、語彙の文書との語の一致を固定する。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

use std::collections::BTreeSet;
use std::path::Path;

use fandhe_workshop_contract::judgment::*;
use serde_json::{json, Value};

fn golden(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/judgment_1")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|_| panic!("cannot read golden {name}"))
}

fn render<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut b = serde_json::to_vec_pretty(v).unwrap();
    b.push(b'\n');
    b
}

fn na(s: &str) -> NextAction {
    NextAction::new(s).unwrap()
}

fn sample_judgments() -> Vec<Judgment> {
    vec![
        Judgment::Accepted(None),
        Judgment::Accepted(Some(AcceptNote {
            text: "free-text-note".into(),
            next_action: na("次は ls を実行してください"),
        })),
        Judgment::Rejected(Rejection {
            category: RejectCategory::UnknownCommand,
            text: "unknown-command".into(),
            next_action: na("help と入力すると一覧が出ます"),
        }),
        Judgment::Clarified(Clarification {
            text: "which-file".into(),
            next_action: na("対象のファイル名を指定してください"),
        }),
        Judgment::Confirmed,
        Judgment::Progressed,
        Judgment::Ignored,
    ]
}

fn pos(s: &str) -> LexedArg {
    LexedArg::Positional(s.into())
}

fn named(k: &str, v: Option<&str>) -> LexedArg {
    LexedArg::Named {
        key: k.into(),
        value: v.map(Into::into),
    }
}

fn sample_lexed() -> Vec<LexedCommand> {
    vec![
        LexedCommand {
            name: "ls".into(),
            args: vec![],
        },
        LexedCommand {
            name: "cat".into(),
            args: vec![pos("a.txt"), pos("b.txt")],
        },
        LexedCommand {
            name: "build".into(),
            args: vec![
                named("out", Some("dist")),
                named("verbose", None),
                named("out", Some("other")),
                pos("x"),
            ],
        },
    ]
}

#[test]
fn contract_version_is_1() {
    assert_eq!(JUDGMENT_CONTRACT_VERSION, 1);
}

// --- JUDGE-3 ---

#[test]
fn judgment_kind_has_six_distinct_values() {
    let names: BTreeSet<_> = JudgmentKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(JudgmentKind::ALL.len(), 6);
    assert_eq!(names.len(), 6);
    for k in JudgmentKind::ALL {
        assert_eq!(serde_json::to_value(k).unwrap(), json!(k.as_str()));
    }
}

#[test]
fn every_golden_judgment_maps_to_exactly_one_kind() {
    let list: Vec<Judgment> = serde_json::from_slice(&golden("judgments.json")).unwrap();
    let kinds: BTreeSet<_> = list.iter().map(|j| j.kind().as_str()).collect();
    let all: BTreeSet<_> = JudgmentKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(kinds, all);
    for j in &list {
        let expected = match j {
            Judgment::Clarified(_) => JudgmentKind::Clarified,
            Judgment::Rejected(_) => JudgmentKind::Rejected,
            _ => j.kind(),
        };
        assert_eq!(j.kind(), expected);
    }
    let clarified = list
        .iter()
        .find(|j| matches!(j, Judgment::Clarified(_)))
        .unwrap();
    assert_ne!(clarified.kind(), JudgmentKind::Rejected);
}

#[test]
fn clarified_is_not_a_category() {
    assert!(Category::from_str_opt("clarified").is_none());
    assert!(serde_json::from_value::<Category>(json!("clarified")).is_err());
}

// --- 分類 ---

#[test]
fn category_sets_are_consistent() {
    assert_eq!(Category::ALL.len(), 8);
    assert_eq!(RejectCategory::ALL.len(), 7);
    let rejects: BTreeSet<_> = RejectCategory::ALL.iter().map(|c| c.as_str()).collect();
    let all_but_free: BTreeSet<_> = Category::ALL
        .iter()
        .map(|c| c.as_str())
        .filter(|s| *s != "free_text_unjudged")
        .collect();
    assert_eq!(rejects, all_but_free);
}

#[test]
fn category_strings_round_trip() {
    for c in Category::ALL {
        assert_eq!(Category::from_str_opt(c.as_str()), Some(c));
        assert_eq!(serde_json::to_value(c).unwrap(), json!(c.as_str()));
        let back: Category = serde_json::from_value(json!(c.as_str())).unwrap();
        assert_eq!(back, c);
    }
    for c in RejectCategory::ALL {
        let back: RejectCategory = serde_json::from_value(json!(c.as_str())).unwrap();
        assert_eq!(back, c);
        assert_eq!(Category::from(c).as_str(), c.as_str());
    }
}

#[test]
fn only_free_text_unjudged_is_not_a_rejection() {
    for c in Category::ALL {
        let is_free = c == Category::FreeTextUnjudged;
        assert_eq!(c.is_rejection(), !is_free);
        assert_eq!(RejectCategory::try_from(c).is_err(), is_free);
    }
}

#[test]
fn unknown_category_strings_fail() {
    for s in ["clarified", "Unknown_Command", "", " busy"] {
        assert!(
            serde_json::from_value::<Category>(json!(s)).is_err(),
            "{s:?}"
        );
        assert!(
            serde_json::from_value::<RejectCategory>(json!(s)).is_err(),
            "{s:?}"
        );
    }
}

#[test]
fn categories_golden_matches() {
    assert_eq!(render(&Category::ALL), golden("categories.json"));
}

// --- JUDGE-4 ---

#[test]
fn next_action_new_rejects_empty_and_whitespace() {
    for s in ["", " ", "\u{3000}", "\n\t"] {
        assert_eq!(NextAction::new(s), Err(EmptyNextAction), "{s:?}");
    }
    assert_eq!(NextAction::new("a").unwrap().as_str(), "a");
    let padded = NextAction::new("  a b \n").unwrap();
    assert_eq!(padded.as_str(), "  a b \n");
    assert_eq!(padded.into_inner(), "  a b \n");
}

#[test]
fn deserialization_rejects_empty_next_action() {
    for bad in [json!(""), json!("  "), json!("\u{3000}"), json!("\n\t")] {
        let rejected = json!({"rejected": {"category": "busy", "text": "t", "next_action": bad}});
        let clarified = json!({"clarified": {"text": "t", "next_action": bad}});
        let note = json!({"accepted": {"text": "t", "next_action": bad}});
        for v in [rejected, clarified, note] {
            assert!(
                serde_json::from_value::<Judgment>(v.clone()).is_err(),
                "{v}"
            );
        }
    }
}

#[test]
fn missing_next_action_fails() {
    let rejected = json!({"rejected": {"category": "busy", "text": "t"}});
    let clarified = json!({"clarified": {"text": "t"}});
    let note = json!({"accepted": {"text": "t"}});
    for v in [rejected, clarified, note] {
        assert!(
            serde_json::from_value::<Judgment>(v.clone()).is_err(),
            "{v}"
        );
    }
}

#[test]
fn rejection_with_free_text_unjudged_fails() {
    let v =
        json!({"rejected": {"category": "free_text_unjudged", "text": "t", "next_action": "go"}});
    assert!(serde_json::from_value::<Judgment>(v).is_err());
    assert_eq!(AcceptNote::CATEGORY, Category::FreeTextUnjudged);
}

#[test]
fn error_displays_do_not_embed_input() {
    assert_eq!(EmptyNextAction.to_string(), "next_action must not be empty");
    assert!(NotARejection.to_string().is_ascii());
    for e in [
        LexError::EmptyName,
        LexError::NameHasWhitespace,
        LexError::EmptyPositional,
        LexError::PositionalHasWhitespace,
        LexError::EmptyKey,
        LexError::KeyHasWhitespace,
        LexError::KeyStartsWithDash,
        LexError::EmptyValue,
        LexError::ValueHasWhitespace,
    ] {
        assert!(e.to_string().is_ascii());
    }
}

// --- 直列化の形 ---

#[test]
fn judgments_golden_matches_both_ways() {
    let bytes = golden("judgments.json");
    let parsed: Vec<Judgment> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed, sample_judgments());
    assert_eq!(render(&parsed), bytes);
    assert_eq!(render(&sample_judgments()), bytes);
}

#[test]
fn unknown_fields_and_variants_fail() {
    let cases = [
        json!({"rejected": {"category": "busy", "text": "t", "next_action": "go", "x": 1}}),
        json!({"clarified": {"text": "t", "next_action": "go", "x": 1}}),
        json!({"accepted": {"text": "t", "next_action": "go", "x": 1}}),
        json!("clarify"),
        json!({"rejected": {"category": "busy", "text": "t", "next_action": "go"}, "extra": 1}),
    ];
    for v in cases {
        assert!(
            serde_json::from_value::<Judgment>(v.clone()).is_err(),
            "{v}"
        );
    }
}

// --- 字句化 ---

#[test]
fn lexed_golden_round_trips_and_validates() {
    let bytes = golden("lexed.json");
    let parsed: Vec<LexedCommand> = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed, sample_lexed());
    assert_eq!(render(&parsed), bytes);
    for c in &parsed {
        assert_eq!(c.validate(), Ok(()));
    }
}

#[test]
fn lexed_keeps_order_and_duplicates() {
    let c = &sample_lexed()[2];
    assert_eq!(c.args.len(), 4);
    assert_eq!(c.args[0], named("out", Some("dist")));
    assert_eq!(c.args[2], named("out", Some("other")));
    assert_ne!(named("f", None), named("f", Some("x")));
}

#[test]
fn lexed_validation_reports_each_kind() {
    let cmd = |name: &str, args: Vec<LexedArg>| LexedCommand {
        name: name.into(),
        args,
    };
    let cases = [
        (cmd("", vec![]), LexError::EmptyName),
        (cmd("a b", vec![]), LexError::NameHasWhitespace),
        (cmd("a", vec![pos("")]), LexError::EmptyPositional),
        (
            cmd("a", vec![pos("x\u{3000}y")]),
            LexError::PositionalHasWhitespace,
        ),
        (cmd("a", vec![named("", None)]), LexError::EmptyKey),
        (
            cmd("a", vec![named("k k", None)]),
            LexError::KeyHasWhitespace,
        ),
        (
            cmd("a", vec![named("-k", None)]),
            LexError::KeyStartsWithDash,
        ),
        (cmd("a", vec![named("k", Some(""))]), LexError::EmptyValue),
        (
            cmd("a", vec![named("k", Some("v w"))]),
            LexError::ValueHasWhitespace,
        ),
    ];
    for (c, e) in cases {
        assert_eq!(c.validate(), Err(e));
    }
}

#[test]
fn json_value_shape_of_unit_variants_is_a_string() {
    let v: Value = serde_json::to_value(Judgment::Confirmed).unwrap();
    assert_eq!(v, json!("confirmed"));
}
