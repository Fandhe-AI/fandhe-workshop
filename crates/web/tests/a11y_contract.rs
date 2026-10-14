//! アクセシビリティ構造の契約（`a11y`、TASK-366）の適合試験。値の固定・正の見本・負の見本・キー規則を確かめる。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_web::a11y::*;
use std::collections::HashSet;

fn tab(t: PaneTab, selected: bool) -> Node {
    el(
        "button",
        vec![
            ("role", "tab"),
            (TESTID_ATTR, TAB_TESTID),
            ("id", t.tab_id()),
            ("aria-controls", t.panel_id()),
            ("aria-selected", if selected { "true" } else { "false" }),
            ("tabindex", if selected { "0" } else { "-1" }),
        ],
        vec![text(t.label())],
    )
}

fn panel(t: PaneTab, children: Vec<Node>) -> Node {
    el(
        "section",
        vec![
            ("role", "tabpanel"),
            ("id", t.panel_id()),
            ("aria-labelledby", t.tab_id()),
        ],
        children,
    )
}

fn live(l: LiveRegion) -> Node {
    let mut attrs = vec![(TESTID_ATTR, l.testid())];
    attrs.extend(LIVE_REGION_ATTRS);
    el("div", attrs, vec![text("x")])
}

fn notice(n: StatusNotice) -> Node {
    el(
        "div",
        vec![("role", "status"), (TESTID_ATTR, n.testid())],
        vec![],
    )
}

/// 契約どおりの最小の画面。
fn conforming_view(selected: PaneTab) -> Node {
    let tabs: Vec<Node> = PaneTab::ALL
        .iter()
        .map(|t| tab(*t, *t == selected))
        .collect();
    el(
        "main",
        vec![],
        vec![
            el(
                "section",
                vec![("aria-label", Region::Artifacts.aria_label())],
                vec![notice(StatusNotice::View)],
            ),
            el(
                "section",
                vec![("aria-label", Region::GuideAndLog.aria_label())],
                vec![
                    notice(StatusNotice::Log),
                    el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], vec![]),
                ],
            ),
            el(
                "section",
                vec![("aria-label", Region::CommandInput.aria_label())],
                vec![notice(StatusNotice::Input)],
            ),
            live(LiveRegion::Response),
            live(LiveRegion::Left),
            el(
                "div",
                vec![
                    ("role", "tablist"),
                    (TESTID_ATTR, TABLIST_TESTID),
                    ("aria-label", TABLIST_LABEL),
                ],
                tabs,
            ),
            panel(PaneTab::Ops, vec![]),
            panel(PaneTab::Artifacts, vec![]),
        ],
    )
}

/// 根の子を差し替えた見本を作る。
fn with_children(f: impl FnOnce(&mut Vec<Node>)) -> Node {
    match conforming_view(PaneTab::Ops) {
        Node::Element {
            tag,
            attrs,
            mut children,
        } => {
            f(&mut children);
            Node::Element {
                tag,
                attrs,
                children,
            }
        }
        other => other,
    }
}

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

fn elem(attrs: Vec<(&str, &str)>, children: Vec<Node>) -> Node {
    el("div", attrs, children)
}

fn unique<T: std::hash::Hash + Eq>(it: impl Iterator<Item = T>) -> usize {
    it.collect::<HashSet<_>>().len()
}

#[test]
fn constants_match_spec() {
    assert_eq!(A11Y_CONTRACT_VERSION, 1);
    assert_eq!(TESTID_ATTR, "data-testid");
    assert_eq!(Region::ALL.len(), 3);
    assert_eq!(unique(Region::ALL.iter().map(|r| r.aria_label())), 3);
    let labels: Vec<_> = Region::ALL.iter().map(|r| r.aria_label()).collect();
    assert_eq!(labels, ["成果物", "案内とログ", "コマンドの入力"]);
    let lives: Vec<_> = LiveRegion::ALL.iter().map(|l| l.testid()).collect();
    assert_eq!(lives, ["live-response", "live-left"]);
    assert_eq!(
        LIVE_REGION_ATTRS,
        [("aria-live", "polite"), ("aria-atomic", "true")]
    );
    let notices: Vec<_> = StatusNotice::ALL.iter().map(|n| n.testid()).collect();
    assert_eq!(notices, ["view-notice", "input-notice", "log-notice"]);
    assert_eq!(LOG_LIST_TESTID, "log-list");
    assert_eq!(TABLIST_TESTID, "pane-tabs");
    assert_eq!(TABLIST_LABEL, "表示する領域");
    assert_eq!(TAB_TESTID, "pane-tab");
    assert_eq!(PaneTab::ALL, [PaneTab::Ops, PaneTab::Artifacts]);
    assert_eq!(PaneTab::Ops.id(), "ops");
    assert_eq!(PaneTab::Artifacts.label(), "成果物");
    for t in PaneTab::ALL {
        assert_eq!(t.tab_id(), format!("pane-tab-{}", t.id()));
        assert_eq!(t.panel_id(), format!("pane-{}", t.id()));
    }
    let mut want: Vec<&str> = Region::ALL.iter().map(|r| r.aria_label()).collect();
    want.push(TABLIST_LABEL);
    assert_eq!(ARIA_LABEL_VALUES.to_vec(), want);
    assert_eq!(ROLE_VALUES, ["status", "tablist", "tab", "tabpanel"]);
    assert!(A11Y_ATTRIBUTE_NAMES.contains(&"aria-live"));
    assert!(!A11Y_ATTRIBUTE_NAMES.contains(&"aria-hidden"));
}

