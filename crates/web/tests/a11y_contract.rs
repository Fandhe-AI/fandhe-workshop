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
    let marks: Vec<_> = ChangeMark::ALL
        .iter()
        .map(|m| (m.data_value(), m.glyph(), m.text()))
        .collect();
    assert_eq!(
        marks,
        [
            ("add", "+", "【追加】"),
            ("remove", "-", "【削除】"),
            ("change", "~", "【変更】")
        ]
    );
    assert_eq!(unique(ChangeMark::ALL.iter().map(|m| m.data_value())), 3);
    assert_eq!(unique(ChangeMark::ALL.iter().map(|m| m.glyph())), 3);
    assert_eq!(unique(ChangeMark::ALL.iter().map(|m| m.text())), 3);
    assert_eq!(MARK_ATTR, "data-mark");
    assert_eq!(MARK_GLYPH_TESTID, "mark-glyph");
    assert_eq!(MARK_TEXT_TESTID, "mark-text");
    assert_eq!(MARK_NONE_VALUE, "");
    assert_eq!(MIN_TARGET_SIZE_CSS_PX, 24);
    assert_eq!(
        TARGET_SIZE_SELECTOR,
        "button, select, input:not([type=\"hidden\"]), [data-action]"
    );
    assert_eq!(LIVE_EXCLUDED_LINE_KINDS, ["received", "progress"]);
}

/// 印の行の見本。`m` が `None` なら印なし（記号・文字とも空）。
fn mark_row(m: Option<ChangeMark>) -> Node {
    let (value, glyph, label) = match m {
        Some(m) => (m.data_value(), m.glyph(), m.text()),
        None => (MARK_NONE_VALUE, "", ""),
    };
    el(
        "li",
        vec![(MARK_ATTR, value)],
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, MARK_GLYPH_TESTID)],
                vec![text(glyph)],
            ),
            el(
                "span",
                vec![(TESTID_ATTR, MARK_TEXT_TESTID)],
                vec![text(label)],
            ),
            text("line"),
        ],
    )
}

fn mark_list() -> Node {
    el(
        "ul",
        vec![],
        vec![
            mark_row(Some(ChangeMark::Add)),
            mark_row(Some(ChangeMark::Remove)),
            mark_row(Some(ChangeMark::Change)),
            mark_row(None),
        ],
    )
}

#[test]
fn change_marks_conform_in_three_ways() {
    let v = with_children(|c| {
        c[0] = el(
            "section",
            vec![("aria-label", Region::Artifacts.aria_label())],
            vec![notice(StatusNotice::View), mark_list()],
        )
    });
    assert_eq!(check_complete(&v), Ok(()));
    assert_eq!(check_no_undefined(&v), Ok(()));
    let html = render(&mark_row(Some(ChangeMark::Add)));
    assert!(html.contains("data-mark=\"add\""), "{html}");
    assert!(html.contains('+') && html.contains("【追加】"), "{html}");
    assert_eq!(MarkValue::parse(""), Some(MarkValue::None));
    assert_eq!(
        MarkValue::parse("change"),
        Some(MarkValue::Mark(ChangeMark::Change))
    );
    assert_eq!(MarkValue::parse("ADD"), None);
}

#[test]
fn change_mark_violations_fail() {
    let glyph = |t: &str| {
        el(
            "span",
            vec![(TESTID_ATTR, MARK_GLYPH_TESTID)],
            vec![text(t)],
        )
    };
    let label = |t: &str| el("span", vec![(TESTID_ATTR, MARK_TEXT_TESTID)], vec![text(t)]);
    let row = |v: &str, kids: Vec<Node>| el("li", vec![(MARK_ATTR, v)], kids);
    let cases: Vec<(&str, Node, &str)> = vec![
        (
            "unknown value",
            row("added", vec![]),
            "undefined-mark-value",
        ),
        (
            "upper-case value",
            row("ADD", vec![]),
            "undefined-mark-value",
        ),
        (
            "glyph missing",
            row("add", vec![label("【追加】")]),
            "mark-glyph",
        ),
        (
            "glyph duplicated",
            row("add", vec![glyph("+"), glyph("+"), label("【追加】")]),
            "mark-glyph",
        ),
        (
            "glyph mismatch",
            row("add", vec![glyph("-"), label("【追加】")]),
            "mark-glyph",
        ),
        ("text missing", row("add", vec![glyph("+")]), "mark-text"),
        (
            "text mismatch",
            row("add", vec![glyph("+"), label("【削除】")]),
            "mark-text",
        ),
        (
            "none with glyph",
            row("", vec![glyph("+"), label("")]),
            "mark-glyph",
        ),
        (
            "none with text",
            row("", vec![glyph(""), label("【追加】")]),
            "mark-text",
        ),
        (
            "part with element child",
            row(
                "add",
                vec![
                    el(
                        "span",
                        vec![(TESTID_ATTR, MARK_GLYPH_TESTID)],
                        vec![el("b", vec![], vec![])],
                    ),
                    label("【追加】"),
                ],
            ),
            "mark-part-children",
        ),
        ("glyph outside row", glyph("+"), "mark-part-outside-row"),
        (
            "glyph in grandchild",
            row(
                "add",
                vec![
                    glyph("+"),
                    label("【追加】"),
                    el("div", vec![], vec![label("【追加】")]),
                ],
            ),
            "mark-part-outside-row",
        ),
        (
            "bound mark",
            el("li", vec![("data-bind-attr", "Data-Mark:x")], vec![]),
            "bound-mark",
        ),
        (
            "aria-hidden on glyph",
            row(
                "add",
                vec![
                    el(
                        "span",
                        vec![(TESTID_ATTR, MARK_GLYPH_TESTID), ("aria-hidden", "true")],
                        vec![text("+")],
                    ),
                    label("【追加】"),
                ],
            ),
            "undefined-attribute",
        ),
    ];
    for (name, view, rule) in cases {
        assert!(
            rules(check_no_undefined(&view)).contains(&rule),
            "{name}: expected {rule}"
        );
        assert!(
            rules(check_complete(&view)).contains(&rule),
            "{name}: complete expected {rule}"
        );
    }
}

