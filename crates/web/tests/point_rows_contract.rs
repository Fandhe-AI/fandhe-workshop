//! 7 種のビューの「指せる行」の契約試験（TASK-480 1/2。ART-11）。
//!
//! 固定データを 7 種のビュー × 許される変種（差分・文書の (a) は対照の口）で描き、宛先のある行が `data-action`・
//! `data-payload`・`data-row`・`tabindex` を組で持ち、宛先の無い行が 1 つも持たないことを確かめる。
//! 宛先は呼び出し側が注入する（`PointOf`）。ここでは「先頭の行だけ表示だけ、残りは指せる」を注入する。
//! 期待値は属性名のリテラルと行数から作り、実装の関数から導かない。クリックの受け付けは `point_click_contract.rs`（TASK-480 2/2）が確かめる。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_contract::doc::Block;
use fandhe_workshop_pack::markdown::parse;
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::app_demo_view::{app_demo_items, app_demo_list};
use fandhe_workshop_web::artifact_doc_rows::align_blocks;
use fandhe_workshop_web::artifact_mark::{no_point, point_all, PointOf};
use fandhe_workshop_web::artifact_view::{
    Cell, DocRow, ItemRow, Phase, PointTarget, Row, RowId, RowKind, Variant,
};
use fandhe_workshop_web::card_view::{card_items, card_list};
use fandhe_workshop_web::check_view::{check_items, check_list};
use fandhe_workshop_web::diff_view::{diff_items, diff_items_a, diff_list};
use fandhe_workshop_web::document_view::{doc_items, doc_items_a, doc_list};
use fandhe_workshop_web::file_tree_view::{
    file_tree_items, file_tree_items_a, file_tree_list, file_tree_list_a,
};
use fandhe_workshop_web::task_tree::task_tree_list;

macro_rules! sample {
    ($name:literal) => {
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../testdata/artifact-view-parts/test-data/sample/",
            $name
        ))
    };
}

const FOUR: [&str; 4] = [
    "data-action=\"ui\"",
    "data-payload=\"",
    "data-row=\"",
    "tabindex=\"0\"",
];

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

fn doc_rows() -> Vec<DocRow<Block>> {
    align_blocks(
        "document",
        &parse(sample!("doc-before.md")).blocks,
        &parse(sample!("doc-after.md")).blocks,
    )
    .expect("行")
}

type Items = Vec<(String, Node)>;

/// 1 つの（ビュー, 変種）の組。`build` は注入された宛先でその状態の項目を作り、`list` は親の一覧を作る。
struct Case {
    name: &'static str,
    view: &'static str,
    first: RowId,
    build: Box<dyn Fn(Phase, PointOf) -> Items>,
    list: Box<dyn Fn(Phase, PointOf) -> Node>,
}

fn items_case(
    name: &'static str,
    json: &'static str,
    f: impl Fn(&[ItemRow], Phase, PointOf) -> Items + Clone + 'static,
    l: impl Fn(Items) -> Node + Clone + 'static,
) -> Case {
    let rows = rows_of(json);
    let first = rows[0].id().clone();
    let rows2 = rows.clone();
    let (f2, l2) = (f.clone(), l);
    Case {
        name,
        view: name,
        first,
        build: Box::new(move |p, pt| f(&rows, p, pt)),
        list: Box::new(move |p, pt| l2(f2(&rows2, p, pt))),
    }
}

fn doc_case(name: &'static str, a: bool) -> Case {
    let rows = doc_rows();
    let first = rows[0].id().clone();
    let rows2 = rows.clone();
    let f = move |rows: &[DocRow<Block>], p, pt: PointOf| {
        if a {
            doc_items_a(rows, p, pt)
        } else {
            doc_items(rows, p, pt)
        }
    };
    Case {
        name,
        view: name,
        first,
        build: Box::new(move |p, pt| f(&rows, p, pt)),
        list: Box::new(move |p, pt| doc_list("ul", true, f(&rows2, p, pt)).expect("一覧")),
    }
}

