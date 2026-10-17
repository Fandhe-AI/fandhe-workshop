//! チェックの一覧のビュー（変種 (b)）の契約試験（TASK-478。ART-1・ART-17）。
//!
//! 固定データ（`checks.json`）と規模のデータ（`checks-20.json`）から、項目数・印の数と宿主・部品の形・状態の記号つきの
//! 文字と色調・項目の形の不変・a11y の閉じた一覧・エスケープを確かめる。期待値は JSON と手で数えた値から作り、
//! 実装の関数から導かない。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::check_view::{check_items_b, check_list, CheckState};

const CHECKS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/checks.json"
));
const CHECKS_20: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/checks-20.json"
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
    let items = check_items_b(rows, phase);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = check_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
}

fn list(phase: Phase) -> (Vec<String>, Vec<String>, Node) {
    list_of(&rows_of(CHECKS), phase)
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
    let items = check_items_b(&[r], Phase::Before);
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
fn data_mark_は_li_自身にだけあり_内側には無い() {
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
fn 各項目はラベル_marker_補足を_1_つずつ持つ() {
    for phase in [Phase::Before, Phase::After] {
        let (keys, htmls, _) = list(phase);
        let all = htmls.join("");
        assert_eq!(
            all.matches("data-testid=\"check-label\"").count(),
            keys.len()
        );
        assert_eq!(
            all.matches("data-testid=\"check-detail\"").count(),
            keys.len()
        );
        assert_eq!(
            all.matches("data-scope=\"marker\" data-part=\"root\"")
                .count(),
            keys.len(),
            "{all}"
        );
    }
}

#[test]
fn 状態は記号つきの文字と色調で出る() {
    let want = [
        ("success", "✓ 成功", "info"),
        ("failure", "✗ 失敗", "danger"),
        ("running", "… 実行中", "warning"),
        ("pending", "… 待機中", "warning"),
        ("skipped", "– 省略", "neutral"),
        ("Success", "? 不明", "neutral"),
        ("", "? 不明", "neutral"),
    ];
    for (state, label, tone) in want {
        let html = one(Cell {
            label: "l".into(),
            state: state.into(),
            ..Cell::default()
        });
        assert!(html.contains(&format!(">{label}<")), "{state}: {html}");
        assert!(
            html.contains(&format!("data-tone=\"{tone}\"")),
            "{state}: {html}"
        );
    }
    let html = one(Cell {
        state: "Success".into(),
        ..Cell::default()
    });
    assert!(!html.contains("Success"), "{html}");
}

#[test]
fn 語彙の外の状態は不明になり_既知の状態へ寄せない() {
    for bad in ["Success", " success", "success ", "", "ok", "FAILURE"] {
        assert_eq!(CheckState::parse(bad), CheckState::Unknown, "{bad:?}");
    }
    assert_eq!(CheckState::ALL.len(), 6);
    for s in CheckState::ALL {
        assert_eq!(s.label(), format!("{} {}", s.glyph(), s.text()));
    }
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
fn 補足が空でも要素は出る() {
    let html = one(Cell {
        label: "l".into(),
        state: "success".into(),
        ..Cell::default()
    });
    assert_eq!(
        html.matches("data-testid=\"check-detail\"").count(),
        1,
        "{html}"
    );
}

#[test]
fn a11y_の閉じた一覧に前後とも違反せず_role_と_aria_と_svg_と_icon_は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        for banned in ["role=", "aria-", "<svg", "data-part=\"icon\"", "data-state"] {
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
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = check_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = check_items_b(&rows_of(CHECKS), Phase::Before);
    let hidden = check_list("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
}

#[test]
fn 規模の_20_件は前後とも_20_項目で_全行が変更で形が不変() {
    let rows = rows_of(CHECKS_20);
    assert_eq!(rows.len(), 20);
    let (kb, hb, _) = list_of(&rows, Phase::Before);
    let (ka, ha, _) = list_of(&rows, Phase::After);
    assert_eq!((kb.len(), ka.len()), (20, 20));
    assert_eq!(kb, ka);
    assert_eq!(hb.join("").matches("data-mark=\"\"").count(), 20);
    // 補足が全行で変わるため、後は全行が変更になる。
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 20, "{after}");
    for i in 0..20 {
        assert_eq!(shape(&hb[i]), shape(&ha[i]), "{}", kb[i]);
    }
}
