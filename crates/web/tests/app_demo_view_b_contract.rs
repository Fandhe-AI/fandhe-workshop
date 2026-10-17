//! アプリのデモのワイヤーのビュー（変種 (b)）の契約試験（TASK-479 2/2。ART-1・ART-17）。
//!
//! 固定データ（`appdemo.json`。後の状態がちょうど 9 行）から、項目数・印の数と宿主・部品の数・状態の文字・欄の `id`・
//! 項目の形の不変・`tablist` と `tab` の数え上げ・a11y の閉じた一覧・エスケープを確かめる。
//! 期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_web::a11y::{
    check_complete, check_no_undefined, LiveRegion, PaneTab, Region, StatusNotice,
    LIVE_REGION_ATTRS, LOG_LIST_TESTID, TABLIST_LABEL, TABLIST_TESTID, TAB_TESTID, TESTID_ATTR,
};
use fandhe_workshop_web::app_demo_view::{app_demo_items_b, app_demo_list, EMPTY_FIELD_TEXT};
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
    let items = app_demo_items_b(&rows(), phase, &no_point);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = app_demo_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn one(kind: RowKind, cell: Cell) -> String {
    let r = Row::new(RowId::new("x").expect("id"), 0, kind, Some(cell), None).expect("行");
    let items = app_demo_items_b(&[r], Phase::Before, &no_point);
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
fn data_mark_は_li_自身にだけあり内側には無い() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            let li_open = &h[..h.find('>').expect("li の開きタグ")];
            assert!(li_open.starts_with("<li"), "{h}");
            assert_eq!(li_open.matches("data-mark=").count(), 1, "{h}");
            assert!(li_open.contains("data-kind="), "{h}");
            assert_eq!(h.matches("data-mark=").count(), 1, "{h}");
        }
    }
}

#[test]
fn 部品の数は種類ごとに期待どおり() {
    let (_, ha, _) = list(Phase::After);
    assert_eq!(count(&ha, "data-part=\"root\""), 1);
    assert_eq!(count(&ha, "<label "), 1);
    assert_eq!(count(&ha, "<input "), 1);
    assert_eq!(count(&ha, " readonly"), 1);
    assert_eq!(count(&ha, "<button type=\"button\""), 1);
    assert_eq!(count(&ha, "data-testid=\"app-demo-tab\""), 2);
    assert_eq!(count(&ha, "data-testid=\"app-demo-item-label\""), 3);
    assert_eq!(count(&ha, "data-testid=\"app-demo-item-detail\""), 3);
    // 見出し 1 件（class つき）と文字 1 件。
    assert_eq!(count(&ha, "<p class=\"app-demo-heading\""), 1);
    assert_eq!(count(&ha, "<p>"), 1);
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(count(&hb, "data-testid=\"app-demo-item-label\""), 2);
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
fn 無効と選択の属性は出さず_role_と_aria_と_svg_と見出しは前後とも_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for banned in [
            "disabled", "aria-", "role=", "<svg", "<h1", "<h2", "<h3", "<h4", "<h5", "<h6",
        ] {
            assert_eq!(count(&htmls, banned), 0, "{banned}");
        }
    }
}

#[test]
fn 状態の文字は選択中と無効と不明() {
    let (_, hb, _) = list(Phase::Before);
    assert!(hb[1].contains(">メモ（選択中）<"), "{}", hb[1]);
    assert!(hb[2].contains(">設定<"), "{}", hb[2]);
    assert!(hb[4].contains(">追加（無効）<"), "{}", hb[4]);
    let (_, ha, _) = list(Phase::After);
    assert!(ha[4].contains(">追加<"), "{}", ha[4]);
    let html = one(RowKind::Tab, state_cell("T", "ACTIVE"));
    assert!(html.contains(">T（不明）<"), "{html}");
}

#[test]
fn 欄の_id_と_for_は対応し_value_は空でも出る() {
    let (_, hb, _) = list(Phase::Before);
    let (_, ha, _) = list(Phase::After);
    for h in [&hb[3], &ha[3]] {
        // 行の並びの添字 3。
        assert!(h.contains("for=\"app-demo-field-3-control\""), "{h}");
        assert!(h.contains("id=\"app-demo-field-3-control\""), "{h}");
        assert!(h.contains("placeholder=\"（未入力）\""), "{h}");
        assert!(h.contains("value=\""), "{h}");
    }
    assert!(hb[3].contains("value=\"\""), "{}", hb[3]);
    assert!(ha[3].contains("value=\"買い物に行く\""), "{}", ha[3]);
    assert_eq!(EMPTY_FIELD_TEXT, "（未入力）");
    // 行の識別子（信頼できない値）は id に入らない。
    assert!(!hb[3].contains("a-field"), "{}", hb[3]);
}

#[test]
fn 種類の外の行でも描け_文字として出る() {
    let html = one(RowKind::Card, state_cell("カード", ""));
    assert!(html.contains("<p>カード</p>"), "{html}");
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
fn a11y_の閉じた一覧に前後とも違反しない() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
    }
}

fn tab(t: PaneTab, selected: bool) -> Node {
    el(
        "button",
        vec![
            ("role", "tab"),
            (TESTID_ATTR, TAB_TESTID),
            ("id", t.tab_id()),
            ("aria-controls", t.panel_id()),
            ("aria-selected", if selected { "true" } else { "false" }),
            ("tabindex", if selected { "0" } else { "-1" }),
        ],
        vec![text(t.label())],
    )
}

