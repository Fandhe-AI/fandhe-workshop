//! 文書ビューの本文の宣言（`views.<ID>.body`）の閉じた集合・読み取り・本文の解決の契約試験（TASK-66 1/2、PACK-38）。

use fandhe_workshop_contract::doc::{Block, Doc, Inline, TREE_VERSION};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::view_body::{body_doc, read_view_body, ViewBody, ViewBodyReadError};
use serde_json::json;

fn doc(text: &str) -> Doc {
    Doc {
        version: TREE_VERSION,
        blocks: vec![Block::Paragraph {
            children: vec![Inline::Text { text: text.into() }],
        }],
    }
}

fn ir(views: serde_json::Value) -> Ir {
    let src = include_str!("data/ir_format_1/minimal.json");
    let mut ir: Ir = serde_json::from_str(src).unwrap();
    ir.views = serde_json::from_value(views).unwrap();
    ir.fixture_docs.insert("doc-a".into(), doc("a"));
    ir
}

#[test]
fn closed_set_round_trips() {
    for v in ViewBody::ALL {
        assert_eq!(ViewBody::from_vocab(v.as_str()), Some(v));
    }
    assert_eq!(ViewBody::ALL.len(), 2);
    assert_eq!(ViewBody::default(), ViewBody::Hidden);
}

#[test]
fn absent_key_and_non_mapping_are_hidden() {
    assert_eq!(
        read_view_body(&json!({"kind": "document"})),
        Ok(ViewBody::Hidden)
    );
    assert_eq!(read_view_body(&json!("document")), Ok(ViewBody::Hidden));
    assert_eq!(read_view_body(&json!(null)), Ok(ViewBody::Hidden));
}

#[test]
fn reads_both_values() {
    assert_eq!(
        read_view_body(&json!({"body": "shown"})),
        Ok(ViewBody::Shown)
    );
    assert_eq!(
        read_view_body(&json!({"body": "hidden"})),
        Ok(ViewBody::Hidden)
    );
}

#[test]
fn wrong_type_is_not_a_string() {
    for bad in [
        json!(null),
        json!(1),
        json!(true),
        json!(["shown"]),
        json!({}),
    ] {
        assert_eq!(
            read_view_body(&json!({"body": bad})),
            Err(ViewBodyReadError::NotAString)
        );
    }
}

#[test]
fn unknown_spelling_is_refused_exactly() {
    for bad in ["Shown", "SHOWN", " shown", "shown ", "visible", ""] {
        assert_eq!(
            read_view_body(&json!({"body": bad})),
            Err(ViewBodyReadError::Unknown),
            "{bad:?}"
        );
    }
}

#[test]
fn body_doc_returns_tree_only_when_shown_and_known() {
    let ir = ir(json!({"v": {"kind": "document", "body": "shown"}}));
    assert_eq!(body_doc(&ir, "v", Some("doc-a")), Some(&doc("a")));
    assert_eq!(body_doc(&ir, "v", None), None);
    assert_eq!(body_doc(&ir, "v", Some("missing")), None);
    assert_eq!(body_doc(&ir, "undefined", Some("doc-a")), None);
}

#[test]
fn body_doc_is_none_when_hidden_or_unreadable() {
    let ir = ir(json!({
        "h": {"kind": "document", "body": "hidden"},
        "d": {"kind": "document"},
        "bad": {"kind": "document", "body": "visible"}
    }));
    for id in ["h", "d", "bad"] {
        assert_eq!(body_doc(&ir, id, Some("doc-a")), None, "{id}");
    }
}
