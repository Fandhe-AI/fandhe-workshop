//! ホストの画面の枠（TASK-517 1/2。SHELL-1）の試験。`EngineHost::view` が `shell` の完成した画面の検査と
//! `a11y` の定義外の検査を通ること、取り付け点・tablist・tabpanel の構造、部品の左右の振り分け、
//! `static/app.css` の 960px 以上の 2 列の規則を固定する。ブラウザでの配置は `tools/browser/shell-columns.test.mjs`
//! の担当である。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::TabKey;
use fandhe_workshop_web::a11y::{self, PaneTab};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::{Component, DirtyTracked, PartKind};
use fandhe_workshop_web::shell::{self, PaneMount};
use fandhe_workshop_web::shell_frame::{
    pane_from_payload, pane_of, ACTION_PANE_KEY, ACTION_SELECT_PANE, PANES_CLASS, PANE_TABS_FIELD,
};
use serde_json::Value;

const APP_CSS: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/static/app.css"));

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

/// 子孫（自分を含む）から `data-testid` が一致する要素を、文書順で集める。
fn find_all<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    for c in children(n) {
        find_all(c, testid, out);
    }
}

fn find<'a>(n: &'a Node, testid: &str) -> Vec<&'a Node> {
    let mut v = Vec::new();
    find_all(n, testid, &mut v);
    v
}

fn tag(n: &Node) -> &str {
    match n {
        Node::Element { tag, .. } => tag,
        _ => "",
    }
}

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn initial_state() -> SessionState {
    let p = testdata().join("contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    serde_json::from_value(s["initial"].clone()).unwrap()
}

fn host_of(path: PathBuf) -> EngineHost {
    let ir = load_ir(&std::fs::read(path).unwrap()).unwrap();
    EngineHost::with_session(
        Session::from_state(ir, initial_state()),
        standard_registry(),
    )
    .unwrap()
}

fn assert_frame_ok(view: &Node) {
    assert_eq!(shell::check_complete(view), Ok(()));
    assert_eq!(shell::check_no_undefined(view), Ok(()));
    assert_eq!(a11y::check_no_undefined(view), Ok(()));
}

#[test]
fn default_host_view_passes_shell_and_a11y_checks() {
    let h = EngineHost::new();
    assert_frame_ok(&h.view());
}

#[test]
fn loaded_pack_view_passes_after_steps() {
    let mut h = host_of(testdata().join("engine-event-consistency/test-data/min-3step.json"));
    assert_frame_ok(&h.view());
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        assert_frame_ok(&h.view());
        guard += 1;
        assert!(guard < 50);
    }
}

#[test]
fn input_hidden_pack_view_passes() {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/input_field_hidden/ir.json");
    let h = host_of(p);
    assert_frame_ok(&h.view());
}

#[test]
fn mounts_exist_once_and_hold_their_parts() {
    let h = EngineHost::new();
    let v = h.view();
    let lefts = find(&v, "shell-left");
    let rights = find(&v, "shell-right");
    assert_eq!(lefts.len(), 1);
    assert_eq!(rights.len(), 1);
    let (left, right) = (lefts[0], rights[0]);
    assert_eq!(find(left, "part-artifact-view").len(), 1);
    assert_eq!(find(right, "part-artifact-view").len(), 0);
    assert_eq!(find(left, "workshop-parts").len(), 1);
    assert_eq!(find(left, "hello-root").len(), 0);
    assert_eq!(find(right, "hello-root").len(), 1);
    assert_eq!(find(right, "part-log").len(), 1);
    assert_eq!(find(left, "part-log").len(), 0);
    // `workshop-parts` は `main` を含まない（Esc の場面の判定が main の中で偽のままになる）。
    let parts = find(left, "workshop-parts")[0];
    assert!(!contains_tag(parts, "main"));
    assert!(contains_tag(right, "main"));
}

fn contains_tag(n: &Node, t: &str) -> bool {
    tag(n) == t || children(n).iter().any(|c| contains_tag(c, t))
}

#[test]
fn tabs_are_in_order_with_ops_selected() {
    let h = EngineHost::new();
    let v = h.view();
    assert_eq!(find(&v, "pane-tabs").len(), 1);
    let tabs = find(&v, "pane-tab");
    let ids: Vec<&str> = tabs.iter().map(|t| attr(t, "id").unwrap()).collect();
    let want: Vec<&str> = PaneTab::ALL.iter().map(|t| t.tab_id()).collect();
    assert_eq!(ids, want);
    let selected: Vec<&str> = tabs
        .iter()
        .filter(|t| attr(t, "aria-selected") == Some("true"))
        .map(|t| attr(t, "id").unwrap())
        .collect();
    assert_eq!(selected, vec![PaneTab::Ops.tab_id()]);
}