#[test]
fn conforming_views_pass() {
    for t in PaneTab::ALL {
        let v = conforming_view(t);
        assert_eq!(check_complete(&v), Ok(()));
        assert_eq!(check_no_undefined(&v), Ok(()));
    }
}

#[test]
fn live_attributes_are_written_directly() {
    let html = render(&conforming_view(PaneTab::Ops));
    assert!(html.contains("aria-live=\"polite\""), "{html}");
    assert!(html.contains("aria-atomic=\"true\""), "{html}");
}

#[test]
fn unrelated_testid_is_out_of_scope() {
    let v = elem(vec![(TESTID_ATTR, "hello-root")], vec![text("x")]);
    assert_eq!(check_no_undefined(&v), Ok(()));
}

#[test]
fn undefined_items_fail() {
    let cases: Vec<(&str, Node, &str)> = vec![
        (
            "aria-hidden",
            elem(vec![("aria-hidden", "true")], vec![]),
            "undefined-attribute",
        ),
        (
            "role log",
            elem(vec![("role", "log")], vec![]),
            "undefined-role",
        ),
        (
            "role list",
            elem(vec![("role", "list")], vec![]),
            "undefined-role",
        ),
        (
            "label",
            elem(vec![("aria-label", "その他")], vec![]),
            "undefined-aria-label",
        ),
        (
            "assertive",
            elem(
                vec![
                    (TESTID_ATTR, "live-response"),
                    ("aria-live", "assertive"),
                    ("aria-atomic", "true"),
                ],
                vec![],
            ),
            "live-attribute",
        ),
        (
            "missing atomic",
            elem(
                vec![(TESTID_ATTR, "live-left"), ("aria-live", "polite")],
                vec![],
            ),
            "live-attribute",
        ),
        (
            "live on other",
            elem(vec![("aria-live", "polite")], vec![]),
            "live-attribute-on-non-live",
        ),
        (
            "live-foo",
            elem(vec![(TESTID_ATTR, "live-foo")], vec![]),
            "undefined-live-id",
        ),
        (
            "status on other",
            elem(vec![("role", "status"), (TESTID_ATTR, "foo")], vec![]),
            "status-on-non-notice",
        ),
        (
            "notice without role",
            elem(vec![(TESTID_ATTR, "view-notice")], vec![]),
            "notice-role",
        ),
        (
            "bind live",
            elem(vec![("data-bind-attr", "aria-live:x")], vec![]),
            "bound-a11y-attribute",
        ),
        (
            "bind selected",
            elem(vec![("data-bind-attr", "text:a aria-selected:x")], vec![]),
            "bound-a11y-attribute",
        ),
        (
            "raw html",
            elem(vec![], vec![Node::RawHtml("<b>".into())]),
            "raw-html",
        ),
        (
            "duplicate",
            elem(vec![("id", "a"), ("id", "b")], vec![]),
            "duplicate-attribute",
        ),
    ];
    for (name, view, rule) in cases {
        assert!(
            rules(check_no_undefined(&view)).contains(&rule),
            "{name}: expected {rule}"
        );
    }
}

#[test]
fn live_region_placement_is_checked() {
    let in_panel = with_children(|c| {
        c.pop();
        c.pop();
        c.push(panel(PaneTab::Ops, vec![live(LiveRegion::Response)]));
        c.push(panel(PaneTab::Artifacts, vec![]));
    });
    assert!(rules(check_no_undefined(&in_panel)).contains(&"live-in-tabpanel"));

    let mut attrs = vec![(TESTID_ATTR, "live-left")];
    attrs.extend(LIVE_REGION_ATTRS);
    let with_child = elem(attrs, vec![el("p", vec![], vec![])]);
    assert!(rules(check_no_undefined(&with_child)).contains(&"live-children"));

    let mut attrs = vec![(TESTID_ATTR, "live-left")];
    attrs.extend(LIVE_REGION_ATTRS);
    let log_in_live = elem(
        attrs,
        vec![el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], vec![])],
    );
    assert!(rules(check_no_undefined(&log_in_live)).contains(&"log-list-in-live"));
}

