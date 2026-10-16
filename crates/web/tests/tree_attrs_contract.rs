//! ツリーの位置の計算と木の属性（`tree_attrs`・`artifact_view::tree_view_list`。TASK-472。ART-9・ART-13）の適合試験。
//!
//! 位置の計算は、試作の正解値・端の場合・素朴な O(n^2) の数え方との全数照合・性質で確かめる。
//! 組み合わせの試験は、ファイルツリーとタスクツリーの試料を `Node` の木にして、受入基準の 4 点
//! （親が `role=tree`・全項目が `role=treeitem`・全項目が 3 属性・隠れた項目が 0 件）を数える。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{
    list_attrs, tree_list_attrs, tree_view_list, Cell, ItemRow, Phase, Row, RowId, RowKind,
    Variant, ViewKind, ROW_ATTR,
};
use fandhe_workshop_web::tree_attrs::{shown_tree_attrs, tree_positions, TreePos};

fn pos(level: usize, posinset: usize, setsize: usize, has_children: bool) -> TreePos {
    TreePos {
        level,
        posinset,
        setsize,
        has_children,
    }
}

/// 独立の答え。親は直前の、より浅い行で、兄弟は同じ親を持つ行どうし。O(n^2) で素朴に数える。
fn naive(depths: &[u8]) -> Vec<TreePos> {
    let parent_of = |i: usize| (0..i).rev().find(|&j| depths[j] < depths[i]);
    let level_of = |mut i: usize| {
        let mut n = 1;
        while let Some(p) = parent_of(i) {
            n += 1;
            i = p;
        }
        n
    };
    (0..depths.len())
        .map(|i| {
            let siblings: Vec<usize> = (0..depths.len())
                .filter(|&j| parent_of(j) == parent_of(i))
                .collect();
            TreePos {
                level: level_of(i),
                posinset: siblings.iter().position(|&j| j == i).unwrap() + 1,
                setsize: siblings.len(),
                has_children: depths.get(i + 1).is_some_and(|&d| d > depths[i]),
            }
        })
        .collect()
}

#[test]
fn 試作の正解値と一致する() {
    assert_eq!(
        tree_positions(&[0, 1, 1, 0, 1]),
        vec![
            pos(1, 1, 2, true),
            pos(2, 1, 2, false),
            pos(2, 2, 2, false),
            pos(1, 2, 2, true),
            pos(2, 1, 1, false),
        ]
    );
}

#[test]
fn 端の場合() {
    assert_eq!(tree_positions(&[]), vec![]);
    assert_eq!(tree_positions(&[0]), vec![pos(1, 1, 1, false)]);
    // 先頭の深さが 0 でなくても level は 1。
    assert_eq!(tree_positions(&[3]), vec![pos(1, 1, 1, false)]);
    // 深さが飛ぶ並びでも、level は祖先の数 + 1。
    assert_eq!(
        tree_positions(&[0, 2]),
        vec![pos(1, 1, 1, true), pos(2, 1, 1, false)]
    );
    // 浅くなる側が最初の行より浅い場合は、新しい根の兄弟になる。
    assert_eq!(
        tree_positions(&[2, 0]),
        vec![pos(1, 1, 2, false), pos(1, 2, 2, false)]
    );
}

#[test]
fn 長い並びでも_panic_しない() {
    for pattern in [0u8, 255] {
        let flat = vec![pattern; 4_096];
        let r = tree_positions(&flat);
        assert_eq!(r.len(), 4_096);
        assert_eq!(r[4_095], pos(1, 4_096, 4_096, false));
    }
    let chain: Vec<u8> = (0..=255).collect();
    let r = tree_positions(&chain);
    assert_eq!(r[255].level, 256);
    let zigzag: Vec<u8> = (0..4_096).map(|i| u8::from(i % 2 == 1)).collect();
    let r = tree_positions(&zigzag);
    assert_eq!(r.len(), 4_096);
    assert_eq!(r[0].setsize, 2_048);
}

