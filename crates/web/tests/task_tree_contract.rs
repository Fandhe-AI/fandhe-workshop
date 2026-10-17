//! タスクツリーの変種 (b)（`task_tree`。TASK-474 1/2。ART-1・ART-9・ART-13・A11Y-22）の適合試験。
//!
//! 試料は spec 由来の `tasks.json`（前 5 行・後 6 行）を直接読み、期待値は JSON から手で数えた固定値にする
//! （`view_fixture` の読み込みには依らない）。試験では `unwrap` を使ってよい。

use fandhe_frontend_core::{render, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{Cell, ItemRow, Phase, Row, RowId, RowKind};
use fandhe_workshop_web::task_tree::{
    state_badge, task_items_a, task_items_b, task_tree_list_a, task_tree_list_b, TaskState,
    STATE_GLYPH_TESTID, STATE_TEXT_TESTID,
};

const TASKS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/artifact-view-parts/test-data/sample/tasks.json"
));

fn cell(v: &serde_json::Value) -> Option<Cell> {
    if v.is_null() {
        return None;
    }
    let s = |k: &str| v[k].as_str().unwrap().to_owned();
    Some(Cell {
        label: s("label"),
        detail: s("detail"),
        state: s("state"),
        tags: vec![],
    })
}

fn sample() -> Vec<ItemRow> {
    let root: serde_json::Value = serde_json::from_str(TASKS).unwrap();
    root["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            Row::new(
                RowId::new(r["id"].as_str().unwrap()).unwrap(),
                u8::try_from(r["depth"].as_u64().unwrap()).unwrap(),
                RowKind::parse(r["kind"].as_str().unwrap()).unwrap(),
                cell(&r["before"]),
                cell(&r["after"]),
            )
            .unwrap()
        })
        .collect()
}

fn row(id: &str, depth: u8, before: Option<Cell>, after: Option<Cell>) -> ItemRow {
    Row::new(
        RowId::new(id).unwrap(),
        depth,
        RowKind::parse("task").unwrap(),
        before,
        after,
    )
    .unwrap()
}

fn cell_of(label: &str, state: &str) -> Cell {
    Cell {
        label: label.to_owned(),
        detail: "d".to_owned(),
        state: state.to_owned(),
        tags: vec![],
    }
}

fn html(rows: &[ItemRow], phase: Phase) -> String {
    render(&task_tree_list_b(rows, phase, true).unwrap())
}

fn count(h: &str, needle: &str) -> usize {
    h.matches(needle).count()
}

#[test]
fn 項目数と印の数は手で数えた値と一致する() {
    let rows = sample();
    let before = html(&rows, Phase::Before);
    let after = html(&rows, Phase::After);
    // 出所: tasks.json。前は t1・t1a・t1b・t2・t3 の 5 件、後は t2a が加わり t3 は削除の印つきで残る 6 件。
    assert_eq!(count(&before, "role=\"treeitem\""), 5, "{before}");
    assert_eq!(count(&after, "role=\"treeitem\""), 6, "{after}");
    assert_eq!(count(&before, "data-mark=\"\""), 5, "{before}");
    assert_eq!(count(&after, "data-mark=\"change\""), 3, "{after}");
    assert_eq!(count(&after, "data-mark=\"add\""), 1, "{after}");
    assert_eq!(count(&after, "data-mark=\"remove\""), 1, "{after}");
    assert_eq!(count(&after, "data-mark=\"\""), 1, "{after}");
}

#[test]
fn 木の属性は全項目にあり_親は_role_tree() {
    let rows = sample();
    for (phase, n, expanded) in [(Phase::Before, 5, 1), (Phase::After, 6, 2)] {
        let h = html(&rows, phase);
        assert_eq!(count(&h, "role=\"tree\""), 1, "{h}");
        assert_eq!(count(&h, "aria-level="), n, "{h}");
        assert_eq!(count(&h, "aria-posinset="), n, "{h}");
        assert_eq!(count(&h, "aria-setsize="), n, "{h}");
        assert_eq!(count(&h, "aria-expanded=\"true\""), expanded, "{h}");
        assert_eq!(count(&h, "data-row"), 0, "{h}");
    }
}

