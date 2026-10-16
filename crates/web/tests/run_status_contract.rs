//! 状態表示（`run_status`、TASK-436 1/2）の適合の試験。処理中の入力で「処理中」になり、進行が最後まで届くと
//! 「待機中」へ戻ること、状態が変わった操作でだけ束縛点が dirty になること、描いた要素が TASK-431 の契約
//! （`terminal_area`）に適合することを確かめる。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::{el, Node};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::{check_no_undefined, Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::*;
use fandhe_workshop_web::run_status::run_status;
use fandhe_workshop_web::terminal_area::{check_area, RunStatus, RUN_STATUS_TESTID, STATE_ATTR};

const WORD_FIELD: &str = "run_status_word";
const STATE_FIELD: &str = "run_status_state";
const TOTAL_TICKS: u32 = 5;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn begun_host() -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(&bytes).unwrap(), state);
    EngineHost::with_session(session, standard_registry()).unwrap()
}

fn make_note() -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

fn tick(index: u32) -> HostAction {
    HostAction::Engine(Op::Tick(TickAddr {
        generation: Generation(1),
        step: StepUid::parse("main/st-1").unwrap(),
        index,
    }))
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

fn find<'a>(n: &'a Node, testid: &str) -> Option<&'a Node> {
    if attr(n, TESTID_ATTR) == Some(testid) {
        return Some(n);
    }
    match n {
        Node::Element { children, .. } => children.iter().find_map(|c| find(c, testid)),
        _ => None,
    }
}

fn children_of(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn text_of(n: &Node) -> String {
    children_of(n)
        .iter()
        .map(|c| match c {
            Node::Text(t) => t.clone(),
            _ => String::new(),
        })
        .collect()
}

fn text_field(host: &EngineHost, field: &str) -> String {
    match host.bound_value(field) {
        Some(BoundValue::Text(s)) => s,
        other => panic!("{field}: {other:?}"),
    }
}

/// 描画の `run-status` と、束縛値が同じ値を指すこと。値は `RunStatus` の固定の語だけである。
fn assert_shows(host: &EngineHost, expected: RunStatus) {
    let view = host.view();
    let node = find(&view, RUN_STATUS_TESTID).expect("run-status");
    assert_eq!(attr(node, STATE_ATTR), Some(expected.data_state()));
    assert_eq!(text_of(node), expected.word());
    assert_eq!(text_field(host, WORD_FIELD), expected.word());
    assert_eq!(text_field(host, STATE_FIELD), expected.data_state());
}

#[test]
fn 未読込のホストは待機中である() {
    let host = EngineHost::new();
    assert_eq!(run_status(None), RunStatus::Idle);
    assert_shows(&host, RunStatus::Idle);
    assert!(check_no_undefined(&host.view()).is_ok());
}

#[test]
fn 入力で処理中になり_最後の進行の後に待機中へ戻る() {
    let mut host = begun_host();
    assert_eq!(run_status(host.model().session()), RunStatus::Idle);
    assert_shows(&host, RunStatus::Idle);
    host.update(make_note());
    assert_eq!(run_status(host.model().session()), RunStatus::Busy);
    assert_shows(&host, RunStatus::Busy);
    for i in 0..TOTAL_TICKS - 1 {
        host.update(tick(i));
        assert_shows(&host, RunStatus::Busy);
    }
    host.update(tick(TOTAL_TICKS - 1));
    assert_eq!(host.next_tick(), None);
    assert_eq!(run_status(host.model().session()), RunStatus::Idle);
    assert_shows(&host, RunStatus::Idle);
}

#[test]
fn 状態が切り替わった操作でだけ_束縛点が_dirty_になる() {
    let mut host = begun_host();
    host.update(make_note());
    let d = host.dirty_fields();
    assert!(d.contains(&WORD_FIELD) && d.contains(&STATE_FIELD), "{d:?}");
    for i in 0..TOTAL_TICKS - 1 {
        host.update(tick(i));
        let d = host.dirty_fields();
        assert!(
            !d.contains(&WORD_FIELD) && !d.contains(&STATE_FIELD),
            "{i}: {d:?}"
        );
    }
    host.update(tick(TOTAL_TICKS - 1));
    let d = host.dirty_fields();
    assert!(d.contains(&WORD_FIELD) && d.contains(&STATE_FIELD), "{d:?}");
}

#[test]
fn 進行の行は進行の数だけ並び_data_index_は昇順で_data_total_は一定() {
    let mut host = begun_host();
    host.update(make_note());
    let rows = |host: &EngineHost| -> Vec<(String, String)> {
        let view = host.view();
        let list = find(&view, "part-log-list").expect("part-log-list");
        children_of(list)
            .iter()
            .filter(|li| attr(li, "data-kind") == Some("progress"))
            .map(|li| {
                (
                    attr(li, "data-index").unwrap().to_owned(),
                    attr(li, "data-total").unwrap().to_owned(),
                )
            })
            .collect()
    };
    let mut seen = rows(&host).len();
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
        let now = rows(&host);
        assert_eq!(now.len(), seen + 1, "tick {i}");
        seen = now.len();
    }
    let all = rows(&host);
    let expected: Vec<(String, String)> = (0..TOTAL_TICKS)
        .map(|i| (i.to_string(), TOTAL_TICKS.to_string()))
        .collect();
    assert_eq!(all, expected);
}

#[test]
fn 描いた状態表示は契約に適合し_画面全体は未定義の識別子を持たない() {
    let mut host = begun_host();
    host.update(make_note());
    for step in 0..=TOTAL_TICKS {
        let view = host.view();
        assert!(check_no_undefined(&view).is_ok());
        let status = find(&view, RUN_STATUS_TESTID).expect("run-status").clone();
        let list = find(&view, "part-log-list").expect("part-log-list");
        let lis = children_of(list).to_vec();
        let area = el(
            "section",
            vec![("aria-label", Region::GuideAndLog.aria_label())],
            vec![status, el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], lis)],
        );
        check_area(&area).unwrap_or_else(|v| panic!("{step}: {v:?}"));
        if step < TOTAL_TICKS {
            host.update(tick(step));
        }
    }
}
