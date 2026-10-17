//! ファイルツリーのビュー（変種 (a)）の契約試験（TASK-473 2/2。ART-1・ART-2・ART-3・ART-9・ART-13）。
//!
//! 固定データ（`filetree.json`）から、項目数・印の数と宿主・a11y の閉じた一覧・`rich_text` の形・項目の形の不変・
//! エスケープを確かめる。期待値は JSON と手で数えた位置から作り、実装の関数から導かない。
//! 200 行の構成の確認は時間を測らない（測定は TASK-485）。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::file_tree_view::{file_tree_items_a, file_tree_list_a};

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
    let items = file_tree_items_a(&rows(), phase);
    let keys = items.iter().map(|(k, _)| k.clone()).collect();
    let htmls = items.iter().map(|(_, n)| render(n)).collect();
    let node = file_tree_list_a("ul", true, items).expect("一覧");
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
    let items = file_tree_items_a(&[r], Phase::Before);
    let html = render(&items[0].1);
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
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
fn a11y_の閉じた一覧に前後とも違反せず_木の属性と_aria_は_0_件() {
    for phase in [Phase::Before, Phase::After] {
        let (_, _, node) = list(phase);
        assert_eq!(check_no_undefined(&node), Ok(()), "{phase:?}");
        let html = render(&node);
        for banned in ["role=", "treeitem", "aria-", "<svg", "data-part"] {
            assert!(!html.contains(banned), "{banned}: {html}");
        }
    }
}

#[test]
fn 全項目が_rich_text_を持ち_太字と記号はディレクトリだけ() {
    let (_, htmls, _) = list(Phase::After);
    let rows = rows();
    let shown: Vec<&ItemRow> = rows
        .iter()
        .filter(|r| r.shown(Phase::After).is_some())
        .collect();
    assert_eq!(shown.len(), htmls.len());
    let mut dirs = 0;
    for (row, h) in shown.iter().zip(htmls.iter()) {
        assert_eq!(
            h.matches("<div class=\"fw-wire-rich-text").count(),
            1,
            "{h}"
        );
        let is_dir = row.kind() == RowKind::Dir;
        assert!(
            h.contains(&format!("data-kind=\"{}\"", row.kind().as_str())),
            "{h}"
        );
        assert_eq!(h.contains("fw-wire-bold"), is_dir, "{h}");
        assert_eq!(h.contains('\u{25be}'), is_dir, "{h}");
        dirs += usize::from(is_dir);
    }
    assert!(dirs > 0);
}

#[test]
fn 補足が空でも末尾の_span_と先頭の_span_は出る() {
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
    let items = file_tree_items_a(&[r], Phase::Before);
    let html = render(&items[0].1);
    assert!(html.contains("class=\"detail\""), "{html}");
    assert!(html.contains("class=\"icon\""), "{html}");
}

#[test]
fn 項目が_0_件か選ばれていない一覧には_role_が付かない() {
    let empty = file_tree_list_a("ul", true, Vec::new()).expect("一覧");
    assert!(!render(&empty).contains("role="));
    let items = file_tree_items_a(&rows(), Phase::Before);
    let hidden = file_tree_list_a("ul", false, items).expect("一覧");
    assert!(!render(&hidden).contains("role="));
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
    let items = file_tree_items_a(&v, Phase::After);
    assert_eq!(items.len(), 200);
    let mut keys: Vec<&String> = items.iter().map(|(k, _)| k).collect();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 200);
    assert!(file_tree_list_a("ul", true, items).is_ok());
}
