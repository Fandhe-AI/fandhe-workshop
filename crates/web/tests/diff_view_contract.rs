//! 差分のビュー（変種 (b)）の契約試験（TASK-475 1/2。ART-1・ART-8）。
//!
//! 固定データ（`diff.json`）から、項目数・2 層の印・インデントを保つ `pre`・行番号・項目の形の不変・
//! a11y の閉じた一覧・エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。
//! 500 行の構成の確認は時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::diff_view::{diff_items, diff_list};

const DIFF: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/diff.json"
));
const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css"));

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
    let items = diff_items(&rows(), phase);
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
fn 項目数と_key_の並びは_json_の順と一致する() {
    let (kb, _, _) = list(Phase::Before);
    let (ka, _, _) = list(Phase::After);
    assert_eq!(kb, ["kh1", "kl1", "kl2", "kl3", "kl4", "kl7", "kl8"]);
    assert_eq!(
        ka,
        ["kh1", "kl1", "kl2", "kl3", "kl4", "kl5", "kl6", "kl7", "kl8"]
    );
}

#[test]
fn 層1の印の数は前が0_後が変更3_追加2_削除1_印なし3() {
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 7);
    let (_, ha, _) = list(Phase::After);
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 3, "{after}");
    assert_eq!(after.matches("data-mark=\"add\"").count(), 2, "{after}");
    assert_eq!(after.matches("data-mark=\"remove\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"\"").count(), 3, "{after}");
}

#[test]
fn 層1と層2は別の要素に出る() {
    let l4b = html_of(Phase::Before, "kl4");
    assert_eq!(text_of(&l4b, "mark-glyph"), "");
    assert_eq!(text_of(&l4b, "diff-sign"), "-");

    let l4a = html_of(Phase::After, "kl4");
    assert_eq!(text_of(&l4a, "mark-glyph"), "~");
    assert_eq!(text_of(&l4a, "diff-sign"), "+");

    let l7 = html_of(Phase::After, "kl7");
    assert_eq!(text_of(&l7, "mark-glyph"), "-");
    assert_eq!(text_of(&l7, "diff-sign"), "-");
    assert_eq!(l7.matches("data-testid=\"mark-glyph\"").count(), 1);
    assert_eq!(l7.matches("data-testid=\"diff-sign\"").count(), 1);
}

#[test]
fn 差分の記号は_state_と対応し_集計は手で数えた値と一致する() {
    let want_before = [
        ("kh1", ""),
        ("kl1", ""),
        ("kl2", ""),
        ("kl3", ""),
        ("kl4", "-"),
        ("kl7", "-"),
        ("kl8", ""),
    ];
    let want_after = [
        ("kh1", ""),
        ("kl1", ""),
        ("kl2", ""),
        ("kl3", ""),
        ("kl4", "+"),
        ("kl5", "+"),
        ("kl6", "+"),
        ("kl7", "-"),
        ("kl8", ""),
    ];
    for (phase, want) in [
        (Phase::Before, &want_before[..]),
        (Phase::After, &want_after[..]),
    ] {
        for &(key, sign) in want {
            assert_eq!(text_of(&html_of(phase, key), "diff-sign"), sign, "{key}");
        }
    }
    let (_, ha, _) = list(Phase::After);
    let signs: Vec<String> = ha.iter().map(|h| text_of(h, "diff-sign")).collect();
    assert_eq!(signs.iter().filter(|s| *s == "+").count(), 3);
    assert_eq!(signs.iter().filter(|s| *s == "-").count(), 1);
    let (_, hb, _) = list(Phase::Before);
    let signs: Vec<String> = hb.iter().map(|h| text_of(h, "diff-sign")).collect();
    assert_eq!(signs.iter().filter(|s| *s == "+").count(), 0);
}

#[test]
fn コードは_pre_で_インデントの空白が文字に残る() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        // span に戻しても空白は文字列に残るため、タグの数で確かめる。
        for h in &htmls {
            assert_eq!(h.matches("<pre ").count(), 1, "{h}");
            assert!(h.contains("</pre>"), "{h}");
        }
    }
    for key in ["kl4", "kl7"] {
        assert!(text_of(&html_of(Phase::Before, key), "diff-code").starts_with("    "));
    }
    for key in ["kl4", "kl5", "kl6"] {
        assert!(text_of(&html_of(Phase::After, key), "diff-code").starts_with("    "));
    }
    assert_eq!(
        text_of(&html_of(Phase::After, "kl3"), "diff-code"),
        "fn main() {"
    );
    assert_eq!(text_of(&html_of(Phase::After, "kl2"), "diff-code"), "");
}