#[test]
fn 全数照合_素朴な数え方と一致する() {
    let mut checked = 0usize;
    for len in 0..=6usize {
        let total = 4usize.pow(u32::try_from(len).unwrap());
        for code in 0..total {
            let depths: Vec<u8> = (0..len)
                .map(|i| u8::try_from((code >> (2 * i)) & 3).unwrap())
                .collect();
            assert_eq!(tree_positions(&depths), naive(&depths), "{depths:?}");
            checked += 1;
        }
    }
    assert_eq!(checked, (0..=6u32).map(|l| 4usize.pow(l)).sum::<usize>());
}

#[test]
fn 性質_位置は範囲に収まり_兄弟の数は等しい() {
    for len in 1..=6usize {
        let total = 4usize.pow(u32::try_from(len).unwrap());
        for code in 0..total {
            let depths: Vec<u8> = (0..len)
                .map(|i| u8::try_from((code >> (2 * i)) & 3).unwrap())
                .collect();
            let r = tree_positions(&depths);
            for p in &r {
                assert!(1 <= p.posinset && p.posinset <= p.setsize, "{depths:?}");
                assert!(p.level >= 1);
            }
            // 兄弟の位置は 1 から連続し、数は setsize と一致する。
            let tidy = depths[0] == 0 && depths.windows(2).all(|w| w[1] <= w[0] + 1);
            if tidy {
                for (d, p) in depths.iter().zip(&r) {
                    assert_eq!(p.level, usize::from(*d) + 1, "{depths:?}");
                }
            }
            // 子を持つのは、次の行がより深い行だけ。
            for (i, p) in r.iter().enumerate() {
                assert_eq!(
                    p.has_children,
                    depths.get(i + 1).is_some_and(|&n| n > depths[i])
                );
            }
        }
    }
}

fn cell(s: &str) -> Cell {
    Cell {
        label: s.to_owned(),
        ..Cell::default()
    }
}

fn row(id: &str, depth: u8, kind: RowKind, before: Option<&str>, after: Option<&str>) -> ItemRow {
    Row::new(
        RowId::new(id).unwrap(),
        depth,
        kind,
        before.map(cell),
        after.map(cell),
    )
    .unwrap()
}

/// ファイルツリーの試料。追加 1・削除 1 を含み、前の状態では 5 行、後の状態では 6 行が出る。
fn file_rows() -> Vec<ItemRow> {
    vec![
        row("d-a", 0, RowKind::Dir, Some("a"), Some("a")),
        row("f-a1", 1, RowKind::File, Some("a1"), Some("a1")),
        row("d-a2", 1, RowKind::Dir, Some("a2"), Some("a2")),
        row("f-a2x", 2, RowKind::File, None, Some("a2x")),
        row("f-b", 0, RowKind::File, Some("b"), Some("b")),
        row("f-old", 0, RowKind::File, Some("old"), None),
    ]
}

/// タスクツリーの試料。
fn task_rows() -> Vec<ItemRow> {
    vec![
        row("t-1", 0, RowKind::Task, Some("1"), Some("1")),
        row("t-11", 1, RowKind::Task, Some("11"), Some("11")),
        row("t-12", 1, RowKind::Task, Some("12"), Some("12")),
        row("t-2", 0, RowKind::Task, Some("2"), Some("2")),
        row("t-21", 1, RowKind::Task, Some("21"), Some("21")),
    ]
}

