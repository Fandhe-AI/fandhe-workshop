//! カードのビュー（変種 (b)）の契約試験（TASK-477 2/2。ART-1・ART-17）。
//!
//! 固定データ（`cards.json`）と規模のデータ（`cards-20.json`）から、項目数・印の数と宿主・部品の形・状態の文字と色調・
//! 項目の形の不変・a11y の閉じた一覧・エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::card_view::{card_items_b, card_list};

const CARDS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/cards.json"
));
const CARDS_20: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/cards-20.json"
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
        tags: v["tags"]
            .as_array()
            .expect("配列")
            .iter()
            .map(|t| t.as_str().expect("文字列").to_owned())
            .collect(),
    })
}

fn rows_of(json: &str) -> Vec<ItemRow> {
    let root: serde_json::Value = serde_json::from_str(json).expect("JSON");
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

fn list_of(rows: &[ItemRow], phase: Phase) -> (Vec<String>, Vec<String>, Node) {
    let items = card_items_b(rows, phase);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = card_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn list(phase: Phase) -> (Vec<String>, Vec<String>, Node) {
    list_of(&rows_of(CARDS), phase)
}

fn one(cell: Cell) -> String {
    let r = Row::new(
        RowId::new("x").expect("id"),
        0,
        RowKind::Card,
        Some(cell),
        None,
    )
    .expect("行");
    let items = card_items_b(&[r], Phase::Before);
    render(&items[0].1)
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

#[test]
fn 項目数と_key_の並びは_json_の順と一致する() {
    let (kb, _, _) = list(Phase::Before);
    let (ka, _, _) = list(Phase::After);
    assert_eq!(kb, ["kpr1", "kpr2", "kpr4"]);
    assert_eq!(ka, ["kpr1", "kpr2", "kpr3", "kpr4"]);
}

#[test]
fn 印の数は前が_0_後が追加1_削除1_変更2_印なし0() {
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 3);
    let (_, ha, _) = list(Phase::After);
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"add\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"remove\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 2, "{after}");
    assert_eq!(after.matches("data-mark=\"\"").count(), 0, "{after}");
}

#[test]
fn data_mark_は_li_自身にあり内側の要素には無い() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            let li_open = &h[..h.find('>').expect("li の開きタグ")];
            assert!(li_open.starts_with("<li"), "{h}");
            assert_eq!(li_open.matches("data-mark=").count(), 1, "{h}");
            assert_eq!(h.matches("data-mark=").count(), 1, "{h}");
        }
    }
}

#[test]
fn 各項目は_article_marker_ul_を_1_つずつ持ち_ラベルの_li_は_1_個() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            assert_eq!(h.matches("<article").count(), 1, "{h}");
            assert_eq!(h.matches("data-scope=\"marker\"").count(), 2, "{h}");
            assert_eq!(h.matches("data-part=\"root\"").count(), 1, "{h}");
            assert_eq!(h.matches("data-part=\"content\"").count(), 1, "{h}");
            assert_eq!(h.matches("data-testid=\"card-tags\"").count(), 1, "{h}");
            assert_eq!(h.matches("<p ").count(), 1, "{h}");
            // 項目自身の li とラベルの li の 2 個だけ。
            assert_eq!(h.matches("<li").count(), 2, "{h}");
            assert_eq!(h.matches("<h").count(), 0, "{h}");
        }
    }
}

#[test]
fn 状態の文字と色調は固定の語彙に対応する() {
    let want = [
        ("draft", "下書き", "warning"),
        ("open", "オープン", "neutral"),
        ("merged", "マージ済み", "info"),
        ("closed", "クローズ", "danger"),
        ("OPEN", "不明", "neutral"),
        ("", "不明", "neutral"),
    ];
    for (state, text, tone) in want {
        let html = one(Cell {
            label: "l".into(),
            state: state.into(),
            ..Cell::default()
        });
        assert!(html.contains(&format!(">{text}<")), "{state}: {html}");
        assert!(
            html.contains(&format!("data-tone=\"{tone}\"")),
            "{state}: {html}"
        );
    }
    // 語彙の外の生の値は描画に出ない。
    let html = one(Cell {
        state: "OPEN".into(),
        ..Cell::default()
    });
    assert!(!html.contains("OPEN"), "{html}");
}

