//! 差分のビュー（変種 (a)。対照専用）の契約試験（TASK-475 2/2。ART-1・ART-8）。
//!
//! 固定データ（`diff.json`）から、項目数・2 層の印・`wire_text` で描くコード（`pre` が無い）・行番号・項目の形の不変・
//! a11y の閉じた一覧・エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。
//! 500 行の構成の確認は時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::diff_view::{diff_items_a, diff_list};

const DIFF: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/diff.json"
));

fn cell(v: &serde_json::Value) -> Option<Cell> {
    if v.is_null() {
        return None;
    }
    let s = |k: &str| v[k].as_str().expect("文字列").to_owned();
    Some(Cell {
        label: s("label"),
        detail: s("detail"),
        state: s("state"),
        tags: Vec::new(),
    })
}

fn rows() -> Vec<ItemRow> {
    let root: serde_json::Value = serde_json::from_str(DIFF).expect("JSON");
    root["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| {
            Row::new(
                RowId::new(r["id"].as_str().expect("id")).expect("id"),
                u8::try_from(r["depth"].as_u64().expect("depth")).expect("u8"),
                RowKind::parse(r["kind"].as_str().expect("kind")).expect("kind"),
                cell(&r["before"]),
                cell(&r["after"]),
            )
            .expect("行")
        })
        .collect()
}

fn list(phase: Phase) -> (Vec<String>, Vec<String>, Node) {
    let items = diff_items_a(&rows(), phase);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = diff_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn html_of(phase: Phase, key: &str) -> String {
    let (keys, htmls, _) = list(phase);
    let i = keys.iter().position(|k| k == key).expect("key");
    htmls[i].clone()
}

/// `data-testid` が `id` の要素の文字（子は文字ノード 1 つ）。
fn text_of(html: &str, id: &str) -> String {
    let marker = format!("data-testid=\"{id}\"");
    let at = html.find(&marker).expect("testid");
    let rest = &html[at..];
    let start = rest.find('>').expect(">") + 1;
    let end = rest[start..].find("</").expect("</");
    rest[start..start + end].to_owned()
}

/// 開きタグの属性値と文字を落として、形だけにする。
fn shape(html: &str) -> String {
    let mut out = String::new();
    let mut in_val = false;
    let mut in_tag = false;
    for c in html.chars() {
        match (in_tag, in_val, c) {
            (_, false, '<') => {
                in_tag = true;
                out.push(c);
            }
            (true, false, '"') => in_val = true,
            (true, true, '"') => in_val = false,
            (true, true, _) => {}
            (true, false, '>') => {
                in_tag = false;
                out.push(c);
            }
            (true, false, _) => out.push(c),
            (false, _, _) => {}
        }
    }
    out
}

fn one_row(label: &str, detail: &str, state: &str) -> Vec<ItemRow> {
    vec![Row::new(
        RowId::new("x").expect("id"),
        0,
        RowKind::Line,
        Some(Cell {
            label: label.into(),
            detail: detail.into(),
            state: state.into(),
            tags: Vec::new(),
        }),
        None,
    )
    .expect("行")]
}

#[test]
fn a_項目数と_key_の並びは_json_の順と一致する() {
    let (kb, _, _) = list(Phase::Before);
    let (ka, _, _) = list(Phase::After);
    assert_eq!(kb, ["kh1", "kl1", "kl2", "kl3", "kl4", "kl7", "kl8"]);
    assert_eq!(
        ka,
        ["kh1", "kl1", "kl2", "kl3", "kl4", "kl5", "kl6", "kl7", "kl8"]
    );
}

#[test]
fn a_data_mark_は_li_には無く内側の_div_にだけある() {
    let (_, hb, _) = list(Phase::Before);
    for h in &hb {
        assert!(!h.starts_with("<li data-mark"), "{h}");
        assert_eq!(h.matches("<div").count(), 1, "{h}");
    }
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 7);
    let (_, ha, _) = list(Phase::After);
    for h in &ha {
        assert!(!h.starts_with("<li data-mark"), "{h}");
    }
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 3, "{after}");
    assert_eq!(after.matches("data-mark=\"add\"").count(), 2, "{after}");
    assert_eq!(after.matches("data-mark=\"remove\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"\"").count(), 3, "{after}");
}

