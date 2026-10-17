//! ファイルツリーのビュー（変種 (b)）の契約試験（TASK-473 1/2。ART-1・ART-2・ART-3・ART-9・ART-13）。
//!
//! 固定データ（`filetree.json`）から、項目数・印の数・木の属性・a11y の閉じた一覧・項目の形の不変・エスケープを確かめる。
//! 期待値は JSON と手で数えた位置から作り、実装の関数から導かない。200 行の構成の確認は時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_mark::no_point;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::file_tree_view::{file_tree_items, file_tree_list};

const FILETREE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/filetree.json"
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
    let root: serde_json::Value = serde_json::from_str(FILETREE).expect("JSON");
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
    let items = file_tree_items(&rows(), phase, &no_point);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = file_tree_list("ul", true, items).expect("一覧");
    (keys, htmls, node)
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
    assert_eq!(kb.len(), 10);
    assert_eq!(ka.len(), 11);
    assert_eq!(ka[0], "kmemo-app");
    assert_eq!(ka[4], "kmemo-app/notes/ideas.md");
    assert_eq!(ka[5], "kmemo-app/notes/old-draft.md");
    assert!(!kb.contains(&"kmemo-app/notes/ideas.md".to_owned()));
}

#[test]
fn 印の数は前が_0_後が追加1_削除1_変更2_印なし7() {
    let (_, hb, _) = list(Phase::Before);
    let before = hb.join("");
    assert_eq!(before.matches("data-mark=\"\"").count(), 10);
    let (_, ha, _) = list(Phase::After);
    let after = ha.join("");
    assert_eq!(after.matches("data-mark=\"add\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"remove\"").count(), 1, "{after}");
    assert_eq!(after.matches("data-mark=\"change\"").count(), 2, "{after}");
    assert_eq!(after.matches("data-mark=\"\"").count(), 7, "{after}");
}

#[test]
fn a11y_の閉じた一覧に前後とも違反しない() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        assert!(render(&node).contains("role=\"tree\""));
    }
}

#[test]
fn 木の属性は手で数えた位置と一致する() {
    // （level, posinset, setsize, 子を持つか）。
    let before = [
        (1, 1, 1, true),
        (2, 1, 4, true),
        (3, 1, 3, false),
        (3, 2, 3, false),
        (3, 3, 3, false),
        (2, 2, 4, true),
        (3, 1, 2, false),
        (3, 2, 2, false),
        (2, 3, 4, false),
        (2, 4, 4, false),
    ];
    let after = [
        (1, 1, 1, true),
        (2, 1, 4, true),
        (3, 1, 4, false),
        (3, 2, 4, false),
        (3, 3, 4, false),
        (3, 4, 4, false),
        (2, 2, 4, true),
        (3, 1, 2, false),
        (3, 2, 2, false),
        (2, 3, 4, false),
        (2, 4, 4, false),
    ];
    for (phase, want) in [(Phase::Before, &before[..]), (Phase::After, &after[..])] {
        let (_, htmls, _) = list(phase);
        assert_eq!(htmls.len(), want.len());
        for (h, &(l, p, s, e)) in htmls.iter().zip(want) {
            assert!(h.contains("role=\"treeitem\""), "{h}");
            assert!(h.contains(&format!("aria-level=\"{l}\"")), "{h}");
            assert!(h.contains(&format!("aria-posinset=\"{p}\"")), "{h}");
            assert!(h.contains(&format!("aria-setsize=\"{s}\"")), "{h}");
            assert_eq!(h.contains("aria-expanded=\"true\""), e, "{h}");
            assert!(!h.contains("aria-expanded=\"false\""), "{h}");
        }
    }
}

#[test]
fn ラベルは_item_text_で_禁じた属性は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, htmls, node) = list(phase);
        let all = render(&node);
        assert_eq!(all.matches("data-part=\"item-text\"").count(), htmls.len());
        for banned in [
            "aria-selected",
            "aria-hidden",
            "data-value",
            "role=\"group\"",
        ] {
            assert_eq!(all.matches(banned).count(), 0, "{banned}");
        }
        for h in &htmls {
            assert_eq!(h.matches("data-part=\"item-text\"").count(), 1, "{h}");
            assert!(
                h.contains("data-kind=\"dir\"") || h.contains("data-kind=\"file\""),
                "{h}"
            );
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
            // タグ・属性名・子の数と種別。属性の値（印・位置）と文字は前後で変わりうるので落とす。
            assert_eq!(shape(&hb[i]), shape(&ha[j]), "{k}");
            compared += 1;
        }
    }
    assert_eq!(compared, 10);
}

#[test]
fn 補足が空でも_span_は出る() {
    let r = Row::new(
        RowId::new("x").expect("id"),
        0,
        RowKind::File,
        Some(Cell {
            label: "l".into(),
            detail: String::new(),
            ..Cell::default()
        }),
        None,
    )
    .expect("行");
    let items = file_tree_items(&[r], Phase::Before, &no_point);
    assert!(render(&items[0].1).contains("class=\"detail\""));
}

#[test]
fn ラベルと補足は文字としてエスケープされる() {
    let r = Row::new(
        RowId::new("x").expect("id"),
        0,
        RowKind::File,
        Some(Cell {
            label: "<script>alert(1)</script>\"".into(),
            detail: "<img src=x onerror=1>".into(),
            ..Cell::default()
        }),
        None,
    )
    .expect("行");
    let items = file_tree_items(&[r], Phase::Before, &no_point);
    let html = render(&items[0].1);
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_tree_が付かない() {
    let empty = file_tree_list("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role=\"tree\""));
    let items = file_tree_items(&rows(), Phase::Before, &no_point);
    let hidden = file_tree_list("ul", false, items).expect("一覧");
    let html = render(&hidden);
    assert!(!html.contains("role=\"tree\""), "{html}");
    assert!(!html.contains("treeitem"), "{html}");
}

#[test]
fn 二百行でも一意の_key_で一覧を作れる() {
    let mut v = vec![Row::new(
        RowId::new("n000").expect("id"),
        0,
        RowKind::Dir,
        Some(Cell::default()),
        Some(Cell::default()),
    )
    .expect("行")];
    for i in 1..200 {
        v.push(
            Row::new(
                RowId::new(&format!("n{i:03}")).expect("id"),
                1,
                RowKind::File,
                Some(Cell {
                    label: format!("f{i}"),
                    ..Cell::default()
                }),
                Some(Cell {
                    label: format!("f{i}"),
                    ..Cell::default()
                }),
            )
            .expect("行"),
        );
    }
    let items = file_tree_items(&v, Phase::After, &no_point);
    assert_eq!(items.len(), 200);
    let mut keys: Vec<&String> = items.iter().map(|(k, _)| k).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 200);
    assert!(file_tree_list("ul", true, items).is_ok());
}