#[allow(
    clippy::vec_init_then_push,
    reason = "ビューごとに組み立て方が違う並びを、読み順のまま 1 件ずつ足す"
)]
fn cases() -> Vec<Case> {
    let mut v = Vec::new();
    v.push(items_case(
        "file_tree/b",
        sample!("filetree.json"),
        file_tree_items,
        |i| file_tree_list("ul", true, i).expect("一覧"),
    ));
    v.push(items_case(
        "file_tree/a",
        sample!("filetree.json"),
        file_tree_items_a,
        |i| file_tree_list_a("ul", true, i).expect("一覧"),
    ));
    v.push(items_case(
        "diff/b",
        sample!("diff.json"),
        diff_items,
        |i| diff_list("ul", true, i).expect("一覧"),
    ));
    v.push(items_case(
        "diff/a-contrast",
        sample!("diff.json"),
        diff_items_a,
        |i| diff_list("ul", true, i).expect("一覧"),
    ));
    v.push(doc_case("document/b", false));
    v.push(doc_case("document/a-contrast", true));
    for (name, variant) in [("card_list/a", Variant::A), ("card_list/b", Variant::B)] {
        v.push(items_case(
            name,
            sample!("cards.json"),
            move |r, p, pt| card_items(r, p, variant, pt),
            |i| card_list("ul", true, i).expect("一覧"),
        ));
    }
    for (name, variant) in [("check_list/a", Variant::A), ("check_list/b", Variant::B)] {
        v.push(items_case(
            name,
            sample!("checks.json"),
            move |r, p, pt| check_items(r, p, variant, pt),
            |i| check_list("ul", true, i).expect("一覧"),
        ));
    }
    for (name, variant) in [("app_demo/a", Variant::A), ("app_demo/b", Variant::B)] {
        v.push(items_case(
            name,
            sample!("appdemo.json"),
            move |r, p, pt| app_demo_items(r, p, variant, pt),
            |i| app_demo_list("ul", true, i).expect("一覧"),
        ));
    }
    v
}

/// 先頭の行だけ表示だけ、残りは指せる行にする宛先。
fn rule<'a>(view: &'a str, first: &'a RowId) -> impl Fn(&RowId) -> Option<PointTarget> + 'a {
    move |row| {
        if row == first {
            None
        } else {
            PointTarget::new(view, row.clone()).ok()
        }
    }
}

fn attr_value<'a>(html: &'a str, name: &str) -> Option<&'a str> {
    let start = html.find(&format!(" {name}=\""))? + name.len() + 3;
    let rest = &html[start..];
    Some(&rest[..rest.find('"')?])
}

#[test]
fn 宛先のある行は_4_属性を組で持ち_宛先の無い行は_1_つも持たない() {
    for case in cases() {
        for phase in [Phase::Before, Phase::After] {
            let r = rule(case.view, &case.first);
            let items = (case.build)(phase, &r);
            let mut pointed = 0;
            for (key, node) in &items {
                let html = render(node);
                // 根の開始タグだけを見る（内側の属性を数えない）。
                let root = &html[..html.find('>').expect("開始タグ")];
                let counts: Vec<usize> = FOUR.iter().map(|a| root.matches(a).count()).collect();
                if *key == format!("k{}", case.first.as_str()) {
                    assert_eq!(counts, [0, 0, 0, 0], "{} {phase:?} {key}", case.name);
                } else {
                    assert_eq!(counts, [1, 1, 1, 1], "{} {phase:?} {key}", case.name);
                    pointed += 1;
                    // 順は action・payload・row・tabindex。
                    let pos: Vec<usize> =
                        FOUR.iter().map(|a| root.find(a).expect("属性")).collect();
                    assert!(
                        pos.windows(2).all(|w| w[0] < w[1]),
                        "{} {key}: {root}",
                        case.name
                    );
                    // payload が宛先に戻る。
                    let payload = attr_value(root, "data-payload").expect("payload");
                    let t = PointTarget::decode("ui", payload).expect("復号");
                    assert_eq!(t.view(), case.view);
                    assert_eq!(format!("k{}", t.row().as_str()), *key);
                    assert_eq!(attr_value(root, "data-row"), Some(t.row().as_str()));
                }
            }
            // 属性は内側に漏れない: 全体の数は指せる行の数と一致する。
            let all: String = items.iter().map(|(_, n)| render(n)).collect();
            for a in FOUR {
                assert_eq!(
                    all.matches(a).count(),
                    pointed,
                    "{} {phase:?} {a}",
                    case.name
                );
            }
            assert!(pointed > 0, "{} {phase:?}", case.name);
        }
    }
}

#[test]
fn 宛先が無ければ_4_属性は_1_つも出ない() {
    for case in cases() {
        for phase in [Phase::Before, Phase::After] {
            let all: String = (case.build)(phase, &no_point)
                .iter()
                .map(|(_, n)| render(n))
                .collect();
            for a in FOUR {
                assert_eq!(all.matches(a).count(), 0, "{} {phase:?} {a}", case.name);
            }
        }
    }
}