#[test]
fn each_tabpanel_holds_its_mount() {
    let h = EngineHost::new();
    let v = h.view();
    for m in PaneMount::ALL {
        let mounts = find(&v, m.testid());
        let panel = find_panel(&v, m.pane_tab().panel_id()).expect("tabpanel");
        assert_eq!(attr(panel, "role"), Some("tabpanel"));
        assert_eq!(attr(panel, "aria-labelledby"), Some(m.pane_tab().tab_id()));
        assert_eq!(find(panel, m.testid()).len(), 1);
        assert_eq!(mounts.len(), 1);
    }
}

fn find_panel<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
    if attr(n, "id") == Some(id) {
        return Some(n);
    }
    children(n).iter().find_map(|c| find_panel(c, id))
}

#[test]
fn pane_of_table_is_fixed() {
    assert_eq!(pane_of(PartKind::ArtifactView), PaneMount::Left);
    for k in [
        PartKind::Log,
        PartKind::Offers,
        PartKind::Candidates,
        PartKind::Review,
        PartKind::Checkpoints,
    ] {
        assert_eq!(pane_of(k), PaneMount::Right, "{k:?}");
    }
}

#[test]
fn css_has_two_columns_and_hides_tabs_from_960px() {
    assert!(APP_CSS.contains("@media (min-width: 960px)"));
    let start = APP_CSS.find("@media (min-width: 960px)").unwrap();
    // 960px 以上の節だけを見る（次の @media の前まで）。
    let rest = &APP_CSS[start + 1..];
    let end = rest.find("@media").map_or(APP_CSS.len(), |i| start + 1 + i);
    let block = &APP_CSS[start..end];
    assert!(block.contains("[data-testid=\"pane-tabs\"]"));
    assert!(block.contains("display: none"));
    assert!(block.contains(&format!(".{PANES_CLASS}")));
    assert!(block.contains("display: grid"));
    assert!(block.contains("grid-template-columns: minmax(0, 1fr) minmax(0, 1fr)"));
    assert!(!APP_CSS.contains("http://") && !APP_CSS.contains("https://"));
    assert!(!APP_CSS.contains("data:"));
}

#[test]
fn root_keeps_esc_keydown_attrs_and_container_has_no_testid() {
    let h = EngineHost::new();
    let v = h.view();
    assert_eq!(attr(&v, "data-testid"), Some("workshop-root"));
    let panes = children(&v)
        .iter()
        .find(|c| attr(c, "class") == Some(PANES_CLASS))
        .expect("panes container");
    assert_eq!(attr(panes, "data-testid"), None);
}

fn selected_tabs(v: &Node) -> Vec<&str> {
    find(v, "pane-tab")
        .iter()
        .filter(|t| attr(t, "aria-selected") == Some("true"))
        .map(|t| attr(t, "id").unwrap())
        .collect()
}

fn dirty_of(h: &EngineHost) -> Vec<&'static str> {
    h.dirty_fields().to_vec()
}

#[test]
fn tablist_is_keyed_list_before_panes() {
    let h = EngineHost::new();
    let v = h.view();
    let list = find(&v, "pane-tabs")[0];
    assert_eq!(attr(list, "data-bind-list"), Some(PANE_TABS_FIELD.name));
    assert_eq!(attr(list, "data-pane"), Some("ops"));
    assert_eq!(attr(list, "data-action-keydown"), Some(ACTION_PANE_KEY));
    assert!(attr(list, "data-keys").unwrap().contains("Escape"));
    assert_eq!(attr(list, "data-keydown-prevent-default"), Some(""));
    // 項目の root が button[role=tab] そのもの（フォーカスを保つため li で包まない）。
    for t in find(list, "pane-tab") {
        assert_eq!(tag(t), "button");
        assert_eq!(attr(t, "role"), Some("tab"));
    }
    let kids = children(&v);
    let i = kids
        .iter()
        .position(|c| attr(c, "data-testid") == Some("pane-tabs"))
        .unwrap();
    assert_eq!(attr(&kids[i + 1], "class"), Some(PANES_CLASS));
}

#[test]
fn select_pane_switches_selection_and_dirty() {
    let mut h = EngineHost::new();
    h.update(HostAction::SelectPane(PaneTab::Artifacts));
    assert_eq!(dirty_of(&h), vec!["pane_tabs"]);
    let v = h.view();
    assert_eq!(
        attr(find(&v, "pane-tabs")[0], "data-pane"),
        Some("artifacts")
    );
    assert_eq!(selected_tabs(&v), vec![PaneTab::Artifacts.tab_id()]);
    for t in find(&v, "pane-tab") {
        let on = attr(t, "aria-selected") == Some("true");
        assert_eq!(attr(t, "tabindex"), Some(if on { "0" } else { "-1" }));
    }
    assert_frame_ok(&v);
    h.update(HostAction::SelectPane(PaneTab::Artifacts));
    assert!(dirty_of(&h).is_empty());
}