#[test]
fn ラベルは区切りで連結した_li_1_個で_空でも_ul_と_li_が出る() {
    let (_, ha, _) = list(Phase::After);
    assert!(ha[0].contains(">保存・レビュー待ち<"), "{}", ha[0]);
    let html = one(Cell::default());
    assert_eq!(
        html.matches("data-testid=\"card-tags\"").count(),
        1,
        "{html}"
    );
    assert_eq!(html.matches("<li").count(), 2, "{html}");
}

#[test]
fn 補足が空でも_p_が出る() {
    let html = one(Cell {
        label: "l".into(),
        ..Cell::default()
    });
    assert_eq!(
        html.matches("data-testid=\"card-detail\"").count(),
        1,
        "{html}"
    );
}

#[test]
fn 同じ_key_の項目の形は前後で変わらない_ラベルの件数が変わる行を含む() {
    let (kb, hb, _) = list(Phase::Before);
    let (ka, ha, _) = list(Phase::After);
    let mut compared = 0;
    for (i, k) in kb.iter().enumerate() {
        if let Some(j) = ka.iter().position(|x| x == k) {
            assert_eq!(shape(&hb[i]), shape(&ha[j]), "{k}");
            compared += 1;
        }
    }
    assert_eq!(compared, 3);
    // pr1 はラベルが前 1 件・後 2 件である（連結した 1 個の li のため形は変わらない）。
    assert!(!hb[0].contains('・'), "{}", hb[0]);
    assert!(ha[0].contains('・'), "{}", ha[0]);
}

#[test]
fn 状態とラベルが変わった行の印は_行の_change_だけで_欄ごとの印は無い() {
    let (_, ha, _) = list(Phase::After);
    let total: usize = ha.iter().map(|h| h.matches("data-mark=").count()).sum();
    assert_eq!(total, ha.len());
    assert_eq!(
        ha[0].matches("data-mark=\"change\"").count(),
        1,
        "{}",
        ha[0]
    );
}

#[test]
fn a11y_の閉じた一覧に前後とも違反せず_role_と_aria_と_svg_と_icon_は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        for banned in ["role=", "aria-", "<svg", "data-part=\"icon\""] {
            assert!(!html.contains(banned), "{banned}: {html}");
        }
    }
}

#[test]
fn ラベル_補足_状態_tags_は文字としてエスケープされる() {
    let html = one(Cell {
        label: "<script>alert(1)</script>\"".into(),
        detail: "<img src=x onerror=1>".into(),
        state: "<b>open</b>".into(),
        tags: vec!["<i>t</i>".into(), "\"q\"".into()],
    });
    for raw in ["<script", "<img", "<b>", "<i>"] {
        assert!(!html.contains(raw), "{raw}: {html}");
    }
    assert!(html.contains("&lt;script&gt;"), "{html}");
    assert!(html.contains("&lt;i&gt;t&lt;/i&gt;"), "{html}");
    assert!(html.contains(">不明<"), "{html}");
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = card_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = card_items_b(&rows_of(CARDS), Phase::Before);
    let hidden = card_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}

#[test]
fn 規模の_20_件は前後とも_20_項目で_変更の印は_json_から数えた値と一致し形が不変() {
    let rows = rows_of(CARDS_20);
    assert_eq!(rows.len(), 20);
    // JSON から数える: 前後の両方があり中身が異なる行。
    let root: serde_json::Value = serde_json::from_str(CARDS_20).expect("JSON");
    let changed = root["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .filter(|r| !r["before"].is_null() && !r["after"].is_null() && r["before"] != r["after"])
        .count();
    assert_eq!(changed, 8);
    let (kb, hb, nb) = list_of(&rows, Phase::Before);
    let (ka, ha, na) = list_of(&rows, Phase::After);
    assert_eq!((kb.len(), ka.len()), (20, 20));
    assert_eq!(kb, ka);
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 20);
    assert_eq!(ha.join("").matches("data-mark=\"change\"").count(), changed);
    assert_eq!(ha.join("").matches("data-mark=\"\"").count(), 20 - changed);
    for (b, a) in hb.iter().zip(&ha) {
        assert_eq!(shape(b), shape(a));
    }
    assert_eq!(check_no_undefined(&nb), Ok(()));
    assert_eq!(check_no_undefined(&na), Ok(()));
}
