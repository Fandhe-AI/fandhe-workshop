//! チェックの一覧のビュー（変種 (a)）の契約試験（TASK-478。ART-1・ART-17）。
//!
//! 固定データ（`checks.json`）から、項目数・印の数と宿主・部品の形・状態の記号と文字・項目の形の不変・
//! a11y の閉じた一覧・エスケープを確かめる。期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::check_view::{check_items_a, check_list, CheckState};

const CHECKS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/checks.json"
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
    let root: serde_json::Value = serde_json::from_str(CHECKS).expect("JSON");
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
    let items = check_items_a(&rows(), phase, &no_point);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = check_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn one(cell: Cell) -> String {
    let r = Row::new(
        RowId::new("x").expect("id"),
        0,
        RowKind::parse("check").expect("kind"),
        Some(cell),
        None,
    )
    .expect("行");
    let items = check_items_a(&[r], Phase::Before, &no_point);
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
    assert_eq!(kb, ["kc1", "kc2", "kc3", "kc4", "kc6"]);
    assert_eq!(ka, ["kc1", "kc2", "kc3", "kc4", "kc5", "kc6"]);
}

#[test]
fn 印の数は前が全行印なし_後が追加1_削除1_変更3_印なし1() {
    let (_, hb, _) = list(Phase::Before);
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 5);
    let (_, ha, _) = list(Phase::After);
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"add\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"remove\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 3, "{after}");
    assert_eq!(after.matches("data-mark=\"\"").count(), 1, "{after}");
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
fn 各項目は_rich_text_1_つと先頭の記号_1_つと状態の_tag_1_つを持つ() {
    for phase in [Phase::Before, Phase::After] {
        let (keys, htmls, _) = list(phase);
        let all = htmls.join("");
        assert_eq!(
            all.matches("class=\"fw-wire-rich-text ").count(),
            keys.len()
        );
        assert_eq!(all.matches("class=\"icon\"").count(), keys.len());
        assert_eq!(all.matches("class=\"fw-wire-tag ").count(), keys.len());
        assert_eq!(all.matches("class=\"check-detail\"").count(), keys.len());
    }
}

#[test]
fn 状態は記号と文字の両方で出て_強調は失敗だけに付く() {
    let (_, hb, _) = list(Phase::Before);
    let (_, ha, _) = list(Phase::After);
    // 前: c1 成功、c2 失敗、c3 実行中、c4 待機中、c6 省略。
    for (h, glyph, word) in [
        (&hb[0], "✓", "成功"),
        (&hb[1], "✗", "失敗"),
        (&hb[2], "…", "実行中"),
        (&hb[3], "…", "待機中"),
        (&hb[4], "–", "省略"),
    ] {
        assert!(h.contains(&format!(">{glyph}<")), "{h}");
        assert!(h.contains(&format!(">{word}<")), "{h}");
    }
    // 後: c4 が失敗、c2・c3 が成功。
    assert!(
        ha[3].contains(">失敗<") && ha[3].contains(">✗<"),
        "{}",
        ha[3]
    );
    assert!(
        ha[1].contains(">成功<") && ha[1].contains(">✓<"),
        "{}",
        ha[1]
    );
    assert_eq!(hb[1].matches("fw-wire-primary").count(), 1, "{}", hb[1]);
    assert_eq!(ha[3].matches("fw-wire-primary").count(), 1, "{}", ha[3]);
    assert_eq!(hb[0].matches("fw-wire-primary").count(), 0, "{}", hb[0]);
    assert_eq!(hb[2].matches("fw-wire-primary").count(), 0, "{}", hb[2]);
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
    assert_eq!(compared, 5);
}

#[test]
fn どの状態でも先頭の記号と補足の要素が出る() {
    for state in ["success", "failure", "running", "pending", "skipped", "x"] {
        let html = one(Cell {
            label: "l".into(),
            detail: String::new(),
            state: state.into(),
            tags: Vec::new(),
        });
        assert_eq!(html.matches("class=\"icon\"").count(), 1, "{state}: {html}");
        assert_eq!(
            html.matches("class=\"check-detail\"").count(),
            1,
            "{state}: {html}"
        );
        assert_eq!(
            html.matches("class=\"fw-wire-tag ").count(),
            1,
            "{state}: {html}"
        );
    }
}

#[test]
fn 語彙の外の状態は不明になり_既知の状態へ寄せない() {
    for bad in ["Success", " success", "success ", "", "ok", "FAILURE"] {
        assert_eq!(CheckState::parse(bad), CheckState::Unknown, "{bad:?}");
    }
    for (s, want) in [
        ("success", CheckState::Success),
        ("failure", CheckState::Failure),
        ("running", CheckState::Running),
        ("pending", CheckState::Pending),
        ("skipped", CheckState::Skipped),
    ] {
        assert_eq!(CheckState::parse(s), want);
    }
    assert_eq!(CheckState::ALL.len(), 6);
    let html = one(Cell {
        label: "l".into(),
        state: "Success".into(),
        ..Cell::default()
    });
    assert!(html.contains(">不明<") && html.contains(">?<"), "{html}");
    assert!(
        !html.contains("成功") && !html.contains("fw-wire-primary"),
        "{html}"
    );
}

#[test]
fn 状態の記号は印の記号と一致しない() {
    for s in CheckState::ALL {
        for mark in ["+", "-", "~"] {
            assert_ne!(s.glyph(), mark, "{s:?}");
        }
    }
}

#[test]
fn 状態が変わった行の印は_行の_change_だけで_欄ごとの印は無い() {
    let (_, ha, _) = list(Phase::After);
    let total: usize = ha.iter().map(|h| h.matches("data-mark=").count()).sum();
    assert_eq!(total, ha.len());
    assert_eq!(
        ha[1].matches("data-mark=\"change\"").count(),
        1,
        "{}",
        ha[1]
    );
}

#[test]
fn a11y_の閉じた一覧に前後とも違反せず_role_と_aria_と_svg_は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        for banned in ["role=", "aria-", "<svg", "data-part", "data-state"] {
            assert!(!html.contains(banned), "{banned}: {html}");
        }
    }
}

#[test]
fn ラベル_補足_状態は文字としてエスケープされる() {
    let html = one(Cell {
        label: "<script>alert(1)</script>\"".into(),
        detail: "<img src=x onerror=1>".into(),
        state: "<b>success</b>".into(),
        tags: Vec::new(),
    });
    for raw in ["<script", "<img", "<b>"] {
        assert!(!html.contains(raw), "{raw}: {html}");
    }
    assert!(html.contains("&lt;script&gt;"), "{html}");
    assert!(html.contains("&lt;img"), "{html}");
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = check_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = check_items_a(&rows(), Phase::Before, &no_point);
    let hidden = check_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}
