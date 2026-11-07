//! 製品の文書のビューの経路ごとの行（`doc_element_item`。#2053・SECOND-6）の契約試験。
//!
//! 行の形（題・状態・補足の 3 つの `span` が値の空でも出ること、本文の区画の位置）、印・宛先で子の形が変わらないこと、
//! 属性の値が文字ノードにだけ入りエスケープされること、a11y の閉じた一覧を確かめる。期待値は手で数えた値で、実装から導かない。
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;

use fandhe_frontend_core::render;
use fandhe_workshop_contract::doc::{Block, Doc, Inline};
use fandhe_workshop_core::world::Element;
use fandhe_workshop_web::a11y::{check_no_undefined, ChangeMark};
use fandhe_workshop_web::artifact_view::{PointTarget, RowId};
use fandhe_workshop_web::document_view::{doc_element_item, doc_list};

const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css"));

fn element(title: &str, state: &str, detail: &str) -> Element {
    let mut attrs = BTreeMap::new();
    attrs.insert("title".to_owned(), title.to_owned());
    attrs.insert("state".to_owned(), state.to_owned());
    attrs.insert("detail".to_owned(), detail.to_owned());
    Element {
        fixture: None,
        attrs,
    }
}

fn target() -> PointTarget {
    PointTarget::new("v", RowId::new("a/b").unwrap()).unwrap()
}

fn doc() -> Doc {
    Doc {
        version: fandhe_workshop_contract::doc::TREE_VERSION,
        blocks: vec![Block::Paragraph {
            children: vec![Inline::Text {
                text: "本文".to_owned(),
            }],
        }],
    }
}

fn testids(html: &str) -> Vec<String> {
    html.split("data-testid=\"")
        .skip(1)
        .map(|s| s.split('"').next().unwrap().to_owned())
        .collect()
}

#[test]
fn 属性の無い要素でも_3_つの_span_が空で出る() {
    let html = render(&doc_element_item("a/b", None, None, None, None));
    assert!(html.contains("data-path=\"a/b\""), "{html}");
    assert!(html.contains("data-kind=\"item\""), "{html}");
    assert!(html.contains("data-testid=\"doc-row-title\""), "{html}");
    assert!(html.contains(">a/b</span>"), "{html}");
    assert!(html.contains("class=\"dr-state\"></span>"), "{html}");
    assert!(html.contains("class=\"dr-detail\"></span>"), "{html}");
    assert_eq!(html.matches("artifact-doc-body").count(), 0, "{html}");
}

#[test]
fn 属性の値がそのまま題_状態_補足になり_本文は後ろに続く() {
    let e = element("下書き", "open", "最初の版");
    let d = doc();
    let html = render(&doc_element_item("a/b", Some(&e), None, None, Some(&d)));
    let ids = testids(&html);
    let pos = |id: &str| ids.iter().position(|x| x == id).unwrap();
    assert!(pos("doc-row-title") < pos("doc-row-state"));
    assert!(pos("doc-row-state") < pos("doc-row-detail"));
    assert!(pos("doc-row-detail") < pos("artifact-doc-body"));
    for v in ["下書き", "open", "最初の版"] {
        assert_eq!(html.matches(v).count(), 1, "{html}");
    }
}

#[test]
fn 印と宛先で子の_testid_の並びが変わらない() {
    let e = element("t", "s", "d");
    let t = target();
    let marks = [
        None,
        Some(ChangeMark::Add),
        Some(ChangeMark::Change),
        Some(ChangeMark::Remove),
    ];
    let mut counts = Vec::new();
    for mark in marks {
        for pointed in [false, true] {
            let target = pointed.then_some(&t);
            let html = render(&doc_element_item("a/b", Some(&e), mark, target, None));
            assert_eq!(html.contains("data-row="), pointed, "{html}");
            assert_eq!(html.contains("data-action="), pointed, "{html}");
            assert_eq!(html.contains("data-payload="), pointed, "{html}");
            assert_eq!(html.contains("tabindex="), pointed, "{html}");
            let ids = testids(&html);
            let tail: Vec<String> = ids[ids.len() - 3..].to_vec();
            assert_eq!(tail, ["doc-row-title", "doc-row-state", "doc-row-detail"]);
            counts.push(ids.len());
        }
    }
    assert!(counts.iter().all(|n| *n == counts[0]), "{counts:?}");
}

#[test]
fn 属性の値の_html_の形は文字として出る() {
    let evil = [
        "<script>alert(1)</script>",
        "\"><img src=x onerror=alert(1)>",
        "&amp;",
        "javascript:alert(1)",
    ];
    for v in evil {
        let e = element(v, v, v);
        let html = render(&doc_element_item("a/b", Some(&e), None, None, None));
        assert_eq!(html.matches("<script").count(), 0, "{html}");
        assert_eq!(html.matches("<img").count(), 0, "{html}");
        let ids = testids(&html);
        assert_eq!(ids.len(), 5, "{html}");
        assert!(
            ids.iter()
                .all(|i| i.starts_with("doc-row-") || i.starts_with("mark-")),
            "{ids:?}"
        );
    }
    let e = element("<script>x</script>", "", "");
    let html = render(&doc_element_item("a/b", Some(&e), None, None, None));
    assert!(html.contains("&lt;script&gt;"), "{html}");
}

#[test]
fn a11y_の閉じた一覧に収まり_role_と_aria_が無い() {
    let e = element("t", "s", "d");
    let d = doc();
    let node = doc_element_item(
        "a/b",
        Some(&e),
        Some(ChangeMark::Add),
        Some(&target()),
        Some(&d),
    );
    let list = doc_list("ul", true, vec![("pa/b".to_owned(), node)]).unwrap();
    if let Err(v) = check_no_undefined(&list) {
        panic!("a11y の違反: {v:?}");
    }
    let html = render(&list);
    assert_eq!(html.matches("role=").count(), 0, "{html}");
    assert_eq!(html.matches("aria-").count(), 0, "{html}");
}

#[test]
fn app_css_に行の_span_のクラスがある() {
    assert!(APP_CSS.contains(".dr-state"));
    assert!(APP_CSS.contains(".dr-detail"));
}

#[test]
fn app_css_の文書の節が行を折り返し_補足を列として残す() {
    let start = APP_CSS
        .find("左のビュー: 文書（document）")
        .expect("開始の印");
    let end = APP_CSS.find("ここまで document").expect("終了の印");
    let section = &APP_CSS[start..end];
    assert!(
        section.contains("[data-view=\"document\"] > li[data-kind=\"item\"] { flex-wrap: wrap; }"),
        "{section}"
    );
    assert!(
        section.contains(".dr-title, .dr-state { max-width: 100%; overflow-wrap: anywhere; }"),
        "{section}"
    );
    assert!(
        section.contains(".dr-detail { flex: 1 1 8rem; min-width: 0; overflow-wrap: anywhere; }"),
        "{section}"
    );
}

#[test]
fn app_css_の文書の節が本文の区画を行の幅に収め_pre_を区画の中でスクロールさせる() {
    let start = APP_CSS
        .find("左のビュー: 文書（document）")
        .expect("開始の印");
    let end = APP_CSS.find("ここまで document").expect("終了の印");
    let section = &APP_CSS[start..end];
    assert!(
        section.contains(".artifact-doc-body { min-width: 0; }\n"),
        "{section}"
    );
    assert!(
        section.contains(".artifact-doc-body pre { overflow-x: auto; }\n"),
        "{section}"
    );
}