#[test]
fn mark_violation_does_not_echo_value() {
    let v = el("li", vec![(MARK_ATTR, "secret-body")], vec![]);
    let msg = check_no_undefined(&v).unwrap_err()[0].to_string();
    assert!(msg.contains("undefined-mark-value"), "{msg}");
    assert!(!msg.contains("secret-body"), "{msg}");
}

#[test]
fn deep_mark_row_is_checked_linearly() {
    let mut n = mark_row(Some(ChangeMark::Add));
    for _ in 0..100_000 {
        n = elem(vec![], vec![n]);
    }
    assert_eq!(check_no_undefined(&n), Ok(()));
    let mut cur = n;
    while let Node::Element { mut children, .. } = cur {
        match children.pop() {
            Some(c) => cur = c,
            None => break,
        }
    }
}

#[test]
fn target_size_boundaries() {
    assert!(meets_target_size(24.0, 24.0));
    assert!(meets_target_size(100.0, 24.0));
    assert!(!meets_target_size(23.99, 24.0));
    assert!(!meets_target_size(24.0, 23.99));
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -24.0] {
        assert!(!meets_target_size(bad, 24.0));
        assert!(!meets_target_size(24.0, bad));
    }
}

#[test]
fn operation_targets_are_identified() {
    let t =
        |tag: &'static str, attrs: Vec<(&str, &str)>| is_operation_target(&el(tag, attrs, vec![]));
    assert!(t("button", vec![]));
    assert!(t("select", vec![]));
    assert!(t("input", vec![]));
    assert!(t("input", vec![("type", "text")]));
    assert!(!t("input", vec![("type", "hidden")]));
    assert!(!t("input", vec![("TYPE", "Hidden")]));
    assert!(t("div", vec![("data-action", "x")]));
    assert!(!t("div", vec![]));
    assert!(!t("a", vec![]));
    assert!(!is_operation_target(&text("x")));
}

#[test]
fn live_update_units() {
    assert_eq!(live_response_updates(&[]), 0);
    assert_eq!(live_response_updates(&["received"]), 0);
    assert_eq!(live_response_updates(&["progress", "progress"]), 0);
    assert_eq!(live_response_updates(&["received", "response"]), 1);
    assert_eq!(live_response_updates(&["response", "rejected"]), 1);
    assert!(!counts_for_live_response("received"));
    assert!(counts_for_live_response("Received"));
    assert!(!live_left_should_update(0));
    assert!(live_left_should_update(1));
    assert!(live_left_should_update(3));
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
            "upper-case aria-hidden",
            elem(vec![("ARIA-HIDDEN", "true")], vec![]),
            "undefined-attribute",
        ),
        (
            "mixed-case role value",
            elem(vec![("Role", "log")], vec![]),
            "undefined-role",
        ),
        (
            "mixed-case label",
            elem(vec![("Aria-Label", "x")], vec![]),
            "undefined-aria-label",
        ),
        (
            "upper-case live on non-live",
            elem(vec![("ARIA-LIVE", "assertive")], vec![]),
            "live-attribute-on-non-live",
        ),
        (
            "upper-case bound attribute",
            elem(vec![("data-bind-attr", "ARIA-LABEL:x")], vec![]),
            "bound-a11y-attribute",
        ),
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
fn complete_requires_exactly_one_log_list() {
    let dup = with_children(|c| c.push(el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], vec![])));
    assert_eq!(check_no_undefined(&dup), Ok(()));
    assert!(rules(check_complete(&dup)).contains(&"count"));

    let none = with_children(|c| {
        c[1] = el(
            "section",
            vec![("aria-label", Region::GuideAndLog.aria_label())],
            vec![notice(StatusNotice::Log)],
        );
    });
    assert_eq!(check_no_undefined(&none), Ok(()));
    assert!(rules(check_complete(&none)).contains(&"count"));
}

#[test]
fn reserved_ids_are_checked_regardless_of_role() {
    for id in [
        "pane-ops",
        "pane-artifacts",
        "pane-tab-ops",
        "pane-tab-artifacts",
    ] {
        let v = with_children(|c| c.insert(0, elem(vec![("id", id)], vec![])));
        assert!(rules(check_no_undefined(&v)).contains(&"reserved-id-role"));
        let r = rules(check_complete(&v));
        assert!(r.contains(&"reserved-id-role") && r.contains(&"count"));
    }
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
    assert_eq!(tab_key_target(TabKey::ArrowLeft, 1, usize::MAX, m), Some(0));
    assert_eq!(
        tab_key_target(TabKey::ArrowLeft, 0, usize::MAX, m),
        Some(usize::MAX - 1)
    );
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