#[test]
fn 前後で表示行と兄弟の数が変わる() {
    let rows = file_rows();
    let after = shown_tree_attrs(&rows, Phase::After);
    assert_eq!(after.iter().flatten().count(), 6);
    let before = shown_tree_attrs(&rows, Phase::Before);
    assert_eq!(before.iter().flatten().count(), 5);
    // 追加の行は前では出ない。
    assert!(before[3].is_none());
    let pair = |a: &Option<fandhe_workshop_web::tree_attrs::TreeItemAttrs>, key: &str| {
        a.as_ref()
            .unwrap()
            .pairs()
            .into_iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.to_owned())
    };
    // 根の兄弟は、後で 3（a・b・old）、前で 3（a・b・old）。追加の行は d-a2 の子なので根は変わらない。
    assert_eq!(pair(&after[0], "aria-setsize").as_deref(), Some("3"));
    // d-a2 は前では子を持たず、後では持つ（`aria-expanded` は後だけ）。
    assert_eq!(pair(&before[2], "aria-expanded"), None);
    assert_eq!(pair(&after[2], "aria-expanded").as_deref(), Some("true"));
    // 削除された行は、後でも印つきで残って数に入る。
    assert_eq!(pair(&after[5], "aria-posinset").as_deref(), Some("3"));
    // 前では削除の行が表示に残るため、行 f-old の位置も数に入る。
    assert_eq!(pair(&before[5], "aria-posinset").as_deref(), Some("3"));
}

#[test]
fn 隠れる行を数えに入れない() {
    // 前の状態で出ない行（追加）が、兄弟の数に入らないこと。
    let rows = vec![
        row("a", 0, RowKind::Item, Some("a"), Some("a")),
        row("b", 0, RowKind::Item, None, Some("b")),
        row("c", 0, RowKind::Item, Some("c"), Some("c")),
    ];
    let before = shown_tree_attrs(&rows, Phase::Before);
    let sizes: Vec<_> = before
        .iter()
        .map(|a| {
            a.as_ref().map(|a| {
                a.pairs()
                    .into_iter()
                    .find(|(k, _)| *k == "aria-setsize")
                    .map(|(_, v)| v.to_owned())
            })
        })
        .collect();
    assert_eq!(
        sizes,
        vec![Some(Some("2".to_owned())), None, Some(Some("2".to_owned()))]
    );
}

