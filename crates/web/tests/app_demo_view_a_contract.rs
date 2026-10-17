//! アプリのデモのワイヤーのビュー（変種 (a)）の契約試験（TASK-479 1/2。ART-1・ART-17）。
//!
//! 固定データ（`appdemo.json`）から、項目数・印の数と宿主・部品の数・状態の文字・項目の形の不変・a11y の閉じた一覧・
//! エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::app_demo_view::{
    app_demo_items_a, app_demo_list, WireState, EMPTY_FIELD_TEXT,
};
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};

const APPDEMO: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/appdemo.json"
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
    let root: serde_json::Value = serde_json::from_str(APPDEMO).expect("JSON");
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
    let items = app_demo_items_a(&rows(), phase, &no_point);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = app_demo_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn one(kind: RowKind, cell: Cell) -> String {
    let r = Row::new(RowId::new("x").expect("id"), 0, kind, Some(cell), None).expect("行");
    let items = app_demo_items_a(&[r], Phase::Before, &no_point);
    render(&items[0].1)
}

fn state_cell(label: &str, state: &str) -> Cell {
    Cell {
        label: label.into(),
        state: state.into(),
        ..Cell::default()
    }
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

fn count(htmls: &[String], needle: &str) -> usize {
    htmls.iter().map(|h| h.matches(needle).count()).sum()
}

#[test]
fn 項目数と_key_の並びは_json_の順と一致する() {
    let (kb, _, _) = list(Phase::Before);
    let (ka, _, _) = list(Phase::After);
    assert_eq!(
        kb,
        [
            "ka-title",
            "ka-tab-list",
            "ka-tab-set",
            "ka-field",
            "ka-add",
            "ka-i1",
            "ka-i2",
            "ka-note"
        ]
    );
    assert_eq!(ka.len(), 9);
    assert_eq!(ka[7], "ka-i3");
}

#[test]
fn 印の数は前が_0_後が追加1_削除0_変更3_印なし5() {
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(count(&hb, "data-mark=\"\""), 8);
    let (_, ha, _) = list(Phase::After);
    assert_eq!(count(&ha, "data-mark=\"add\""), 1);
    assert_eq!(count(&ha, "data-mark=\"remove\""), 0);
    assert_eq!(count(&ha, "data-mark=\"change\""), 3);
    assert_eq!(count(&ha, "data-mark=\"\""), 5);
}

#[test]
fn data_mark_は_li_には無く内側の_div_にだけある() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            let li_open = &h[..h.find('>').expect("li の開きタグ")];
            assert!(li_open.starts_with("<li"), "{h}");
            assert!(!li_open.contains("data-mark"), "{h}");
            assert!(li_open.contains("data-kind="), "{h}");
            let div = &h[h.find("<div").expect("内側の div")..];
            let div_open = &div[..div.find('>').expect("div の開きタグ")];
            assert_eq!(div_open.matches("data-mark=").count(), 1, "{h}");
        }
    }
}

#[test]
fn 部品の数は種類ごとに期待どおり() {
    let (_, ha, _) = list(Phase::After);
    assert_eq!(count(&ha, "class=\"fw-wire-text "), 2);
    assert_eq!(count(&ha, "class=\"fw-wire-tag "), 2);
    assert_eq!(count(&ha, "fw-wire-rich-text "), 1);
    assert_eq!(count(&ha, "class=\"fw-wire-input "), 1);
    assert_eq!(count(&ha, "class=\"fw-wire-button "), 1);
    assert_eq!(count(&ha, "class=\"fw-wire-card-basic "), 3);
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(count(&hb, "class=\"fw-wire-card-basic "), 2);
}

#[test]
fn 同じ_key_の項目の形は前後で変わらない() {
    let (kb, hb, _) = list(Phase::Before);
    let (ka, ha, _) = list(Phase::After);
    let mut compared = 0;
    for (i, k) in kb.iter().enumerate() {
        if let Some(j) = ka.iter().position(|x| x == k) {
            assert_eq!(shape(&hb[i]), shape(&ha[j]), "{k}");
            compared += 1;
        }
    }
    assert_eq!(compared, 8);
    for must in ["ka-field", "ka-add", "ka-note"] {
        assert!(kb.iter().any(|k| k == must) && ka.iter().any(|k| k == must));
    }
}

#[test]
fn data_active_と_data_disabled_は前後とも_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        assert_eq!(count(&htmls, "data-active"), 0);
        assert_eq!(count(&htmls, "data-disabled"), 0);
    }
}