#[test]
fn 行番号は旧と新の列に出る() {
    let b = html_of(Phase::Before, "kl4");
    assert_eq!(text_of(&b, "diff-old-no"), "4");
    assert_eq!(text_of(&b, "diff-new-no"), "");
    let a = html_of(Phase::After, "kl4");
    assert_eq!(text_of(&a, "diff-old-no"), "");
    assert_eq!(text_of(&a, "diff-new-no"), "4");
    for phase in [Phase::Before, Phase::After] {
        let h = html_of(phase, "kh1");
        assert_eq!(text_of(&h, "diff-old-no"), "");
        assert_eq!(text_of(&h, "diff-new-no"), "");
    }
    let l8 = html_of(Phase::After, "kl8");
    assert_eq!(text_of(&l8, "diff-old-no"), "7");
    assert_eq!(text_of(&l8, "diff-new-no"), "8");
}

#[test]
fn 形が崩れた行番号は空になり生の値は出ない() {
    for (detail, old, new) in [
        ("<b>1</b>:2", "", "2"),
        ("abc", "", ""),
        ("1:2:3", "1", ""),
        ("12345678901:1", "", "1"),
        ("1:12345678901", "1", ""),
    ] {
        let r = one_row("x", detail, "ctx");
        let h = render(&diff_items(&r, Phase::Before)[0].1);
        assert_eq!(text_of(&h, "diff-old-no"), old, "{detail}");
        assert_eq!(text_of(&h, "diff-new-no"), new, "{detail}");
        assert!(!h.contains("<b>"), "{h}");
        assert!(!h.contains("&lt;b&gt;"), "{h}");
        assert!(!h.contains("abc"), "{h}");
    }
}

#[test]
fn 未知の_state_は問符になり生の値は出ない() {
    let r = one_row("x", "1:1", "<script>");
    let h = render(&diff_items(&r, Phase::Before)[0].1);
    assert_eq!(text_of(&h, "diff-sign"), "?");
    assert!(!h.contains("script"), "{h}");
}

#[test]
fn 同じ_key_の項目の形は前後で変わらず_子は_6_つ() {
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
    for h in hb.iter().chain(ha.iter()) {
        // 記号・文字・旧・新・差分の記号・pre。
        for id in [
            "mark-glyph",
            "mark-text",
            "diff-old-no",
            "diff-new-no",
            "diff-sign",
            "diff-code",
        ] {
            assert_eq!(
                h.matches(&format!("data-testid=\"{id}\"")).count(),
                1,
                "{h}"
            );
        }
        assert_eq!(h.matches("<span").count(), 5, "{h}");
        assert_eq!(h.matches("<pre").count(), 1, "{h}");
    }
}

#[test]
fn a11y_の閉じた一覧に前後とも違反せず_role_と_aria_は無い() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        assert_eq!(html.matches("role=").count(), 0, "{html}");
        assert_eq!(html.matches("aria-").count(), 0, "{html}");
    }
}

#[test]
fn コードと行番号は文字としてエスケープされる() {
    let r = one_row("<script>alert(\"1\")</script> & x", "<img src=x>:2", "add");
    let h = render(&diff_items(&r, Phase::Before)[0].1);
    assert!(!h.contains("<script"), "{h}");
    assert!(!h.contains("<img"), "{h}");
    assert!(h.contains("&lt;script&gt;"), "{h}");
    assert!(h.contains("&amp;"), "{h}");
}

#[test]
fn 選ばれていない一覧は空で_hidden() {
    let items = diff_items(&rows(), Phase::Before);
    let hidden = render(&diff_list("ul", false, items).expect("一覧"));
    assert!(hidden.contains("hidden"), "{hidden}");
    assert!(!hidden.contains("diff-code"), "{hidden}");
}

#[test]
fn 五百行でも一意の_key_で一覧を作れる() {
    let v: Vec<ItemRow> = (0..500)
        .map(|i| {
            let c = Cell {
                label: format!("    line {i}"),
                detail: format!("{i}:{i}"),
                state: "ctx".into(),
                tags: Vec::new(),
            };
            Row::new(
                RowId::new(&format!("n{i:03}")).expect("id"),
                0,
                RowKind::Line,
                Some(c.clone()),
                Some(c),
            )
            .expect("行")
        })
        .collect();
    let items = diff_items(&v, Phase::After);
    assert_eq!(items.len(), 500);
    assert!(diff_list("ul", true, items).is_ok());
}

#[test]
fn css_に_4_つのクラスが現れる() {
    for class in ["df-old", "df-new", "df-sign", "df-code"] {
        assert!(APP_CSS.contains(&format!(".{class}")), "{class}");
    }
}