fn panel(t: PaneTab, children: Vec<Node>) -> Node {
    el(
        "section",
        vec![
            ("role", "tabpanel"),
            ("id", t.panel_id()),
            ("aria-labelledby", t.tab_id()),
        ],
        children,
    )
}

fn live(testid: &'static str) -> Node {
    let mut attrs = vec![(TESTID_ATTR, testid)];
    attrs.extend(LIVE_REGION_ATTRS);
    el("div", attrs, vec![text("x")])
}

fn status(testid: &'static str) -> Node {
    el(
        "div",
        vec![("role", "status"), (TESTID_ATTR, testid)],
        vec![],
    )
}

/// 契約どおりの最小の画面。成果物の領域に `view` を入れる。
fn page_with(view: Node) -> Node {
    let tabs: Vec<Node> = PaneTab::ALL
        .iter()
        .map(|t| tab(*t, *t == PaneTab::Artifacts))
        .collect();
    el(
        "main",
        vec![],
        vec![
            el(
                "section",
                vec![("aria-label", Region::Artifacts.aria_label())],
                vec![status(StatusNotice::View.testid()), view],
            ),
            el(
                "section",
                vec![("aria-label", Region::GuideAndLog.aria_label())],
                vec![
                    status(StatusNotice::Log.testid()),
                    el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], vec![]),
                ],
            ),
            el(
                "section",
                vec![("aria-label", Region::CommandInput.aria_label())],
                vec![status(StatusNotice::Input.testid())],
            ),
            live(LiveRegion::Response.testid()),
            live(LiveRegion::Left.testid()),
            el(
                "div",
                vec![
                    ("role", "tablist"),
                    (TESTID_ATTR, TABLIST_TESTID),
                    ("aria-label", TABLIST_LABEL),
                ],
                tabs,
            ),
            panel(PaneTab::Ops, vec![]),
            panel(PaneTab::Artifacts, vec![]),
        ],
    )
}

fn violations(node: &Node) -> String {
    match check_complete(node) {
        Ok(()) => String::new(),
        Err(v) => format!("{v:?}"),
    }
}

#[test]
fn 画面の_tablist_は_1_個で_tab_は_2_個のまま_契約の適合試験を通る() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        let page = page_with(node);
        assert_eq!(violations(&page), "", "{phase:?}");
        let html = render(&page);
        assert_eq!(html.matches("role=\"tablist\"").count(), 1, "{html}");
        assert_eq!(
            html.matches(&format!("data-testid=\"{TABLIST_TESTID}\""))
                .count(),
            1,
            "{html}"
        );
        assert_eq!(html.matches("role=\"tab\"").count(), 2, "{html}");
        assert_eq!(
            html.matches(&format!("data-testid=\"{TAB_TESTID}\""))
                .count(),
            2,
            "{html}"
        );
    }
}

#[test]
fn 親に_tablist_を足すと適合試験が落ちる_試作の形を採れない理由() {
    let items = app_demo_items_b(&rows(), Phase::Before, &no_point);
    let kids: Vec<Node> = items.into_iter().map(|(_, n)| n).collect();
    let bad = el("div", vec![("role", "tablist")], kids);
    let v = violations(&page_with(bad));
    assert!(!v.is_empty(), "role=tablist を足した画面は違反になる");
}

#[test]
fn タブの_span_に_role_tab_を足すと適合試験が落ちる() {
    let tabbed = el(
        "span",
        vec![("role", "tab"), (TESTID_ATTR, "app-demo-tab")],
        vec![text("メモ")],
    );
    let (_, _, node) = list(Phase::Before);
    let page = page_with(el("div", vec![], vec![node, tabbed]));
    let v = violations(&page);
    assert!(!v.is_empty(), "role=tab を足した画面は違反になる");
}

#[test]
fn ラベル_補足_状態と値はエスケープされ属性を閉じない() {
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
                detail: "\"><img src=x onerror=1>&".into(),
                state: "<b>active</b>".into(),
                tags: Vec::new(),
            },
        );
        for raw in ["<script", "<img", "<b>"] {
            assert!(!html.contains(raw), "{kind:?} {raw}: {html}");
        }
        assert!(html.contains("&lt;script&gt;"), "{kind:?}: {html}");
    }
    let field = one(
        RowKind::Field,
        Cell {
            label: "L".into(),
            detail: "\"><img src=x>".into(),
            ..Cell::default()
        },
    );
    assert!(
        field.contains("value=\"&quot;&gt;&lt;img src=x&gt;\""),
        "{field}"
    );
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = app_demo_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = app_demo_items_b(&rows(), Phase::Before, &no_point);
    let hidden = app_demo_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}

#[test]
fn app_css_はタブの横並びの規則を持ち_外部の読み込みを持たない() {
    assert!(APP_CSS.contains("[data-view=\"app_demo\"] > li[data-kind=\"tab\"]"));
    assert!(APP_CSS.contains("[data-view=\"app_demo\"] input"));
    assert!(APP_CSS.contains("min-height: 24px"));
    assert!(!APP_CSS.contains("@import"));
    assert!(!APP_CSS.contains("url("));
}