#[test]
fn 選ばれていないときと空のときは_role_tree_が付かない() {
    let rows = sample();
    let h = render(&task_tree_list_b(&rows, Phase::Before, false).unwrap());
    assert_eq!(count(&h, "role=\"tree\""), 0, "{h}");
    assert_eq!(count(&h, "treeitem"), 0, "{h}");
    let empty = render(&task_tree_list_b(&[], Phase::Before, true).unwrap());
    assert_eq!(count(&empty, "role=\"tree\""), 0, "{empty}");
}

#[test]
fn 全項目に状態のバッジがあり_記号と文字の両方が空でない() {
    let rows = sample();
    for phase in [Phase::Before, Phase::After] {
        let items = task_items_b(&rows, phase);
        for (_, node) in &items {
            let h = render(node);
            assert_eq!(count(&h, "data-testid=\"state-badge\""), 1, "{h}");
            assert_eq!(
                count(&h, &format!("data-testid=\"{STATE_GLYPH_TESTID}\"></span>")),
                0,
                "{h}"
            );
            assert_eq!(
                count(&h, &format!("data-testid=\"{STATE_TEXT_TESTID}\"></span>")),
                0,
                "{h}"
            );
        }
    }
    for s in TaskState::ALL {
        assert!(!s.glyph().is_empty() && !s.text().is_empty(), "{s:?}");
        assert!(!["+", "-", "~"].contains(&s.glyph()), "{s:?}");
    }
    let mut texts: Vec<_> = TaskState::ALL.iter().map(|s| s.text()).collect();
    texts.sort_unstable();
    texts.dedup();
    assert_eq!(texts.len(), 4);
}

#[test]
fn 状態の文字は前後の_state_と対応する() {
    let rows = sample();
    let before = html(&rows, Phase::Before);
    let after = html(&rows, Phase::After);
    // t1 は前が progress、後が done。前の done は t1a、後の done は t1・t1a・t1b。
    assert_eq!(count(&before, "完了"), 1, "{before}");
    assert_eq!(count(&before, "進行中"), 2, "{before}");
    assert_eq!(count(&before, "未着手"), 2, "{before}");
    assert_eq!(count(&after, "完了"), 3, "{after}");
    assert_eq!(count(&after, "進行中"), 1, "{after}");
    assert_eq!(count(&after, "未着手"), 2, "{after}");
}

#[test]
fn 未知の状態は不明になり_生の値は描画に出ない() {
    for raw in ["", "Done", "unknown", "<b>x</b>"] {
        assert_eq!(TaskState::parse(raw), TaskState::Unknown, "{raw}");
        let rows = vec![row(
            "x",
            0,
            Some(cell_of("l", raw)),
            Some(cell_of("l", raw)),
        )];
        let h = html(&rows, Phase::Before);
        assert_eq!(count(&h, "不明"), 1, "{h}");
        assert_eq!(count(&h, "<b>x</b>"), 0, "{h}");
        if !raw.is_empty() {
            assert_eq!(count(&h, &format!("\"{raw}\"")), 0, "{h}");
        }
    }
}

/// 項目の形（根のタグ・子の並び）を文字列にする。
fn shape(n: &Node) -> String {
    let h = render(n);
    // タグ名の並びだけを取り、属性・文字を落とす。
    let mut out = String::new();
    let mut rest = h.as_str();
    while let Some(i) = rest.find('<') {
        rest = &rest[i + 1..];
        let end = rest.find([' ', '>']).unwrap();
        out.push_str(&rest[..end]);
        out.push(',');
    }
    out
}

#[test]
fn 項目の形は状態と印と前後で変わらない() {
    let rows = sample();
    let before = task_items_b(&rows, Phase::Before);
    let after = task_items_b(&rows, Phase::After);
    for (k, n) in &before {
        let (_, m) = after.iter().find(|(k2, _)| k2 == k).unwrap();
        assert_eq!(shape(n), shape(m), "{k}");
    }
    let mut shapes = vec![];
    for state in ["open", "progress", "done", "zzz"] {
        let rows = vec![
            row("a", 0, Some(cell_of("l", state)), Some(cell_of("l", state))),
            row("b", 0, Some(cell_of("l", state)), Some(cell_of("m", state))),
            row("c", 0, None, Some(cell_of("l", state))),
            row("d", 0, Some(cell_of("l", state)), None),
        ];
        for n in task_items_b(&rows, Phase::After) {
            shapes.push(shape(&n.1));
        }
    }
    shapes.dedup();
    assert_eq!(shapes.len(), 1, "{shapes:?}");
}