#[test]
fn tab_structure_is_checked() {
    let no_label = elem(
        vec![("role", "tablist"), (TESTID_ATTR, TABLIST_TESTID)],
        vec![],
    );
    assert!(rules(check_no_undefined(&no_label)).contains(&"tablist-attribute"));

    let list = |tabs: Vec<Node>| {
        elem(
            vec![
                ("role", "tablist"),
                (TESTID_ATTR, TABLIST_TESTID),
                ("aria-label", TABLIST_LABEL),
            ],
            tabs,
        )
    };
    let bad_controls = el(
        "button",
        vec![
            ("role", "tab"),
            (TESTID_ATTR, TAB_TESTID),
            ("id", "pane-tab-ops"),
            ("aria-controls", "pane-artifacts"),
            ("aria-selected", "true"),
            ("tabindex", "0"),
        ],
        vec![],
    );
    assert!(rules(check_no_undefined(&list(vec![bad_controls]))).contains(&"tab-controls"));

    let bad_tabindex = el(
        "button",
        vec![
            ("role", "tab"),
            (TESTID_ATTR, TAB_TESTID),
            ("id", "pane-tab-ops"),
            ("aria-controls", "pane-ops"),
            ("aria-selected", "false"),
            ("tabindex", "0"),
        ],
        vec![],
    );
    assert!(rules(check_no_undefined(&list(vec![bad_tabindex]))).contains(&"tab-tabindex"));

    assert!(rules(check_no_undefined(&tab(PaneTab::Ops, true))).contains(&"tab-outside-tablist"));

    let bad_panel = elem(
        vec![
            ("role", "tabpanel"),
            ("id", "pane-ops"),
            ("aria-labelledby", "pane-tab-artifacts"),
        ],
        vec![],
    );
    assert!(rules(check_no_undefined(&bad_panel)).contains(&"tabpanel-labelledby"));
}

#[test]
fn selected_tab_count_is_checked() {
    let swap = |sel: [bool; 2]| {
        with_children(|c| {
            let tabs = PaneTab::ALL
                .iter()
                .zip(sel)
                .map(|(t, s)| tab(*t, s))
                .collect();
            c[5] = elem(
                vec![
                    ("role", "tablist"),
                    (TESTID_ATTR, TABLIST_TESTID),
                    ("aria-label", TABLIST_LABEL),
                ],
                tabs,
            );
        })
    };
    assert_eq!(check_complete(&swap([false, true])), Ok(()));
    assert!(rules(check_complete(&swap([false, false]))).contains(&"count"));
    assert!(rules(check_complete(&swap([true, true]))).contains(&"count"));
}

#[test]
fn complete_checks_counts() {
    let dup_notice = with_children(|c| c.push(notice(StatusNotice::View)));
    assert_eq!(check_no_undefined(&dup_notice), Ok(()));
    assert!(rules(check_complete(&dup_notice)).contains(&"count"));

    let no_region = with_children(|c| {
        c.remove(0);
    });
    assert_eq!(check_no_undefined(&no_region), Ok(()));
    assert!(rules(check_complete(&no_region)).contains(&"count"));
}

#[test]
fn violation_display_is_english_and_carries_position() {
    let v = elem(vec![("aria-hidden", "true")], vec![]);
    let msg = check_no_undefined(&v).unwrap_err()[0].to_string();
    assert!(msg.contains("undefined-attribute"), "{msg}");
    assert!(msg.contains("aria-hidden"), "{msg}");
}

#[test]
fn deep_tree_does_not_overflow() {
    let mut n = elem(vec![], vec![]);
    for _ in 0..100_000 {
        n = elem(vec![], vec![n]);
    }
    assert_eq!(check_no_undefined(&n), Ok(()));
    // Drop の再帰で落ちないよう、解放は線形に行う。
    let mut cur = n;
    while let Node::Element { mut children, .. } = cur {
        match children.pop() {
            Some(c) => cur = c,
            None => break,
        }
    }
}

#[test]
fn tab_keys() {
    let m = KeyModifiers::default();
    let t = |k, cur| tab_key_target(k, cur, 2, m);
    assert_eq!(t(TabKey::ArrowRight, 0), Some(1));
    assert_eq!(t(TabKey::ArrowRight, 1), Some(0));
    assert_eq!(t(TabKey::ArrowLeft, 0), Some(1));
    assert_eq!(t(TabKey::ArrowLeft, 1), Some(0));
    assert_eq!(t(TabKey::Home, 1), Some(0));
    assert_eq!(t(TabKey::End, 0), Some(1));
    assert_eq!(tab_key_target(TabKey::End, 0, 0, m), None);
    assert_eq!(tab_key_target(TabKey::Home, 2, 2, m), None);
    for mods in [
        KeyModifiers { alt: true, ..m },
        KeyModifiers { ctrl: true, ..m },
        KeyModifiers { meta: true, ..m },
        KeyModifiers {
            composing: true,
            ..m
        },
    ] {
        assert_eq!(tab_key_target(TabKey::ArrowRight, 0, 2, mods), None);
    }
    for k in [
        TabKey::ArrowLeft,
        TabKey::ArrowRight,
        TabKey::Home,
        TabKey::End,
    ] {
        assert_eq!(TabKey::from_key(k.as_str()), Some(k));
    }
    assert_eq!(TabKey::from_key("ArrowUp"), None);
    assert_eq!(TabKey::from_key("Tab"), None);
}
