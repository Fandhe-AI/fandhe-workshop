//! 模擬ログインの画面にパスワード欄がないことの契約試験（TASK-492。ART-24・ROUND1-25）。
//!
//! 模擬ログインのデータ（`appdemo-login.json`。前 6 行・後 7 行）をアプリのデモのワイヤー (a)・(b) で描き、
//! パスワードの欄・属性・文言が前後とも 0 件であることを確かめる。負の対照（`appdemo-login-contrast.json`）では
//! 文言の計数が 1 以上になり、計数が空振りしていないことを示す。期待値は JSON と手で数えた値から作り、実装の関数から導かない。

use fandhe_frontend_core::render;
use fandhe_workshop_web::app_demo_view::{app_demo_items_a, app_demo_items_b};
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};

const LOGIN: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/appdemo-login.json"
));
const CONTRAST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/data/artifact_view_scale/appdemo-login-contrast.json"
));

/// パスワードを求める文言の語（小文字で比べる）。
const WORDS: [&str; 4] = ["パスワード", "password", "暗証", "パスコード"];

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

fn rows(src: &str) -> Vec<ItemRow> {
    let root: serde_json::Value = serde_json::from_str(src).expect("JSON");
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

/// 変種 (a)・(b) を描いた HTML（項目ごと）。
fn htmls(src: &str, phase: Phase, variant_b: bool) -> Vec<String> {
    let r = rows(src);
    let items = if variant_b {
        app_demo_items_b(&r, phase, &no_point)
    } else {
        app_demo_items_a(&r, phase, &no_point)
    };
    items.iter().map(|(_, n)| render(n)).collect()
}

fn count(htmls: &[String], needle: &str) -> usize {
    htmls
        .iter()
        .map(|h| h.to_lowercase().matches(needle).count())
        .sum()
}

fn word_count(htmls: &[String]) -> usize {
    WORDS.iter().map(|w| count(htmls, &w.to_lowercase())).sum()
}

#[test]
fn 項目数は前が_6_後が_7_対照は前が_7_後が_8() {
    for variant_b in [false, true] {
        assert_eq!(htmls(LOGIN, Phase::Before, variant_b).len(), 6);
        assert_eq!(htmls(LOGIN, Phase::After, variant_b).len(), 7);
        assert_eq!(htmls(CONTRAST, Phase::Before, variant_b).len(), 7);
        assert_eq!(htmls(CONTRAST, Phase::After, variant_b).len(), 8);
    }
}

#[test]
fn 模擬ログインにはパスワードの欄も属性も文言も前後とも_0_件() {
    for variant_b in [false, true] {
        for phase in [Phase::Before, Phase::After] {
            let h = htmls(LOGIN, phase, variant_b);
            assert_eq!(count(&h, "type=\"password\""), 0, "{variant_b} {phase:?}");
            assert_eq!(count(&h, "autocomplete="), 0, "{variant_b} {phase:?}");
            assert_eq!(word_count(&h), 0, "{variant_b} {phase:?}: {h:?}");
        }
    }
}

#[test]
fn 変種_b_の入力欄はすべて_type_text_で読み取り専用() {
    for phase in [Phase::Before, Phase::After] {
        let h = htmls(LOGIN, phase, true);
        // 名前の欄が 1 件ある（空振りの防止）。
        assert_eq!(count(&h, "<input "), 1, "{phase:?}");
        assert_eq!(count(&h, "type=\"text\""), 1, "{phase:?}");
        assert_eq!(count(&h, " readonly"), 1, "{phase:?}");
    }
}

#[test]
fn 名前を選ぶ行は_item_で入力欄を持たない() {
    // 変種 (a)・(b) とも、名前の行 2 件は項目の部品（入力欄でない）として出る。
    let h = htmls(LOGIN, Phase::Before, true);
    assert_eq!(count(&h, "data-testid=\"app-demo-item-label\""), 2);
    assert!(h.iter().any(|x| x.contains("さくら")));
    assert!(h.iter().any(|x| x.contains("たろう")));
}

#[test]
fn 対照では文言の計数が_1_以上になり_計数は空振りしていない() {
    for variant_b in [false, true] {
        for phase in [Phase::Before, Phase::After] {
            let h = htmls(CONTRAST, phase, variant_b);
            assert!(word_count(&h) >= 1, "{variant_b} {phase:?}: {h:?}");
        }
    }
}
