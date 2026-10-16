//! 誤入力の拒否が、右のログに理由と次の行動の行として出ることの試験（TASK-437 1/2。TERM-32）。
//!
//! 教材パックの `error_inputs`（期待が `rejected:*` のもの）と `fill-pack` の `text_checks` を、
//! `EngineHost` に `Op::Input` として流す。拒否の理由の控え（`HostModel::judged_text`）は
//! `apply_engine` の経路でしか作られないため、core の `Session` を直接叩かず、必ずホストの `update` を通す。
//!
//! 確かめること: 拒否 1 件につき `rejected` の行がちょうど 1 件増え、理由と次の行動が空でなく、
//! 表示する値が core の返した値（`Judged` の次の行動・同じ状態で呼んだ `filled_text` の文面）と
//! 一致し、`log-list` の下で `check_area` を通る。画面は文面を作らない。
//!
//! 47 件の誤入力（試作パック）は公開の可否が未決のため取り込まず、repo 内のパックの誤入力で代える
//! （`testdata/README.md`・`docs/testing.md`）。`slot-pack` の誤入力は静的な検査用で、実行して期待の分類にならないため使わない。件数はパックごとに定数で固定し、データが減っても素通りしない。
//!
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fandhe_frontend_core::{el, text, Node};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{Busy, RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::text_fill::filled_text;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{parse_expect, ResultExpect};
use fandhe_workshop_pack::source;
use fandhe_workshop_web::a11y::{Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::terminal_area::{check_area, RunStatus, RUN_STATUS_TESTID};
use serde_json::Value;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// 試験データの出どころ、読む欄、拒否の件数の期待（実装時の実測で固定。2026-10-16）。
struct Source {
    label: &'static str,
    dir: &'static str,
    /// `text_checks` も読むか（`error_inputs` は常に読む）。
    text_checks: bool,
    /// 期待が `rejected:*` の件数。
    rejected: usize,
}

const SOURCES: &[Source] = &[
    Source {
        label: "min-3step",
        dir: "testdata/engine-event-consistency/test-data/min-3step-pack",
        text_checks: false,
        rejected: 7,
    },
    Source {
        label: "judgment",
        dir: "crates/pack/tests/data/judgment-pack",
        text_checks: false,
        rejected: 17,
    },
    Source {
        label: "module-reuse",
        dir: "testdata/module-reuse-rounds/test-data/pack",
        text_checks: false,
        rejected: 8,
    },
    Source {
        label: "fill",
        dir: "crates/pack/tests/data/fill-pack",
        text_checks: true,
        rejected: 0,
    },
];

/// 1 件の誤入力。
struct Case {
    id: String,
    start: String,
    status: StepStatus,
    busy: Option<Busy>,
    pre: Vec<Input>,
    input: Input,
    category: RejectCategory,
    message: Option<String>,
    next_action: Option<String>,
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// 入力の指定を core の `Input` に写す。読めない形は黙って外さず失敗にする。
fn input_of(label: &str, v: &Value) -> Input {
    let obj = v
        .as_object()
        .unwrap_or_else(|| panic!("{label}: input is not a mapping"));
    let mut found: Option<Input> = None;
    for (k, val) in obj {
        let next = match k.as_str() {
            "expect" | "expect_rejection" | "expect_message" | "expect_next_action" | "target" => {
                continue
            }
            "cmd" => Input::Command(val.as_str().expect("cmd").to_string()),
            "text" => Input::Text(val.as_str().expect("text").to_string()),
            "choice" => Input::Choice(val.as_str().expect("choice").to_string()),
            "ui" => {
                let (view, path) = val
                    .as_str()
                    .expect("ui")
                    .split_once(' ')
                    .expect("ui: <view> <path>");
                Input::Ui {
                    view: view.to_string(),
                    path: path.to_string(),
                }
            }
            "approval" => Input::Approval {
                decision: match val.as_str().expect("approval") {
                    "approve" => Decision::Approve,
                    "reject" => Decision::Reject,
                    "revise" => Decision::Revise,
                    other => panic!("{label}: unknown decision {other}"),
                },
                target: str_of(v, "target").map(str::to_string),
            },
            "cancel" => Input::Cancel,
            other => panic!("{label}: unknown input key {other}"),
        };
        assert!(found.replace(next).is_none(), "{label}: two input forms");
    }
    found.unwrap_or_else(|| panic!("{label}: no input form"))
}

/// パックから、期待が `rejected:*` の誤入力だけを読む。
fn cases_of(src: &Source) -> (Ir, Vec<Case>) {
    let dir = root().join(src.dir);
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile {}: {d:?}", src.label));
    let ir = load_ir(&bytes).expect("load_ir");
    let lp = source::load(&dir).expect("load");
    let tests = lp.tests.as_ref().expect("tests");
    let mut lists: Vec<&Vec<Value>> = vec![&tests.error_inputs];
    if src.text_checks {
        lists.push(&tests.text_checks);
    }
    let mut out = Vec::new();
    for v in lists.into_iter().flatten() {
        let id = str_of(v, "id").expect("id").to_string();
        let input_v = &v["input"];
        let expect = parse_expect(str_of(input_v, "expect").unwrap_or("accepted"))
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        let ResultExpect::Rejected(category) = expect else {
            continue;
        };
        let status = match str_of(v, "status") {
            None | Some("entered") => StepStatus::Entered,
            Some("accepted") => StepStatus::Accepted,
            Some(other) => panic!("{id}: unknown status {other}"),
        };
        let busy = v.get("busy").map(|b| Busy {
            done: u32::try_from(b["done"].as_u64().expect("done")).expect("done"),
            total: u32::try_from(b["total"].as_u64().expect("total")).expect("total"),
        });
        let pre = v
            .get("pre")
            .and_then(Value::as_array)
            .map(|a| a.iter().map(|p| input_of(&id, p)).collect())
            .unwrap_or_default();
        out.push(Case {
            start: str_of(v, "start").expect("start").to_string(),
            status,
            busy,
            pre,
            input: input_of(&id, input_v),
            category,
            message: str_of(input_v, "expect_message").map(str::to_string),
            next_action: str_of(input_v, "expect_next_action").map(str::to_string),
            id,
        });
    }
    (ir, out)
}

/// 開始点の Step に入った状態。効果は当てない（`pack` の `text_fill` 試験と同じ作り方）。
fn state_of(ir: &Ir, c: &Case) -> SessionState {
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == c.start)
        .unwrap_or_else(|| panic!("start {} not found", c.start));
    let node = ir
        .nodes
        .iter()
        .find(|n| n.uid == start.node)
        .unwrap_or_else(|| panic!("node {} not found", start.node));
    let uid = StepUid(node.uid.clone());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), c.status);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    SessionState {
        run: RunState {
            current: Some(uid),
            busy: c.busy.clone(),
            slices,
            ..RunState::default()
        },
        ..SessionState::default()
    }
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

/// 部品の子が、文字ノード 1 つだけであるときの文字。
fn only_text(n: &Node) -> &str {
    match children_of(n) {
        [Node::Text(t)] => t,
        other => panic!("文字ノード 1 つではない: {other:?}"),
    }
}

/// `log-list` の親に載せ替えた木（`check_area` が確かめる形）。
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

fn rejected_rows(host: &EngineHost) -> usize {
    let view = host.view();
    let list = find(&view, "part-log-list").expect("part-log-list");
    children_of(list)
        .iter()
        .filter(|li| attr(li, "data-kind") == Some("rejected"))
        .count()
}

/// 1 件を流し、ログの行を検査する。返す値は（`rejected` の行の増分、空の次の行動の件数）。
fn run_case(ir: &Ir, c: &Case) -> (usize, usize) {
    let session = Session::from_state(ir.clone(), state_of(ir, c));
    let mut host = EngineHost::with_session(session, standard_registry()).unwrap();
    for p in &c.pre {
        host.update(HostAction::Engine(Op::Input(p.clone())));
    }
    let rows_before = rejected_rows(&host);
    let run_before = host.model().session().unwrap().state().run.clone();
    host.update(HostAction::Engine(Op::Input(c.input.clone())));

    // core が返した判定（直近の `Judged`）。
    let log = host.model().session().unwrap().log();
    let judgment = log
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{}: no Judged event", c.id));
    let Judgment::Rejected(rej) = &judgment else {
        panic!("{}: not rejected: {judgment:?}", c.id);
    };
    assert_eq!(rej.category, c.category, "{}: category", c.id);

    let view = host.view();
    let list = find(&view, "part-log-list").expect("part-log-list");
    let lis = children_of(list);
    let last = lis.last().unwrap_or_else(|| panic!("{}: no rows", c.id));
    assert_eq!(attr(last, "data-kind"), Some("rejected"), "{}: kind", c.id);
    assert_eq!(
        attr(last, "data-category"),
        Some(c.category.as_str()),
        "{}: data-category",
        c.id
    );
    assert_eq!(
        children_of(last).len(),
        2,
        "{}: 子は理由と次の行動だけ",
        c.id
    );
    let reason = find(last, "line-reason").unwrap_or_else(|| panic!("{}: line-reason", c.id));
    let next = find(last, "line-next-action").unwrap_or_else(|| panic!("{}: next", c.id));
    let (reason, next) = (only_text(reason), only_text(next));
    assert!(!reason.trim().is_empty(), "{}: 理由が空", c.id);

    // 表示する値は、core が返した値そのもの（画面は文面を作らない）。
    assert_eq!(next, rej.next_action.as_str(), "{}: next action", c.id);
    let filled = filled_text(ir, &run_before, &c.input, &judgment)
        .unwrap_or_else(|e| panic!("{}: filled_text: {e}", c.id))
        .unwrap_or_else(|| panic!("{}: no filled text", c.id));
    assert_eq!(reason, filled.message, "{}: reason", c.id);
    if let Some(want) = &c.message {
        assert_eq!(reason, want, "{}: expect_message", c.id);
    }
    if let Some(want) = &c.next_action {
        assert_eq!(next, want, "{}: expect_next_action", c.id);
    }

    // 復元（既存のログを持つ Session からの再構築）でも、同じ理由の文面と行が残る。
    let restored = EngineHost::with_session(
        Session::from_state(ir.clone(), host.model().session().unwrap().state().clone()),
        standard_registry(),
    )
    .unwrap();
    let last = u32::try_from(
        log.iter()
            .rposition(|e| matches!(e, Event::Judged { .. }))
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        restored.model().judged_text(last),
        host.model().judged_text(last),
        "{}: 復元後の理由の文面",
        c.id
    );
    assert_eq!(
        rejected_rows(&restored),
        rejected_rows(&host),
        "{}: 復元後の rejected の行",
        c.id
    );

    // `log-list` の親に載せ替えても適合する。
    check_area(&area_of(lis.to_vec())).unwrap_or_else(|v| panic!("{}: {v:?}", c.id));

    let empty = usize::from(next.trim().is_empty());
    (rejected_rows(&host) - rows_before, empty)
}

#[test]
fn 誤入力の拒否は理由と空でない次の行動の行として出る() {
    let mut total = 0;
    let (mut rows, mut empties) = (0, 0);
    for src in SOURCES {
        let (ir, cases) = cases_of(src);
        if !src.text_checks {
            assert_eq!(cases.len(), src.rejected, "{}: 拒否の件数", src.label);
        }
        for c in &cases {
            let (r, e) = run_case(&ir, c);
            rows += r;
            empties += e;
            total += 1;
        }
    }
    // 拒否 1 件につき行がちょうど 1 件。空の次の行動は 0 件。
    assert_eq!(rows, total, "拒否の件数と rejected の行の件数");
    assert_eq!(empties, 0, "空の次の行動");
    assert!(total >= 31, "試験の件数が減った: {total}");
}

#[test]
fn fill_pack_の拒否は期待の文面と次の行動のとおりに出る() {
    let src = SOURCES.iter().find(|s| s.label == "fill").unwrap();
    let (ir, cases) = cases_of(src);
    assert!(!cases.is_empty());
    for c in &cases {
        assert!(
            c.message.is_some() && c.next_action.is_some(),
            "{}: 期待が無い",
            c.id
        );
        run_case(&ir, c);
    }
}
