//! 変化の印の部品（`artifact_mark`）と外部スタイルシートの契約試験（TASK-469 1/2。ART-3・A11Y-22・SAFE-13・SAFE-15）。
//!
//! 確かめること: 印が 3 手段で対応し印なしは 3 つとも空であること、印の宿主が 1 項目にちょうど 1 つで変種ごとの
//! 付け先が正しいこと、深さのクラスが閉じた表に丸まること、配信する HTML・CSS が Rust の定数と食い違わず
//! `<style` 要素も `style` 属性も持たないこと。値はすべて汎用のダミーの文字列である。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_web::a11y::{
    check_no_undefined, ChangeMark, MarkValue, MARK_ATTR, MARK_GLYPH_TESTID, MARK_TEXT_TESTID,
};
use fandhe_workshop_web::artifact_mark::{
    depth_class, mark_attr_value, marked_item, DEPTH_CLASSES, MARK_GLYPH_CLASS, MARK_TEXT_CLASS,
};
use fandhe_workshop_web::artifact_view::Variant;

const APP_CSS: &str = include_str!("../static/app.css");
const INDEX_HTML: &str = include_str!("../static/index.html");

fn marks() -> Vec<Option<ChangeMark>> {
    let mut v = vec![None];
    v.extend(ChangeMark::ALL.iter().copied().map(Some));
    v
}

fn item(variant: Variant, mark: Option<ChangeMark>, depth: Option<u8>) -> Node {
    marked_item(
        "li",
        variant,
        mark,
        depth,
        vec![("data-path", "a/b\"<c>")],
        vec![el("span", vec![], vec![text("ラベル")])],
    )
}

fn list_of(n: Node) -> Node {
    el("ul", vec![], vec![n])
}

#[test]
fn 全組み合わせで_a11y_の閉じた一覧に違反しない() {
    for variant in Variant::ALL {
        for mark in marks() {
            let view = list_of(item(variant, mark, Some(2)));
            assert_eq!(check_no_undefined(&view), Ok(()), "{variant:?} {mark:?}");
        }
    }
}

#[test]
fn 印の宿主は_1_項目にちょうど_1_つ() {
    for variant in Variant::ALL {
        for mark in marks() {
            let html = render(&item(variant, mark, Some(0)));
            assert_eq!(html.matches("data-mark=").count(), 1, "{variant:?} {html}");
        }
    }
}

#[test]
fn 変種_b_は根が宿主で_a_は外枠が宿主() {
    let b = render(&item(Variant::B, Some(ChangeMark::Add), Some(1)));
    assert!(
        b.starts_with("<li data-path=\"a/b&quot;&lt;c&gt;\" data-mark=\"add\" class=\"d1\">"),
        "{b}"
    );
    let a = render(&item(Variant::A, Some(ChangeMark::Add), Some(1)));
    assert!(
        a.starts_with("<li data-path=\"a/b&quot;&lt;c&gt;\"><div data-mark=\"add\" class=\"d1\">"),
        "{a}"
    );
}

#[test]
fn 記号と文字は宿主の直接の子で_3_手段が対応する() {
    for variant in Variant::ALL {
        for mark in marks() {
            let value = mark.map_or(MarkValue::None, MarkValue::Mark);
            let html = render(&item(variant, mark, None));
            let expect_glyph = format!(
                "<span data-testid=\"{MARK_GLYPH_TESTID}\" class=\"{MARK_GLYPH_CLASS}\">{}</span>",
                value.glyph()
            );
            let expect_text = format!(
                "<span data-testid=\"{MARK_TEXT_TESTID}\" class=\"{MARK_TEXT_CLASS}\">{}</span>",
                value.text()
            );
            assert!(html.contains(&expect_glyph), "{html}");
            assert!(
                html.contains(&format!("{expect_glyph}{expect_text}")),
                "{html}"
            );
            assert!(html.contains(&format!("{MARK_ATTR}=\"{}\"", mark_attr_value(value))));
            assert!(!html.contains("aria-hidden"), "{html}");
        }
    }
}

