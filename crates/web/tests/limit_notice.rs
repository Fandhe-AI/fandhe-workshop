//! 遷移の上限で止めた理由の見出しの行（`limit`）と通知欄（`log-notice`）の試験（TASK-160 2/3、SAFE-34）。
//!
//! 作成者向けの検査を通さずに配られた巡回する IR で上限に達する操作を、`EngineHost` に送り、右のログに `limit` の行が
//! ちょうど 1 件、通知欄に理由が出て、1,000 回の着地が行として並ばないことを確かめる。上限に達しない巡回しない連鎖と
//! 通常の進行では、行も通知も出ない（対照）。通知欄は上限の直後に束縛点として更新される。止めた後は操作の前の Step へ戻り（SAFE-35）、そこでの入力が受け付けられると消える。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::{check_no_undefined, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::{event_kind_name, standard_registry};
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::limit_notice::{limit_notice, limit_notice_text};
use fandhe_workshop_web::log_lines::{limit_body, LIMIT_HEADING};
use fandhe_workshop_web::parts::*;

const NOTICE_FIELD: &str = "log_notice";

fn cycle_host(name: &str, at: &str) -> EngineHost {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir")
        .join(format!("{name}.json"));
    let ir = load_ir(&std::fs::read(p).unwrap()).unwrap();
    let mut state = SessionState::default();
    let uid = StepUid(at.to_owned());
    state.run.current = Some(uid.clone());
    state
        .run
        .slices
        .entry(uid.root())
        .or_default()
        .status
        .insert(uid, StepStatus::Entered);
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn go_to(s: &str) -> HostAction {
    HostAction::Engine(Op::Control(Control::GoTo(StepUid(s.to_owned()))))
}

fn text_in(host: &EngineHost) -> HostAction {
    let _ = host;
    HostAction::Engine(Op::Input(Input::Text("go".to_owned())))
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

fn children_of(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn find<'a>(n: &'a Node, testid: &str) -> Option<&'a Node> {
    if attr(n, TESTID_ATTR) == Some(testid) {
        return Some(n);
    }
    children_of(n).iter().find_map(|c| find(c, testid))
}

fn plain(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(plain).collect(),
        _ => String::new(),
    }
}

fn limit_lines(view: &Node) -> Vec<&Node> {
    let list = find(view, "part-log-list").expect("part-log-list");
    children_of(list)
        .iter()
        .filter(|li| attr(li, "data-kind") == Some("limit"))
        .collect()
}

fn notice_dom(view: &Node) -> String {
    let n = find(view, "log-notice").expect("log-notice");
    assert_eq!(attr(n, "role"), Some("status"));
    assert!(children_of(n).iter().all(|c| matches!(c, Node::Text(_))));
    plain(n)
}

fn notice_bound(host: &EngineHost) -> String {
    match host.bound_value(NOTICE_FIELD) {
        Some(BoundValue::Text(s)) => s,
        other => panic!("{other:?}"),
    }
}

fn limit_events(host: &EngineHost) -> usize {
    host.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::TransitionLimitReached { .. }))
        .count()
}

fn received_count(host: &EngineHost) -> usize {
    host.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::InputReceived { .. }))
        .count()
}

#[test]
fn 巡回で上限に達すると_limit_行が1件と通知欄の理由が出て_着地は行にならない() {
    for (name, from, to) in [
        ("cycle-enter-self", "main/st-1", "main/u-self"),
        ("cycle-enter-2", "main/st-1", "main/u-a"),
    ] {
        let mut host = cycle_host(name, from);
        let before = find(&host.view(), "part-log-list")
            .map(|l| children_of(l).len())
            .unwrap();
        host.update(go_to(to));
        assert_eq!(limit_events(&host), 1, "{name}");
        let view = host.view();
        let lines = limit_lines(&view);
        assert_eq!(lines.len(), 1, "{name}");
        let heading = find(lines[0], "line-heading").expect("line-heading");
        assert_eq!(plain(heading), LIMIT_HEADING);
        let body = find(lines[0], "line-body").expect("line-body");
        assert_eq!(plain(body), limit_body(1000));
        assert!(plain(body).contains("1,000"));
        // 1,000 件の着地を行に並べない。増えた行は少数（入力の記録など）で、上限の行を含めても数行に収まる。
        let after = children_of(find(&view, "part-log-list").unwrap()).len();
        assert!(after - before <= 5, "{name}: {before} -> {after}");
        // 通知欄は DOM と束縛値が同じ文で、固定の定数から作られる。
        assert_eq!(notice_dom(&view), limit_notice_text(1000), "{name}");
        assert_eq!(notice_bound(&host), limit_notice_text(1000), "{name}");
        assert!(check_no_undefined(&view).is_ok());
        // 上限で止めたイベントの名前は専用の名前で、`other` に吸われない。
        let log = host.model().session().unwrap().log();
        let last = log.last().unwrap();
        assert_eq!(event_kind_name(last), "transition_limit_reached");
        // 止めた後は操作の前の Step に戻り、そこでの入力は受け付けられる（SAFE-35）。
        // 受け付けた入力でログの末尾が変わるので通知は消え、上限の行は 1 件のまま残る。
        let received = received_count(&host);
        host.update(text_in(&host));
        assert_eq!(received_count(&host), received + 1, "{name}");
        assert_eq!(limit_events(&host), 1, "{name}");
        let view = host.view();
        assert_eq!(limit_lines(&view).len(), 1, "{name}");
        assert_eq!(notice_dom(&view), "", "{name}");
        assert_eq!(notice_bound(&host), "", "{name}");
    }
}

#[test]
fn 上限に達した操作で通知欄の束縛点が_dirty_になる() {
    let mut host = cycle_host("cycle-enter-self", "main/st-1");
    assert_eq!(notice_bound(&host), "");
    host.update(go_to("main/u-self"));
    assert!(host.dirty_fields().contains(&NOTICE_FIELD));
}

#[test]
fn 上限に達しない進行では行も通知も出ない() {
    let mut host = cycle_host("cycle-d07-flat", "impl-run@round1/merge-gate@gate/st-7-4");
    host.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-pr-cards".to_owned(),
        path: "prs/pr-1-a#merge".to_owned(),
    })));
    let view = host.view();
    assert!(limit_lines(&view).is_empty());
    assert_eq!(notice_dom(&view), "");
    assert_eq!(notice_bound(&host), "");
    assert_eq!(limit_notice(host.model().session()), "");
}

#[test]
fn 未読込のホストでも通知欄は空で描かれる() {
    let host = EngineHost::new();
    let view = host.view();
    assert_eq!(notice_dom(&view), "");
    assert_eq!(limit_notice(None), "");
    assert!(check_no_undefined(&view).is_ok());
}
