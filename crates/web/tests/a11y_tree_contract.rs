//! アクセシビリティ構造の契約の版 2 が足したツリーの規則（TASK-472。ART-9・ART-13）の負の対照と正の見本。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_web::a11y::{check_no_undefined, Violation};

type Attrs = Vec<(&'static str, &'static str)>;

fn rules(r: Result<(), Vec<Violation>>) -> Vec<&'static str> {
    r.unwrap_err()
        .iter()
        .map(|v| match v {
            Violation::RawHtml { .. } => "raw-html",
            Violation::Element { rule, .. } => rule,
            Violation::Count { .. } => "count",
        })
        .collect()
}

fn item(attrs: Attrs) -> Node {
    el("li", attrs, vec![text("x")])
}

fn tree_of(items: Vec<Node>) -> Node {
    el("ul", vec![("role", "tree")], items)
}

fn good() -> Attrs {
    vec![
        ("role", "treeitem"),
        ("aria-level", "1"),
        ("aria-posinset", "1"),
        ("aria-setsize", "2"),
        ("aria-expanded", "true"),
    ]
}

fn without(name: &str) -> Attrs {
    good().into_iter().filter(|(k, _)| *k != name).collect()
}

fn with(name: &str, value: &'static str) -> Attrs {
    good()
        .into_iter()
        .map(|(k, v)| if k == name { (k, value) } else { (k, v) })
        .collect()
}

#[test]
fn 契約に合う木は通る() {
    let ok = tree_of(vec![
        item(good()),
        item(vec![
            ("role", "treeitem"),
            ("aria-level", "1"),
            ("aria-posinset", "2"),
            ("aria-setsize", "2"),
        ]),
    ]);
    assert_eq!(check_no_undefined(&ok), Ok(()));
    // 入れ子の項目（親の項目の下の項目）も木の中にある。
    let nested = tree_of(vec![el(
        "li",
        good(),
        vec![el(
            "ul",
            vec![],
            vec![item(vec![
                ("role", "treeitem"),
                ("aria-level", "2"),
                ("aria-posinset", "1"),
                ("aria-setsize", "1"),
            ])],
        )],
    )]);
    assert_eq!(check_no_undefined(&nested), Ok(()));
}

#[test]
fn 違反の木は規則の名前つきで失敗する() {
    let big = "99999999999999999999999";
    let cases: Vec<(&str, Node, &str)> = vec![
        (
            "level なし",
            tree_of(vec![item(without("aria-level"))]),
            "treeitem-attribute",
        ),
        (
            "posinset なし",
            tree_of(vec![item(without("aria-posinset"))]),
            "treeitem-attribute",
        ),
        (
            "setsize なし",
            tree_of(vec![item(without("aria-setsize"))]),
            "treeitem-attribute",
        ),
        (
            "posinset が setsize を超える",
            tree_of(vec![item(with("aria-posinset", "3"))]),
            "treeitem-attribute",
        ),
        (
            "0",
            tree_of(vec![item(with("aria-level", "0"))]),
            "treeitem-attribute",
        ),
        (
            "先頭 0",
            tree_of(vec![item(with("aria-level", "01"))]),
            "treeitem-attribute",
        ),
        (
            "数でない",
            tree_of(vec![item(with("aria-setsize", "x"))]),
            "treeitem-attribute",
        ),
        (
            "桁あふれ",
            tree_of(vec![item(with("aria-level", big))]),
            "treeitem-attribute",
        ),
        (
            "閉じた状態",
            tree_of(vec![item(with("aria-expanded", "false"))]),
            "treeitem-expanded",
        ),
        (
            "treeitem の外の level",
            tree_of(vec![el("li", vec![("aria-level", "1")], vec![])]),
            "tree-attribute-placement",
        ),
        (
            "tree の上の expanded",
            el(
                "ul",
                vec![("role", "tree"), ("aria-expanded", "true")],
                vec![],
            ),
            "tree-attribute-placement",
        ),
        (
            "木の外の treeitem",
            el("div", vec![], vec![item(good())]),
            "treeitem-outside-tree",
        ),
        (
            "束縛した木の属性",
            tree_of(vec![item({
                let mut a = good();
                a.push(("data-bind-attr", "aria-level:x"));
                a
            })]),
            "bound-a11y-attribute",
        ),
    ];
    for (name, tree, rule) in cases {
        assert!(rules(check_no_undefined(&tree)).contains(&rule), "{name}");
    }
}

#[test]
fn 違反の文面に値を載せない() {
    let bad = tree_of(vec![item(vec![
        ("role", "treeitem"),
        ("aria-level", "<script>1"),
        ("aria-posinset", "1"),
        ("aria-setsize", "1"),
    ])]);
    let shown = format!("{:?}", check_no_undefined(&bad).unwrap_err());
    assert!(!shown.contains("script"), "{shown}");
}