#[test]
fn 印なしは_3_手段とも空() {
    let value = MarkValue::None;
    assert_eq!(mark_attr_value(value), "");
    assert_eq!(value.glyph(), "");
    assert_eq!(value.text(), "");
    let html = render(&item(Variant::B, None, None));
    assert!(html.contains("data-mark=\"\""), "{html}");
    assert!(
        html.contains(&format!("class=\"{MARK_GLYPH_CLASS}\"></span>")),
        "{html}"
    );
    assert!(
        html.contains(&format!("class=\"{MARK_TEXT_CLASS}\"></span>")),
        "{html}"
    );
}

#[test]
fn 深さのクラスは_d0_から_d6_で_範囲外は_d6_に丸まる() {
    for (d, want) in DEPTH_CLASSES.iter().enumerate() {
        assert_eq!(depth_class(u8::try_from(d).unwrap()), *want);
    }
    assert_eq!(depth_class(0), "d0");
    assert_eq!(depth_class(6), "d6");
    assert_eq!(depth_class(7), "d6");
    assert_eq!(depth_class(255), "d6");
    for d in 0..=u8::MAX {
        assert!(DEPTH_CLASSES.contains(&depth_class(d)), "{d}");
    }
}

#[test]
fn 深さが無ければクラスを付けない() {
    let html = render(&item(Variant::B, None, None));
    assert!(!html.contains("class=\"d"), "{html}");
}

#[test]
fn 描画結果に_style_要素も_style_属性も無い() {
    for variant in Variant::ALL {
        for mark in marks() {
            let html = render(&item(variant, mark, Some(3)));
            assert!(!html.contains("<style"), "{html}");
            assert!(!html.contains(" style="), "{html}");
        }
    }
}

#[test]
fn css_は_rust_の定数のクラスと印の属性を定義している() {
    for class in [MARK_GLYPH_CLASS, MARK_TEXT_CLASS]
        .into_iter()
        .chain(DEPTH_CLASSES)
    {
        assert!(APP_CSS.contains(&format!(".{class} ")), "missing .{class}");
    }
    for m in ChangeMark::ALL {
        let sel = format!("[data-mark=\"{}\"]", m.data_value());
        assert!(APP_CSS.contains(&sel), "missing {sel}");
    }
    assert!(!APP_CSS.contains("@import"));
    assert!(!APP_CSS.contains("url("));
}

#[test]
fn index_html_は外部_css_を読み_style_を持たない() {
    assert!(
        INDEX_HTML.contains("<link rel=\"stylesheet\" href=\"./app.css\">"),
        "{INDEX_HTML}"
    );
    assert!(!INDEX_HTML.contains("<style"));
    assert!(!INDEX_HTML.contains("style="));
    assert!(!INDEX_HTML.contains("unsafe-inline"));
}

/// 木の形だけを残した写し。タグ・属性名の集合・子の数と種別を残し、属性値と文字の中身は捨てる。
/// keyed list は同じ key の項目を、子ノード列の位置対応が一致するときだけ in-place に更新する（上流の
/// `diff_children`）。形が印の値で変わると、前後の切り替えで項目の子が作り直される。
#[derive(Debug, PartialEq, Eq)]
enum Shape {
    Element(&'static str, Vec<String>, Vec<Shape>),
    Text,
}

fn shape(n: &Node) -> Shape {
    match n {
        Node::Element {
            tag,
            attrs,
            children,
        } => {
            let mut names: Vec<String> = attrs.iter().map(|(k, _)| k.clone()).collect();
            names.sort();
            Shape::Element(tag, names, children.iter().map(shape).collect())
        }
        Node::Text(_) | Node::RawHtml(_) => Shape::Text,
    }
}

#[test]
fn 項目の形は印によって変わらない() {
    for variant in Variant::ALL {
        for depth in [None, Some(0), Some(3)] {
            let base = shape(&item(variant, None, depth));
            for mark in ChangeMark::ALL {
                assert_eq!(
                    shape(&item(variant, Some(mark), depth)),
                    base,
                    "{variant:?} {depth:?} {mark:?}"
                );
            }
        }
    }
    // 空でない形であること（空の比較で通らない）。印なしでも記号・文字の 2 つの span が子にある。
    let Shape::Element(_, _, kids) = shape(&item(Variant::B, None, None)) else {
        panic!("root must be an element");
    };
    assert!(kids.len() >= 3, "{kids:?}");
    assert_eq!(
        kids[0],
        Shape::Element(
            "span",
            vec!["class".into(), "data-testid".into()],
            vec![Shape::Text]
        )
    );
}