#[test]
fn pane_key_follows_tab_key_target() {
    let mut h = EngineHost::new();
    let steps = [
        (TabKey::ArrowRight, PaneTab::Artifacts),
        (TabKey::ArrowRight, PaneTab::Ops),
        (TabKey::ArrowLeft, PaneTab::Artifacts),
        (TabKey::Home, PaneTab::Ops),
        (TabKey::End, PaneTab::Artifacts),
    ];
    for (k, want) in steps {
        h.update(HostAction::PaneKey(k));
        let v = h.view();
        assert_eq!(selected_tabs(&v), vec![want.tab_id()], "{k:?}");
        assert_frame_ok(&v);
    }
}

#[test]
fn pane_key_requests_focus_but_click_and_noop_do_not() {
    use std::cell::Cell;
    use std::rc::Rc;
    let count = Rc::new(Cell::new(0u32));
    let mut h = EngineHost::new();
    let c = Rc::clone(&count);
    h.set_pane_focus_request(Box::new(move || c.set(c.get() + 1)));
    // キーで選んだときは、選択に合わせてフォーカスの要求が 1 回出る。
    h.update(HostAction::PaneKey(TabKey::ArrowRight));
    assert_eq!(count.get(), 1);
    h.update(HostAction::PaneKey(TabKey::Home));
    assert_eq!(count.get(), 2);
    // クリックでは出さない（押した要素が既にフォーカスを持つ）。
    h.update(HostAction::SelectPane(PaneTab::Artifacts));
    assert_eq!(count.get(), 2);
}

#[test]
fn dirty_pane_field_is_a_bind_list() {
    let h = EngineHost::new();
    let v = h.view();
    let mut lists = Vec::new();
    collect_bind_lists(&v, &mut lists);
    assert!(lists.contains(&PANE_TABS_FIELD.name.to_owned()));
}

fn collect_bind_lists(n: &Node, out: &mut Vec<String>) {
    if let Some(l) = attr(n, "data-bind-list") {
        out.push(l.to_owned());
    }
    for c in children(n) {
        collect_bind_lists(c, out);
    }
}

#[test]
fn decode_pane_actions() {
    assert!(matches!(
        EngineHost::decode_action(ACTION_SELECT_PANE, "ops"),
        Some(HostAction::SelectPane(PaneTab::Ops))
    ));
    assert!(matches!(
        EngineHost::decode_action(ACTION_SELECT_PANE, "artifacts"),
        Some(HostAction::SelectPane(PaneTab::Artifacts))
    ));
    for bad in ["", "Ops", "ops ", "other"] {
        assert!(
            EngineHost::decode_action(ACTION_SELECT_PANE, bad).is_none(),
            "{bad:?}"
        );
        assert!(pane_from_payload(bad).is_none());
    }
    assert!(matches!(
        EngineHost::decode_action(ACTION_PANE_KEY, "Escape"),
        Some(HostAction::Input(_))
    ));
    for (k, want) in [
        ("ArrowLeft", TabKey::ArrowLeft),
        ("ArrowRight", TabKey::ArrowRight),
        ("Home", TabKey::Home),
        ("End", TabKey::End),
    ] {
        match EngineHost::decode_action(ACTION_PANE_KEY, k) {
            Some(HostAction::PaneKey(got)) => assert_eq!(got, want),
            other => panic!("{k}: {other:?}"),
        }
    }
    for k in ["Enter", " ", "Tab", ""] {
        assert!(
            EngineHost::decode_action(ACTION_PANE_KEY, k).is_none(),
            "{k:?}"
        );
    }
}

#[test]
fn selection_does_not_touch_the_session() {
    let mut h = host_of(testdata().join("engine-event-consistency/test-data/min-3step.json"));
    let before = h.model().session().map(|s| s.state().clone());
    h.update(HostAction::SelectPane(PaneTab::Artifacts));
    h.update(HostAction::PaneKey(TabKey::Home));
    let after = h.model().session().map(|s| s.state().clone());
    assert_eq!(before, after);
}

#[test]
fn css_hides_unselected_pane_below_960px() {
    let start = APP_CSS
        .find("@media not all and (min-width: 960px)")
        .unwrap();
    let block = &APP_CSS[start..];
    assert!(block.contains("[data-pane=\"ops\"] ~ .sh-panes > #pane-artifacts"));
    assert!(block.contains("[data-pane=\"artifacts\"] ~ .sh-panes > #pane-ops"));
    assert!(block.contains("display: none"));
}
