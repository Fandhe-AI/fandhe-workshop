//! 問い返しが、拒否と区別された行として右のログに出ることの試験（TASK-437 2/2。TERM-32）。
//!
//! ST-1.4（`ch1-pack` の `main/st-1-4`）で、引数なしの `/new-idea` は問い返し（`cl-new-idea-name`）に、
//! `/advance-phase` は `rejected:wrong_order` になる（`tests/playthrough.yaml` と同じ順）。
//! `EngineHost::update` を通して流し、`clarified` の行（子は `line-question`・`line-next-action`、`data-category` なし）と
//! `rejected` の行が別の見え方で出ること、表示する値が core の充填した文面・次の行動と一致することを確かめる。
//! 画面は文面を作らない。復元（`with_session`）と履歴の差し替え（`open_history`）でも控えが作り直される。
//!
//! 212 文字の応答は試作パック（公開の可否が未決。D-11・TASK-14）で測られた値で、repo に取り込めない。
//! 長い文面を削らないことは `log_lines_contract` の合成の文面で確かめる（`docs/testing.md`）。
//!
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::SessionOp;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::text_fill::{filled_text, filled_texts_of_log};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::a11y::{Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::terminal_area::{check_area, RunStatus, RUN_STATUS_TESTID};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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

fn only_text(n: &Node) -> &str {
    match children_of(n) {
        [Node::Text(t)] => t,
        other => panic!("文字ノード 1 つではない: {other:?}"),
    }
}

fn area_of(lis: Vec<Node>) -> Node {
    el(
        "section",
        vec![("aria-label", Region::GuideAndLog.aria_label())],
        vec![
            el(
                "span",
                vec![
                    (TESTID_ATTR, RUN_STATUS_TESTID),
                    ("data-state", RunStatus::Idle.data_state()),
                ],
                vec![text(RunStatus::Idle.word())],
            ),
            el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], lis),
        ],
    )
}

fn ir() -> Ir {
    let dir = root().join("testdata/input-field-ime/test-data/ch1-pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

/// `main/st-1-4` に入った直後の状態（`reject_lines.rs` の `state_of` と同じ作り方）。
fn st14_state() -> SessionState {
    let uid = StepUid("main/st-1-4".to_owned());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    SessionState {
        run: RunState {
            current: Some(uid),
            slices,
            ..RunState::default()
        },
        ..SessionState::default()
    }
}

fn host_at_st14(ir: &Ir) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir.clone(), st14_state()),
        standard_registry(),
    )
    .unwrap()
}

fn cmd(c: &str) -> Op {
    Op::Input(Input::Command(c.to_owned()))
}

fn rows(host: &EngineHost) -> Vec<Node> {
    let view = host.view();
    let list = find(&view, "part-log-list").expect("part-log-list");
    children_of(list).to_vec()
}

fn count_kind(host: &EngineHost, kind: &str) -> usize {
    rows(host)
        .iter()
        .filter(|li| attr(li, "data-kind") == Some(kind))
        .count()
}

fn last_judgment(host: &EngineHost) -> Judgment {
    host.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment.clone()),
            _ => None,
        })
        .expect("Judged")
}

#[test]
fn 引数なしの_new_idea_は_clarified_の行になり_拒否の行に数えない() {
    let ir = ir();
    let mut host = host_at_st14(&ir);
    let (clar_before, rej_before) = (
        count_kind(&host, "clarified"),
        count_kind(&host, "rejected"),
    );
    let run_before = host.model().session().unwrap().state().run.clone();
    let input = Input::Command("/new-idea".to_owned());
    host.update(HostAction::Engine(Op::Input(input.clone())));

    let judgment = last_judgment(&host);
    let Judgment::Clarified(c) = &judgment else {
        panic!("問い返しではない: {judgment:?}");
    };
    assert_eq!(count_kind(&host, "clarified"), clar_before + 1);
    assert_eq!(
        count_kind(&host, "rejected"),
        rej_before,
        "拒否の行は増えない"
    );

    let lis = rows(&host);
    let last = lis.last().unwrap();
    assert_eq!(attr(last, "data-kind"), Some("clarified"));
    assert_eq!(attr(last, "data-category"), None);
    let ids: Vec<_> = children_of(last)
        .iter()
        .map(|n| attr(n, TESTID_ATTR).unwrap())
        .collect();
    assert_eq!(ids, ["line-question", "line-next-action"]);
    let question = only_text(find(last, "line-question").unwrap());
    let next = only_text(find(last, "line-next-action").unwrap());
    assert!(!question.trim().is_empty() && !next.trim().is_empty());
    let filled = filled_text(&ir, &run_before, &input, &judgment)
        .unwrap()
        .unwrap();
    assert_eq!(question, filled.message, "問いは core の充填した文面");
    assert_eq!(next, c.next_action.as_str(), "次の行動は core の値");
    check_area(&area_of(lis)).unwrap_or_else(|v| panic!("{v:?}"));
}