#[test]
fn a_コードは_wire_text_で_pre_は無い() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            assert_eq!(h.matches("<pre").count(), 0, "{h}");
            assert_eq!(h.matches("fw-wire-text").count(), 1, "{h}");
        }
    }
    // 空白は文字としては残る。失われるのは表示の段（空白の畳み）である。
    assert!(html_of(Phase::Before, "kl4").contains(">    println"));
}

#[test]
fn a_層1と層2は別の要素に出る() {
    let l4a = html_of(Phase::After, "kl4");
    assert_eq!(text_of(&l4a, "mark-glyph"), "~");
    assert_eq!(text_of(&l4a, "diff-sign"), "+");
    let l7 = html_of(Phase::After, "kl7");
    assert_eq!(text_of(&l7, "mark-glyph"), "-");
    assert_eq!(text_of(&l7, "diff-sign"), "-");
}

#[test]
fn a_差分の記号は_state_と対応する() {
    let (_, ha, _) = list(Phase::After);
    let signs: Vec<String> = ha.iter().map(|h| text_of(h, "diff-sign")).collect();
    assert_eq!(signs.iter().filter(|s| *s == "+").count(), 3);
    assert_eq!(signs.iter().filter(|s| *s == "-").count(), 1);
    let r = one_row("x", "1:1", "<script>");
    let h = render(&diff_items_a(&r, Phase::Before)[0].1);
    assert_eq!(text_of(&h, "diff-sign"), "?");
    assert!(!h.contains("script"), "{h}");
}

#[test]
fn a_形が崩れた行番号は空になる() {
    for (detail, old, new) in [
        ("<b>1</b>:2", "", "2"),
        ("abc", "", ""),
        ("1:2:3", "1", ""),
        ("12345678901:1", "", "1"),
    ] {
        let r = one_row("x", detail, "ctx");
        let h = render(&diff_items_a(&r, Phase::Before)[0].1);
        assert_eq!(text_of(&h, "diff-old-no"), old, "{detail}");
        assert_eq!(text_of(&h, "diff-new-no"), new, "{detail}");
        assert!(!h.contains("abc"), "{h}");
    }
}

#[test]
fn a_同じ_key_の項目の形は前後で変わらない() {
    let (kb, hb, _) = list(Phase::Before);
    let (ka, ha, _) = list(Phase::After);
    let mut compared = 0;
    for (i, k) in kb.iter().enumerate() {
        if let Some(j) = ka.iter().position(|x| x == k) {
            assert_eq!(shape(&hb[i]), shape(&ha[j]), "{k}");
            compared += 1;
        }
    }
    assert_eq!(compared, 7);
}

#[test]
fn a_a11y_の閉じた一覧に違反せず_role_と_aria_は無い() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        assert_eq!(html.matches("role=").count(), 0, "{html}");
        assert_eq!(html.matches("aria-").count(), 0, "{html}");
    }
}

#[test]
fn a_コードと行番号は文字としてエスケープされる() {
    let r = one_row("<script>alert(\"1\")</script> & x", "<img src=x>:2", "add");
    let h = render(&diff_items_a(&r, Phase::Before)[0].1);
    assert!(!h.contains("<script"), "{h}");
    assert!(!h.contains("<img"), "{h}");
    assert!(h.contains("&lt;script&gt;"), "{h}");
}

#[test]
fn wireframe_の_text_の_css_は_nowrap_を持つ() {
    // G-4 の上流の原因の記録。版を上げて CSS が変わったら気づくための 1 件。
    assert!(fandhe_frontend_wireframe_ui::text::TEXT_CSS.contains("white-space: nowrap"));
}