#[test]
fn 契約を通り_aria_hidden_と_aria_selected_は出ない() {
    let rows = sample();
    for phase in [Phase::Before, Phase::After] {
        let list = task_tree_list_b(&rows, phase, true).unwrap();
        assert!(check_no_undefined(&list).is_ok());
        let h = render(&list);
        assert_eq!(count(&h, "aria-hidden"), 0, "{h}");
        assert_eq!(count(&h, "aria-selected"), 0, "{h}");
    }
    assert!(check_no_undefined(&state_badge(TaskState::Done)).is_ok());
}

#[test]
fn ラベルと詳細はエスケープされ_要素にならない() {
    let evil = Cell {
        label: "<script>alert(1)</script>".to_owned(),
        detail: "\"><img src=x>".to_owned(),
        state: "open".to_owned(),
        tags: vec![],
    };
    let rows = vec![row("e", 0, Some(evil.clone()), Some(evil))];
    let h = html(&rows, Phase::Before);
    assert_eq!(count(&h, "<script"), 0, "{h}");
    assert_eq!(count(&h, "<img"), 0, "{h}");
    assert_eq!(count(&h, "&lt;script&gt;"), 1, "{h}");
}

#[test]
fn 印の宿主は項目の根で_記号と文字は印と対応する() {
    let rows = sample();
    let items = task_items_b(&rows, Phase::After);
    let h = render(&items.iter().find(|(k, _)| k == "kt2a").unwrap().1);
    assert!(h.starts_with("<li "), "{h}");
    assert_eq!(count(&h, "data-mark=\"add\""), 1, "{h}");
    assert_eq!(count(&h, "data-testid=\"mark-glyph\""), 1, "{h}");
    assert_eq!(count(&h, "data-testid=\"mark-text\""), 1, "{h}");
}

// ---- 変種 (a)（TASK-474 2/2）----

fn html_a(rows: &[ItemRow], phase: Phase) -> String {
    render(&task_tree_list_a(rows, phase, true).unwrap())
}

#[test]
fn a_項目数と印の数は手で数えた値と一致する() {
    let rows = sample();
    let before = html_a(&rows, Phase::Before);
    let after = html_a(&rows, Phase::After);
    // 出所: tasks.json（(b) の試験と同じ）。(a) の項目は `<li>` で数える。
    assert_eq!(count(&before, "<li"), 5, "{before}");
    assert_eq!(count(&after, "<li"), 6, "{after}");
    assert_eq!(count(&before, "data-mark=\"\""), 5, "{before}");
    assert_eq!(count(&after, "data-mark=\"change\""), 3, "{after}");
    assert_eq!(count(&after, "data-mark=\"add\""), 1, "{after}");
    assert_eq!(count(&after, "data-mark=\"remove\""), 1, "{after}");
    assert_eq!(count(&after, "data-mark=\"\""), 1, "{after}");
}

#[test]
fn a_木の意味は無い() {
    let rows = sample();
    for phase in [Phase::Before, Phase::After] {
        let h = html_a(&rows, phase);
        for needle in [
            "role=\"tree\"",
            "role=\"treeitem\"",
            "aria-level",
            "aria-posinset",
            "aria-setsize",
            "aria-expanded",
        ] {
            assert_eq!(count(&h, needle), 0, "{needle} {h}");
        }
    }
}

#[test]
fn a_完了の項目だけチェック済みで_未知の状態はチェックなし() {
    let rows = sample();
    let before = html_a(&rows, Phase::Before);
    let after = html_a(&rows, Phase::After);
    assert_eq!(count(&before, "data-testid=\"task-check\""), 5, "{before}");
    assert_eq!(count(&after, "data-testid=\"task-check\""), 6, "{after}");
    // 前の done は t1a だけ。後の done は t1・t1a・t1b（削除の t3 は前の中身 open を出す）。
    assert_eq!(count(&before, "data-checked=\"true\""), 1, "{before}");
    assert_eq!(count(&after, "data-checked=\"true\""), 3, "{after}");
    assert_eq!(count(&after, "\u{2713}"), 3, "{after}");
    for raw in ["", "Done", "<b>x</b>"] {
        let rows = vec![row(
            "x",
            0,
            Some(cell_of("l", raw)),
            Some(cell_of("l", raw)),
        )];
        let h = html_a(&rows, Phase::Before);
        assert_eq!(count(&h, "data-checked=\"true\""), 0, "{h}");
        assert_eq!(count(&h, "\u{2713}"), 0, "{h}");
        assert_eq!(count(&h, "不明"), 1, "{h}");
        assert_eq!(count(&h, "<b>x</b>"), 0, "{h}");
    }
}

