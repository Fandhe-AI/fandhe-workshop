//! 状態表示（`run_status`、TASK-436 1/2）の適合の試験。処理中の入力で「処理中」になり、進行が最後まで届くと
//! 「待機中」へ戻ること、状態が変わった操作でだけ束縛点が dirty になること、描いた要素が TASK-431 の契約
//! （`terminal_area`）に適合することを確かめる。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::{el, Node};
use fandhe_frontend_interactive::Component;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::text_fill::filled_text;
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

fn stop() -> HostAction {
    HostAction::Engine(Op::Control(Control::Stop))
}

#[test]
fn 処理中の停止で_停止_になり_古い進行が届いても保たれる() {
    let mut host = begun_host();
    host.update(make_note());
    host.update(tick(0));
    host.update(stop());
    assert_eq!(run_status(host.model().session()), RunStatus::Stopped);
    assert_shows(&host, RunStatus::Stopped);
    assert!(check_no_undefined(&host.view()).is_ok());
    // 古い世代の進行は core が Ignored にする。表示は変わらず、束縛点も dirty にならない。
    host.update(tick(1));
    assert_shows(&host, RunStatus::Stopped);
    let d = host.dirty_fields();
    assert!(
        !d.contains(&WORD_FIELD) && !d.contains(&STATE_FIELD),
        "{d:?}"
    );
}

#[test]
fn 停止の後の再送信で処理中になり_最後まで届くと待機中へ戻る() {
    let mut host = begun_host();
    host.update(make_note());
    host.update(stop());
    host.update(make_note());
    assert_shows(&host, RunStatus::Busy);
    let generation = host.model().session().unwrap().state().run.generation.0;
    for i in 0..TOTAL_TICKS {
        host.update(HostAction::Engine(Op::Tick(TickAddr {
            generation: Generation(generation),
            step: StepUid::parse("main/st-1").unwrap(),
            index: i,
        })));
    }
    assert_shows(&host, RunStatus::Idle);
}

#[test]
fn 処理中でない停止では_停止_にならない() {
    let mut host = begun_host();
    host.update(stop());
    assert_ne!(run_status(host.model().session()), RunStatus::Stopped);
}

#[test]
fn 停止の操作で状態表示の束縛点が_dirty_になる() {
    let mut host = begun_host();
    host.update(make_note());
    host.update(stop());
    let d = host.dirty_fields();
    assert!(d.contains(&WORD_FIELD) && d.contains(&STATE_FIELD), "{d:?}");
}

// 試験名は変異（TERM33-3）の記録が参照するため変えない。待機中にログを変えないのは停止ボタンだけで、
// 待機中の Esc は取り消しの入力になる（TASK-441 2/2）。取り消しを受け付けない Step では拒否が 2 件増え、世代は変わらない。
#[test]
fn esc_と停止ボタンは処理中だけ停止し_待機中ではログを変えない() {
    for (name, payload) in [("esc", "Escape"), ("stop", "")] {
        let act = || EngineHost::decode_action(name, payload).expect(name);
        let mut host = begun_host();
        let len0 = host.model().session().unwrap().log().len();
        let entry = host.model().session().unwrap().state().run.current.clone();
        let world0 = host.model().session().unwrap().state().run.world.clone();
        let gen0 = host.model().session().unwrap().state().run.generation.0;
        host.update(act());
        let grown = if name == "esc" { 2 } else { 0 };
        assert_eq!(host.model().session().unwrap().log().len(), len0 + grown);
        assert_eq!(
            host.model().session().unwrap().state().run.generation.0,
            gen0
        );
        assert_shows(&host, RunStatus::Idle);
        host.update(make_note());
        host.update(tick(0));
        let gen_before = host.model().session().unwrap().state().run.generation.0;
        host.update(act());
        let run = &host.model().session().unwrap().state().run;
        assert_eq!(run.generation.0, gen_before + 1);
        assert!(run.busy.is_none());
        assert_eq!(run.current, entry);
        assert_eq!(run.world, world0);
        assert_shows(&host, RunStatus::Stopped);
    }
}

fn draft(host: &mut EngineHost, text: &str) {
    host.update(EngineHost::decode_action("draft", text).expect("draft"));
}

fn esc() -> HostAction {
    EngineHost::decode_action("esc", "Escape").expect("esc")
}

#[test]
fn 処理中に候補が開いていても_esc_は停止になり候補も閉じる() {
    let mut host = begun_host();
    draft(&mut host, "/he");
    assert!(host.candidates_open());
    host.update(make_note());
    host.update(tick(0));
    assert!(host.candidates_open(), "処理中でも候補は開いたまま");
    let session = host.model().session().unwrap();
    assert!(session.state().run.busy.is_some());
    let gen_before = session.state().run.generation.0;
    let len_before = session.log().len();
    host.update(esc());
    let session = host.model().session().unwrap();
    let run = &session.state().run;
    assert!(run.busy.is_none());
    assert_eq!(run.generation.0, gen_before + 1);
    let stopped = session.log()[len_before..]
        .iter()
        .filter(|e| matches!(e, Event::Stopped { .. }))
        .count();
    assert_eq!(stopped, 1, "停止の行が 1 件だけ増える");
    assert!(!host.candidates_open());
    assert_eq!(host.draft(), "/he");
    assert_shows(&host, RunStatus::Stopped);
    let d = host.dirty_fields();
    assert!(d.contains(&"input_candidates"), "{d:?}");
}