/// 試料を `Node` の木にする。項目の属性は `view()` の側で直接書く（F-2）。
fn tree_node(kind: ViewKind, variant: Variant, rows: &[ItemRow], phase: Phase) -> Node {
    let attrs = shown_tree_attrs(rows, phase);
    let items: Vec<(String, Node)> = rows
        .iter()
        .zip(attrs)
        .filter_map(|(r, t)| {
            let shown = r.shown(phase)?;
            let mut a: Vec<(&str, &str)> = vec![(ROW_ATTR, r.id().as_str())];
            let owned = t.filter(|_| variant == Variant::B);
            if let Some(t) = &owned {
                a.extend(t.pairs());
            }
            Some((
                format!("k{}", r.id().as_str()),
                el("li", a, vec![text(&shown.content.label)]),
            ))
        })
        .collect();
    tree_view_list("ul", kind, variant, true, items).unwrap()
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn children(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

/// 受入基準の 4 点と、位置の値が `tree_positions` の答えと一致することを数える。
fn assert_open_tree(root: &Node, rows: &[ItemRow]) {
    assert_eq!(attr(root, "role"), Some("tree"));
    let items = children(root);
    let shown: Vec<&ItemRow> = rows
        .iter()
        .filter(|r| r.shown(Phase::After).is_some())
        .collect();
    assert_eq!(items.len(), shown.len());
    let depths: Vec<u8> = shown.iter().map(|r| r.depth()).collect();
    let want = tree_positions(&depths);
    for (item, w) in items.iter().zip(&want) {
        assert_eq!(attr(item, "role"), Some("treeitem"));
        assert_eq!(attr(item, "aria-level"), Some(w.level.to_string().as_str()));
        assert_eq!(
            attr(item, "aria-posinset"),
            Some(w.posinset.to_string().as_str())
        );
        assert_eq!(
            attr(item, "aria-setsize"),
            Some(w.setsize.to_string().as_str())
        );
        assert_eq!(attr(item, "hidden"), None);
        assert_eq!(
            attr(item, "aria-expanded"),
            if w.has_children { Some("true") } else { None }
        );
    }
    assert_eq!(check_no_undefined(root), Ok(()));
}

#[test]
fn 変種_b_のファイルツリーとタスクツリーは全項目が開いた木になる() {
    let rows = file_rows();
    assert_open_tree(
        &tree_node(ViewKind::FileTree, Variant::B, &rows, Phase::After),
        &rows,
    );
    let rows = task_rows();
    assert_open_tree(
        &tree_node(ViewKind::TaskTree, Variant::B, &rows, Phase::After),
        &rows,
    );
}

#[test]
fn 変種_a_は_role_を付けない対照() {
    for (kind, rows) in [
        (ViewKind::FileTree, file_rows()),
        (ViewKind::TaskTree, task_rows()),
    ] {
        let root = tree_node(kind, Variant::A, &rows, Phase::After);
        assert_eq!(attr(&root, "role"), None);
        let html = fandhe_frontend_core::render(&root);
        for needle in ["role=", "aria-level", "aria-posinset", "aria-setsize"] {
            assert!(!html.contains(needle), "{needle}: {html}");
        }
        assert_eq!(check_no_undefined(&root), Ok(()));
    }
}

#[test]
fn 親の_role_は_4_条件がそろったときだけ付く() {
    let role = |kind, variant, selected, has_items| {
        tree_list_attrs(kind, variant, selected, has_items)
            .iter()
            .any(|(k, v)| *k == "role" && *v == "tree")
    };
    for kind in ViewKind::ALL {
        for variant in Variant::ALL {
            for selected in [false, true] {
                for has_items in [false, true] {
                    let want = kind.is_tree() && variant == Variant::B && selected && has_items;
                    assert_eq!(
                        role(kind, variant, selected, has_items),
                        want,
                        "{kind:?} {variant:?} {selected} {has_items}"
                    );
                }
            }
        }
    }
    // 既存の `list_attrs` の出力は変えない（role を足さない）。
    for kind in ViewKind::ALL {
        for selected in [false, true] {
            assert!(list_attrs(kind, selected).iter().all(|(k, _)| *k != "role"));
        }
    }
}

#[test]
fn 項目が_0_件と非選択では_role_を付けない() {
    let empty = tree_view_list("ul", ViewKind::FileTree, Variant::B, true, Vec::new()).unwrap();
    assert_eq!(attr(&empty, "role"), None);
    let items = vec![("k".to_owned(), el("li", vec![], vec![text("x")]))];
    let hidden = tree_view_list("ul", ViewKind::FileTree, Variant::B, false, items).unwrap();
    assert_eq!(attr(&hidden, "role"), None);
    assert_eq!(attr(&hidden, "hidden"), Some(""));
    assert!(children(&hidden).is_empty());
    let other = vec![("k".to_owned(), el("li", vec![], vec![text("x")]))];
    let card = tree_view_list("ul", ViewKind::CardList, Variant::B, true, other).unwrap();
    assert_eq!(attr(&card, "role"), None);
}

#[test]
fn 教材の文字列は木の属性に入らない() {
    let rows = vec![
        row("<b>\"x\"&", 0, RowKind::Dir, Some("<i>"), Some("<i>")),
        row("<u>", 1, RowKind::File, Some("<s>"), Some("<s>")),
    ];
    let root = tree_node(ViewKind::FileTree, Variant::B, &rows, Phase::After);
    let html = fandhe_frontend_core::render(&root);
    assert!(!html.contains("<b>"), "{html}");
    assert!(!html.contains("<i>"), "{html}");
    for item in children(&root) {
        for name in ["aria-level", "aria-posinset", "aria-setsize"] {
            let v = attr(item, name).unwrap();
            assert!(v.bytes().all(|b| b.is_ascii_digit()), "{v}");
        }
    }
}