#[test]
fn a_状態のバッジは全項目にあり_文字の数は_b_と同じ() {
    let rows = sample();
    let before = html_a(&rows, Phase::Before);
    let after = html_a(&rows, Phase::After);
    assert_eq!(count(&before, "data-testid=\"state-badge\""), 5, "{before}");
    assert_eq!(count(&after, "data-testid=\"state-badge\""), 6, "{after}");
    for h in [&before, &after] {
        for t in [STATE_GLYPH_TESTID, STATE_TEXT_TESTID] {
            assert_eq!(count(h, &format!("data-testid=\"{t}\"></span>")), 0, "{h}");
        }
    }
    assert_eq!(count(&before, "完了"), 1, "{before}");
    assert_eq!(count(&before, "進行中"), 2, "{before}");
    assert_eq!(count(&before, "未着手"), 2, "{before}");
    assert_eq!(count(&after, "完了"), 3, "{after}");
    assert_eq!(count(&after, "進行中"), 1, "{after}");
    assert_eq!(count(&after, "未着手"), 2, "{after}");
}

#[test]
fn a_項目の形は状態と印と前後で変わらない() {
    let rows = sample();
    let before = task_items_a(&rows, Phase::Before);
    let after = task_items_a(&rows, Phase::After);
    for (k, n) in &before {
        let (_, m) = after.iter().find(|(k2, _)| k2 == k).unwrap();
        assert_eq!(shape(n), shape(m), "{k}");
    }
    let mut shapes = vec![];
    for state in ["open", "progress", "done", "zzz"] {
        let rows = vec![
            row("a", 0, Some(cell_of("l", state)), Some(cell_of("l", state))),
            row("b", 0, Some(cell_of("l", state)), Some(cell_of("m", state))),
            row("c", 0, None, Some(cell_of("l", state))),
            row("d", 0, Some(cell_of("l", state)), None),
        ];
        for n in task_items_a(&rows, Phase::After) {
            shapes.push(shape(&n.1));
        }
    }
    shapes.dedup();
    assert_eq!(shapes.len(), 1, "{shapes:?}");
}

#[test]
fn a_印の宿主は中の_div_で_深さのクラスは深さと一致する() {
    let rows = sample();
    let items = task_items_a(&rows, Phase::After);
    let h = render(&items.iter().find(|(k, _)| k == "kt1a").unwrap().1);
    assert!(h.starts_with("<li><div "), "{h}");
    assert_eq!(count(&h, "data-mark="), 1, "{h}");
    assert_eq!(count(&h, "class=\"d1\""), 1, "{h}");
    let h = render(&items.iter().find(|(k, _)| k == "kt1").unwrap().1);
    assert_eq!(count(&h, "class=\"d0\""), 1, "{h}");
}

#[test]
fn a_契約を通りエスケープされ_チェックの見た目は_app_css_にある() {
    let rows = sample();
    for phase in [Phase::Before, Phase::After] {
        let list = task_tree_list_a(&rows, phase, true).unwrap();
        assert!(check_no_undefined(&list).is_ok());
        let h = render(&list);
        assert_eq!(count(&h, "aria-hidden"), 0, "{h}");
        assert_eq!(count(&h, "aria-selected"), 0, "{h}");
    }
    let evil = Cell {
        label: "<script>alert(1)</script>".to_owned(),
        detail: "\"><img src=x>".to_owned(),
        state: "done".to_owned(),
        tags: vec![],
    };
    let rows = vec![row("e", 0, Some(evil.clone()), Some(evil))];
    let h = html_a(&rows, Phase::Before);
    assert_eq!(count(&h, "<script"), 0, "{h}");
    assert_eq!(count(&h, "<img"), 0, "{h}");
    assert_eq!(count(&h, "&lt;script&gt;"), 1, "{h}");
    let css = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css"));
    assert!(css.contains(".tk-check"));
}