#[test]
fn 状態の語彙は完全一致で読み_語彙の外は不明になる() {
    for bad in ["ACTIVE", "disabled ", "x", " ", "Disabled"] {
        assert_eq!(WireState::parse(bad), WireState::Unknown, "{bad:?}");
    }
    assert_eq!(WireState::parse(""), WireState::Normal);
    assert_eq!(WireState::parse("active"), WireState::Active);
    assert_eq!(WireState::parse("disabled"), WireState::Disabled);
    assert_eq!(WireState::ALL.len(), 4);
    let html = one(RowKind::Tab, state_cell("T", "ACTIVE"));
    assert!(html.contains(">T（不明）<"), "{html}");
    assert!(!html.contains("fw-wire-primary"), "{html}");
}

#[test]
fn 選択中のタブだけ強調と文字が付き_無効のボタンは強調なしと文字が付く() {
    let (_, hb, _) = list(Phase::Before);
    // タブ「メモ」は active、「設定」は通常。
    assert!(hb[1].contains(">メモ（選択中）<"), "{}", hb[1]);
    assert_eq!(hb[1].matches("fw-wire-primary").count(), 1, "{}", hb[1]);
    assert!(hb[2].contains(">設定<"), "{}", hb[2]);
    assert_eq!(hb[2].matches("fw-wire-primary").count(), 0, "{}", hb[2]);
    // ボタン「追加」は前が disabled、後は通常。
    assert!(hb[4].contains(">追加（無効）<"), "{}", hb[4]);
    assert_eq!(hb[4].matches("fw-wire-primary").count(), 0, "{}", hb[4]);
    let (_, ha, _) = list(Phase::After);
    assert!(ha[4].contains(">追加<"), "{}", ha[4]);
    assert_eq!(ha[4].matches("fw-wire-primary").count(), 1, "{}", ha[4]);
}

#[test]
fn 欄は_detail_が空なら固定の文字_空でなければ_detail_が出る() {
    let (_, hb, _) = list(Phase::Before);
    assert!(hb[3].contains(EMPTY_FIELD_TEXT), "{}", hb[3]);
    let (_, ha, _) = list(Phase::After);
    assert!(ha[3].contains(">買い物に行く<"), "{}", ha[3]);
    assert!(!ha[3].contains(EMPTY_FIELD_TEXT), "{}", ha[3]);
}

#[test]
fn 種類の外の行でも描け_文字として出る() {
    let html = one(RowKind::Card, state_cell("カード", ""));
    assert!(html.contains("fw-wire-text"), "{html}");
    assert!(html.contains(">カード<"), "{html}");
}

#[test]
fn 変更行の印は行単位の_change_だけで_欄ごとの印は無い() {
    let (_, ha, _) = list(Phase::After);
    let total: usize = ha.iter().map(|h| h.matches("data-mark=").count()).sum();
    assert_eq!(total, ha.len());
    for i in [3, 4, 8] {
        assert_eq!(
            ha[i].matches("data-mark=\"change\"").count(),
            1,
            "{}",
            ha[i]
        );
    }
}

#[test]
fn a11y_の閉じた一覧に前後とも違反せず_role_と_aria_と_svg_は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        for banned in ["role=", "aria-", "<svg", "data-part"] {
            assert!(!html.contains(banned), "{banned}: {html}");
        }
    }
}

#[test]
fn ラベル_補足_状態は文字としてエスケープされる() {
    for kind in [
        RowKind::Heading,
        RowKind::Tab,
        RowKind::Field,
        RowKind::Button,
        RowKind::Item,
        RowKind::Text,
    ] {
        let html = one(
            kind,
            Cell {
                label: "<script>alert(1)</script>\"".into(),
                detail: "<img src=x onerror=1>".into(),
                state: "<b>active</b>".into(),
                tags: Vec::new(),
            },
        );
        for raw in ["<script", "<img", "<b>"] {
            assert!(!html.contains(raw), "{kind:?} {raw}: {html}");
        }
        assert!(html.contains("&lt;script&gt;"), "{kind:?}: {html}");
    }
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = app_demo_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = app_demo_items_a(&rows(), Phase::Before, &no_point);
    let hidden = app_demo_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}

#[test]
fn app_css_はタブの横並びの規則を持ち_外部の読み込みを持たない() {
    assert!(APP_CSS.contains("[data-view=\"app_demo\"] > li[data-kind=\"tab\"]"));
    assert!(!APP_CSS.contains("@import"));
    assert!(!APP_CSS.contains("url("));
}
