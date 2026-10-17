//! カードのビュー（変種 (a)）の契約試験（TASK-477 1/2。ART-1・ART-17）。
//!
//! 固定データ（`cards.json`）から、項目数・印の数と宿主・部品の形・状態の文字・項目の形の不変・a11y の閉じた一覧・
//! エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::card_view::{card_items_a, card_list, CardState};

const CARDS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/cards.json"
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

fn rows() -> Vec<ItemRow> {
    let root: serde_json::Value = serde_json::from_str(CARDS).expect("JSON");
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
    let items = card_items_a(&rows(), phase, &no_point);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = card_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
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
    let items = card_items_a(&[r], Phase::Before, &no_point);
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
fn data_mark_は_li_には無く内側の_div_にだけある() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            let li_open = &h[..h.find('>').expect("li の開きタグ")];
            assert!(li_open.starts_with("<li"), "{h}");
            assert!(!li_open.contains("data-mark"), "{h}");
            let div = &h[h.find("<div").expect("内側の div")..];
            let div_open = &div[..div.find('>').expect("div の開きタグ")];
            assert_eq!(div_open.matches("data-mark=").count(), 1, "{h}");
        }
    }
}

#[test]
fn 各項目は_card_basic_1_つと_補足_1_つと_tag_2_つを持つ() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, _) = list(phase);
        for h in &htmls {
            assert_eq!(h.matches("fw-wire-card-basic ").count(), 1, "{h}");
            assert_eq!(h.matches("fw-wire-card-basic-secondary").count(), 1, "{h}");
            assert_eq!(h.matches("class=\"fw-wire-tag ").count(), 2, "{h}");
        }
    }
}

#[test]
fn 状態の文字は固定の語彙に対応し_強調はオープンだけに付く() {
    let (_, hb, _) = list(Phase::Before);
    let (_, ha, _) = list(Phase::After);
    // pr1: 下書き -> オープン、pr2: オープン -> マージ済み。
    assert!(hb[0].contains(">下書き<"), "{}", hb[0]);
    assert!(ha[0].contains(">オープン<"), "{}", ha[0]);
    assert!(ha[1].contains(">マージ済み<"), "{}", ha[1]);
    assert!(ha[2].contains(">下書き<"), "{}", ha[2]);
    assert_eq!(hb[0].matches("fw-wire-primary").count(), 0, "{}", hb[0]);
    assert_eq!(ha[0].matches("fw-wire-primary").count(), 1, "{}", ha[0]);
    assert_eq!(ha[1].matches("fw-wire-primary").count(), 0, "{}", ha[1]);
    assert_eq!(hb[1].matches("fw-wire-primary").count(), 1, "{}", hb[1]);
}

#[test]
fn ラベルは区切りで連結した_tag_1_個で_空でも出る() {
    let (_, ha, _) = list(Phase::After);
    assert!(ha[0].contains(">保存・レビュー待ち<"), "{}", ha[0]);
    let (_, hb, _) = list(Phase::Before);
    // pr4（前のみ）は tags が空でも tag が 2 つ出る。
    assert_eq!(
        hb[2].matches("class=\"fw-wire-tag ").count(),
        2,
        "{}",
        hb[2]
    );
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
    assert_eq!(compared, 3);
}

#[test]
fn 補足とラベルが空でも形の部品は出る() {
    let html = one(Cell {
        label: "l".into(),
        detail: String::new(),
        state: "open".into(),
        tags: Vec::new(),
    });
    assert_eq!(
        html.matches("fw-wire-card-basic-secondary").count(),
        1,
        "{html}"
    );
    assert_eq!(html.matches("class=\"fw-wire-tag ").count(), 2, "{html}");
}

#[test]
fn 語彙の外の状態は不明になり_既知の状態へ寄せない() {
    for bad in ["OPEN", "open ", "", "Merged", "unknown"] {
        assert_eq!(CardState::parse(bad), CardState::Unknown, "{bad:?}");
    }
    for (s, want) in [
        ("draft", CardState::Draft),
        ("open", CardState::Open),
        ("merged", CardState::Merged),
        ("closed", CardState::Closed),
    ] {
        assert_eq!(CardState::parse(s), want);
    }
    assert_eq!(CardState::ALL.len(), 5);
    let html = one(Cell {
        label: "l".into(),
        state: "OPEN".into(),
        ..Cell::default()
    });
    assert!(html.contains(">不明<"), "{html}");
    assert!(!html.contains("fw-wire-primary"), "{html}");
}

#[test]
fn 状態とラベルが変わった行の印は_行の_change_だけで_欄ごとの印は無い() {
    let (_, ha, _) = list(Phase::After);
    // 全項目で data-mark は 1 件ずつ（項目数と同じ）。
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
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = card_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = card_items_a(&rows(), Phase::Before, &no_point);
    let hidden = card_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}