#[test]
fn 順序違いの_advance_phase_は_received_の直後の_rejected_の行になり_問い返しと区別される() {
    let ir = ir();
    let mut host = host_at_st14(&ir);
    host.update(HostAction::Engine(cmd("/new-idea")));
    let run_before = host.model().session().unwrap().state().run.clone();
    let input = Input::Command("/advance-phase".to_owned());
    host.update(HostAction::Engine(Op::Input(input.clone())));

    let judgment = last_judgment(&host);
    let Judgment::Rejected(r) = &judgment else {
        panic!("拒否ではない: {judgment:?}");
    };
    assert_eq!(r.category, RejectCategory::WrongOrder);
    let lis = rows(&host);
    let last = lis.last().unwrap();
    assert_eq!(attr(last, "data-kind"), Some("rejected"));
    assert_eq!(attr(last, "data-category"), Some("wrong_order"));
    let prev = &lis[lis.len() - 2];
    assert_eq!(
        attr(prev, "data-kind"),
        Some("received"),
        "応答の前は入力の行"
    );
    let reason = only_text(find(last, "line-reason").unwrap());
    let filled = filled_text(&ir, &run_before, &input, &judgment)
        .unwrap()
        .unwrap();
    assert_eq!(reason, filled.message, "理由は文面のとおりで削らない");
    assert_eq!(
        only_text(find(last, "line-next-action").unwrap()),
        r.next_action.as_str()
    );
    assert_eq!(count_kind(&host, "clarified"), 1);
    assert_eq!(count_kind(&host, "rejected"), 1);
    assert!(find(last, "line-question").is_none());
}

#[test]
fn 復元でも問い返しの行が同じ文面で出る() {
    let ir = ir();
    let mut host = host_at_st14(&ir);
    host.update(HostAction::Engine(cmd("/new-idea")));
    host.update(HostAction::Engine(cmd("/advance-phase")));
    let state = host.model().session().unwrap().state().clone();
    let restored =
        EngineHost::with_session(Session::from_state(ir.clone(), state), standard_registry())
            .unwrap();
    let log = restored.model().session().unwrap().log().to_vec();
    let filled = filled_texts_of_log(&ir, &log);
    assert_eq!(filled.len(), 2, "問い返し 1 件と拒否 1 件");
    for (i, f) in &filled {
        assert_eq!(
            restored.model().judged_text(u32::try_from(*i).unwrap()),
            Some(f.message.as_str())
        );
    }
    assert_eq!(count_kind(&restored, "clarified"), 1);
    assert_eq!(count_kind(&restored, "rejected"), 1);
    assert_eq!(rows(&restored).len(), rows(&host).len());
}

#[test]
fn 履歴の差し替えで問い返しの控えは新しいログから作り直される() {
    let ir = ir();
    let initial = st14_state();
    let mut host = host_at_st14(&ir);
    host.update(HostAction::Engine(cmd("/new-idea")));
    let old_len = host.model().session().unwrap().log().len();
    let old_pos: Vec<usize> = filled_texts_of_log(&ir, host.model().session().unwrap().log())
        .into_iter()
        .map(|(i, _)| i)
        .collect();
    assert_eq!(old_pos.len(), 1, "差し替え前の問い返しは 1 件");

    // 取り込む履歴は、拒否の後に問い返しを持つ（問い返しの位置が差し替え前と違う）。
    let ops: Vec<SessionOp> = vec![
        HistoryOp::Engine(cmd("/advance-phase")),
        HistoryOp::Engine(cmd("/new-idea")),
    ];
    let state = replay(&ir, initial, &ops).unwrap();
    let new_log = state.log.clone();
    let want: BTreeMap<usize, String> = filled_texts_of_log(&ir, &new_log)
        .into_iter()
        .map(|(i, f)| (i, f.message))
        .collect();
    assert_eq!(want.len(), 2, "新しいログの文面は拒否と問い返しの 2 件");
    assert_ne!(
        want.keys().next_back(),
        Some(&old_pos[0]),
        "問い返しの位置が差し替え前と違う"
    );
    let bytes = export_bytes(&build_envelope(&ir, "s-1-1", ops, state).unwrap()).unwrap();
    assert_eq!(host.open_history(&bytes), ImportOutcome::Open);

    for i in 0..old_len.max(new_log.len()) {
        assert_eq!(
            host.model().judged_text(u32::try_from(i).unwrap()),
            want.get(&i).map(String::as_str),
            "位置 {i} の文面の控え"
        );
    }
    assert_eq!(count_kind(&host, "clarified"), 1);
    assert_eq!(count_kind(&host, "rejected"), 1);
}