#[test]
fn 同じ_key_の項目は前後で指せるかどうかが変わらない() {
    for case in cases() {
        let r = rule(case.view, &case.first);
        let before = (case.build)(Phase::Before, &r);
        let after = (case.build)(Phase::After, &r);
        for (k, n) in &before {
            if let Some((_, m)) = after.iter().find(|(k2, _)| k2 == k) {
                let (a, b) = (render(n), render(m));
                for at in FOUR {
                    assert_eq!(
                        a.matches(at).count(),
                        b.matches(at).count(),
                        "{} {k} {at}",
                        case.name
                    );
                }
            }
        }
    }
}

#[test]
fn 一覧全体は前後とも_a11y_の違反が_0() {
    for case in cases() {
        for phase in [Phase::Before, Phase::After] {
            let r = rule(case.view, &case.first);
            let list = (case.list)(phase, &r);
            assert_eq!(check_no_undefined(&list), Ok(()), "{} {phase:?}", case.name);
        }
    }
}

#[test]
fn タスクツリーは_a_b_とも指せる行を持ち_木の_b_も違反が_0() {
    let rows = rows_of(sample!("tasks.json"));
    let first = rows[0].id().clone();
    for variant in [Variant::A, Variant::B] {
        for phase in [Phase::Before, Phase::After] {
            let r = rule("task_tree", &first);
            let list = task_tree_list(&rows, phase, variant, true, &r).expect("一覧");
            assert_eq!(check_no_undefined(&list), Ok(()), "{variant:?} {phase:?}");
            let html = render(&list);
            let shown = html.matches("data-mark=").count();
            for a in FOUR {
                // 先頭の行が出ている状態では 1 行少ない。
                let first_shown = task_items_shown(&rows, phase, &first);
                assert_eq!(
                    html.matches(a).count(),
                    shown - usize::from(first_shown),
                    "{variant:?} {phase:?} {a}"
                );
            }
            if variant == Variant::B {
                assert!(html.contains("role=\"treeitem\""), "{html}");
            }
        }
    }
}

fn task_items_shown(rows: &[ItemRow], phase: Phase, first: &RowId) -> bool {
    rows.iter()
        .any(|r| r.id() == first && r.shown(phase).is_some())
}

#[test]
fn 危険な文字を含む_view_と_row_はエスケープされ_復号で元の組に戻る() {
    let view = "<b>\"x\"&";
    let id = "<i onclick=\"1\">&'";
    let row = Row::new(
        RowId::new(id).expect("id"),
        0,
        RowKind::parse("card").expect("kind"),
        Some(Cell {
            label: "l".into(),
            detail: "d".into(),
            state: "s".into(),
            tags: vec![],
        }),
        Some(Cell {
            label: "l".into(),
            detail: "d".into(),
            state: "s".into(),
            tags: vec![],
        }),
    )
    .expect("行");
    let p = point_all(view);
    let items = card_items(&[row], Phase::After, Variant::B, &p);
    let html = render(&items[0].1);
    let root = &html[..html.find('>').expect("開始タグ")];
    assert!(!root.contains("<b>"), "{root}");
    assert!(!root.contains("<i onclick"), "{root}");
    let payload = unescape(attr_value(root, "data-payload").expect("payload"));
    let t = PointTarget::decode("ui", &payload).expect("復号");
    assert_eq!((t.view(), t.row().as_str()), (view, id));
    assert_eq!(unescape(attr_value(root, "data-row").expect("row")), id);
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&amp;", "&")
}

#[test]
fn payload_が上限を超える行と空の_view_は表示だけになり_行は落ちない() {
    let long_id = "r".repeat(5_000);
    let c = |l: &str| Cell {
        label: l.into(),
        detail: String::new(),
        state: String::new(),
        tags: vec![],
    };
    let row = Row::new(
        RowId::new(&long_id).expect("id"),
        0,
        RowKind::parse("card").expect("kind"),
        Some(c("a")),
        Some(c("a")),
    )
    .expect("行");
    for view in ["v", ""] {
        let p = point_all(view);
        let items = card_items(std::slice::from_ref(&row), Phase::After, Variant::B, &p);
        assert_eq!(items.len(), 1, "{view:?}");
        let html = render(&items[0].1);
        for a in FOUR {
            assert_eq!(html.matches(a).count(), 0, "{view:?} {a}");
        }
    }
    // 空の view は、短い行でも表示だけ。
    let short = Row::new(
        RowId::new("r").expect("id"),
        0,
        RowKind::parse("card").expect("kind"),
        Some(c("a")),
        Some(c("a")),
    )
    .expect("行");
    let p = point_all("");
    let items = card_items(&[short], Phase::After, Variant::B, &p);
    assert_eq!(render(&items[0].1).matches("data-action").count(), 0);
}