#[test]
fn 停止の後に候補が開いていても_esc_は閉じるだけで停止を重ねない() {
    let mut host = begun_host();
    host.update(make_note());
    host.update(tick(0));
    host.update(esc());
    assert_shows(&host, RunStatus::Stopped);
    draft(&mut host, "/hel");
    assert!(host.candidates_open());
    let session = host.model().session().unwrap();
    let gen_before = session.state().run.generation.0;
    let len_before = session.log().len();
    // 閉じた後の 2 回目の Esc（取り消しの入力）は下の試験で確かめるため、ここでは 1 回だけ押す。
    host.update(esc());
    let session = host.model().session().unwrap();
    assert!(!host.candidates_open());
    assert_eq!(session.state().run.generation.0, gen_before);
    assert_eq!(session.log().len(), len_before);
    assert_eq!(host.draft(), "/hel");
    assert_shows(&host, RunStatus::Stopped);
}

/// ログの末尾が `InputReceived { Cancel }` と `Judged { Rejected(NotInThisStep) }` の 2 件であることを確かめ、
/// 拒否の次の行動を返す。
fn assert_cancel_rejected_tail(host: &EngineHost, len_before: usize) -> String {
    let session = host.model().session().unwrap();
    let log = session.log();
    assert_eq!(log.len(), len_before + 2, "{:?}", &log[len_before..]);
    assert!(
        matches!(
            &log[len_before],
            Event::InputReceived {
                input: Input::Cancel,
                ..
            }
        ),
        "{:?}",
        log[len_before]
    );
    let Event::Judged { judgment, .. } = &log[len_before + 1] else {
        panic!("{:?}", log[len_before + 1]);
    };
    let Judgment::Rejected(rej) = judgment else {
        panic!("{judgment:?}");
    };
    assert_eq!(rej.category, RejectCategory::NotInThisStep);
    assert!(!rej.next_action.as_str().trim().is_empty());
    // 拒否の文面の控えが core の `filled_text` と一致し、空でない。
    let key = u32::try_from(len_before + 1).unwrap();
    let filled = filled_text(session.ir(), &session.state().run, &Input::Cancel, judgment)
        .unwrap()
        .expect("filled text");
    assert!(!filled.message.trim().is_empty());
    assert_eq!(host.model().judged_text(key), Some(filled.message.as_str()));
    rej.next_action.as_str().to_owned()
}

/// `part-log-list` の `rejected` の行がちょうど 1 件で、子が理由と次の行動の 2 つであることを確かめる。
fn assert_one_rejected_row(host: &EngineHost, next_action: &str) {
    let view = host.view();
    let list = find(&view, "part-log-list").expect("part-log-list");
    let rows: Vec<&Node> = children_of(list)
        .iter()
        .filter(|li| attr(li, "data-kind") == Some("rejected"))
        .collect();
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(attr(row, "data-category"), Some("not_in_this_step"));
    assert_eq!(children_of(row).len(), 2);
    let reason = text_of(find(row, "line-reason").expect("line-reason"));
    assert!(!reason.trim().is_empty());
    assert_eq!(
        text_of(find(row, "line-next-action").expect("line-next-action")),
        next_action
    );
}

#[test]
fn 待機中で候補が閉じている_esc_は取り消しの入力になり_拒否の行と次の行動が出る() {
    for text in ["", "hello"] {
        let mut host = begun_host();
        draft(&mut host, text);
        assert!(!host.candidates_open());
        assert_shows(&host, RunStatus::Idle);
        let session = host.model().session().unwrap();
        let (gen_before, len_before) = (session.state().run.generation.0, session.log().len());
        host.update(esc());
        let next = assert_cancel_rejected_tail(&host, len_before);
        let run = &host.model().session().unwrap().state().run;
        assert_eq!(run.generation.0, gen_before);
        assert!(run.busy.is_none());
        assert_eq!(host.draft(), text);
        assert!(!host.candidates_open());
        assert_shows(&host, RunStatus::Idle);
        assert_one_rejected_row(&host, &next);
        let d = host.dirty_fields();
        assert!(d.contains(&"engine_log_rows"), "{d:?}");
    }
}

#[test]
fn 候補が開いているときの_esc_は1回目で閉じ2回目で取り消しの入力になる() {
    let mut host = begun_host();
    draft(&mut host, "/he");
    assert!(host.candidates_open());
    let session = host.model().session().unwrap();
    let (gen_before, len_before) = (session.state().run.generation.0, session.log().len());
    host.update(esc());
    assert!(!host.candidates_open());
    assert_eq!(host.model().session().unwrap().log().len(), len_before);
    host.update(esc());
    let next = assert_cancel_rejected_tail(&host, len_before);
    let run = &host.model().session().unwrap().state().run;
    assert_eq!(run.generation.0, gen_before);
    assert_eq!(host.draft(), "/he");
    assert_shows(&host, RunStatus::Idle);
    assert_one_rejected_row(&host, &next);
}

#[test]
fn 停止の後に候補が閉じている_esc_は取り消しの入力になり_世代は変わらない() {
    let mut host = begun_host();
    host.update(make_note());
    host.update(tick(0));
    host.update(esc());
    assert_shows(&host, RunStatus::Stopped);
    let session = host.model().session().unwrap();
    let (gen_before, len_before) = (session.state().run.generation.0, session.log().len());
    host.update(esc());
    let next = assert_cancel_rejected_tail(&host, len_before);
    let session = host.model().session().unwrap();
    assert_eq!(session.state().run.generation.0, gen_before);
    let stopped = session
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Stopped { .. }))
        .count();
    assert_eq!(stopped, 1, "停止の行は 1 件のまま");
    assert_shows(&host, RunStatus::Idle);
    assert_one_rejected_row(&host, &next);
}
