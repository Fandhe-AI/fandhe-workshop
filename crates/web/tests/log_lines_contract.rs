//! 右のログの行（`log_lines`、TASK-434 1/2）が、TASK-431 の行の構造（`terminal_area`）に適合することの試験。
//! 合成した行と、実際の `LogPart` の出力を `log-list` の親に載せ替えた木の両方を `check_area` で照合する。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_contract::judgment::{Judgment, NextAction, RejectCategory, Rejection};
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::{check_no_undefined, Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::log_lines::{
    line_from_event, line_node, LogLine, RELOADED_TEXT, STOPPED_TEXT,
};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::terminal_area::{
    check_area, check_view, InputMethod, LineKind, RunStatus, RUN_STATUS_TESTID,
};
use std::collections::BTreeSet;

const HTML_LIKE: &str = "<img src=x onerror=alert(1)>";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn sample() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(testdata().join("contract/event-sequence.json")).unwrap())
        .unwrap()
}

fn begun_host() -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(&bytes).unwrap(), state);
    EngineHost::with_session(session, standard_registry()).unwrap()
}

fn sample_ops() -> Vec<Op> {
    sample()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e["op"].clone()).unwrap())
        .collect()
}

fn stamp() -> Stamp {
    Stamp {
        generation: Generation(1),
        step: StepUid("main/st-1".to_owned()),
    }
}

/// 案内とログの領域に、状態表示と `log-list` を入れた木。
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

fn na(s: &str) -> NextAction {
    NextAction::new(s).unwrap()
}

/// 6 種すべてと、入力の全種・承認の 3 値（対象あり・なし）を含む合成の行。
fn synthetic_lines() -> Vec<LogLine> {
    let mut v = Vec::new();
    for m in InputMethod::ALL {
        if m != InputMethod::Approval {
            v.push(LogLine::Received {
                method: m,
                body: "ls".to_owned(),
            });
        }
    }
    for d in [Decision::Approve, Decision::Reject, Decision::Revise] {
        v.push(LogLine::Approval {
            decision: d,
            target: None,
        });
        v.push(LogLine::Approval {
            decision: d,
            target: Some("t".to_owned()),
        });
    }
    v.push(LogLine::Response {
        body: "ok".to_owned(),
        next_action: None,
    });
    v.push(LogLine::Response {
        body: "ok".to_owned(),
        next_action: Some(na("次へ")),
    });
    for c in RejectCategory::ALL {
        v.push(LogLine::Rejected {
            category: c,
            reason: "r".to_owned(),
            next_action: na("直す"),
        });
    }
    v.push(LogLine::Clarified {
        question: "どれ".to_owned(),
        next_action: na("選ぶ"),
    });
    v.push(LogLine::Progress {
        stage: "s".to_owned(),
        index: 0,
        total: 3,
    });
    v.push(LogLine::Stopped);
    v.push(LogLine::Reloaded);
    v
}

fn nodes_of(lines: &[LogLine]) -> Vec<Node> {
    lines
        .iter()
        .map(|l| line_node(7, l).expect("契約に適合する行"))
        .collect()
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

#[test]
fn 合成した全種の行が_log_list_の下で_check_area_と_check_view_を通る() {
    let lines = synthetic_lines();
    let covered: BTreeSet<_> = lines.iter().map(|l| l.kind().as_str()).collect();
    let all: BTreeSet<_> = LineKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(covered, all);
    let area = area_of(nodes_of(&lines));
    check_area(&area).unwrap();
    check_view(&el("main", vec![], vec![area])).unwrap();
}

#[test]
fn data_kind_の値は定義にある種類だけである() {
    let defined: BTreeSet<_> = LineKind::ALL.iter().map(|k| k.as_str()).collect();
    for n in nodes_of(&synthetic_lines()) {
        let k = attr(&n, "data-kind").unwrap();
        assert!(defined.contains(k), "{k}");
    }
}

#[test]
fn 操作列の実出力の行を_log_list_へ載せ替えても適合し_行数は写せるイベントの数と等しい() {
    let mut host = begun_host();
    let mut seen = 0usize;
    for op in sample_ops() {
        host.update(HostAction::Engine(op));
        let view = host.view();
        let list = find(&view, "part-log-list").expect("part-log-list");
        let lis: Vec<Node> = children_of(list).to_vec();
        check_area(&area_of(lis.clone())).unwrap_or_else(|v| panic!("{v:?}"));
        let log = host.model().session().unwrap().log();
        let expected = host
            .model()
            .projection()
            .log()
            .iter()
            .filter(|r| {
                line_from_event(&log[r.event as usize], host.model().judged_text(r.event)).is_some()
            })
            .count();
        assert_eq!(lis.len(), expected);
        seen = seen.max(lis.len());
        assert!(check_no_undefined(&host.view()).is_ok());
    }
    assert!(seen > 0, "操作列で行が 1 件も出ていない");
}

#[test]
fn イベントから行への写しは表のとおりで_写さないものは_none() {
    let inputs = [
        (Input::Command("ls".into()), "command"),
        (Input::Text("t".into()), "text"),
        (Input::Choice("c".into()), "choice"),
        (
            Input::Ui {
                view: "v".into(),
                path: "p".into(),
            },
            "ui",
        ),
        (
            Input::Approval {
                decision: Decision::Approve,
                target: None,
            },
            "approval",
        ),
    ];
    for (input, method) in inputs {
        let line = without_text(&Event::InputReceived { at: stamp(), input }).unwrap();
        assert_eq!(line.kind(), LineKind::Received);
        let n = line_node(1, &line).unwrap();
        assert_eq!(attr(&n, "data-input"), Some(method));
    }
    assert!(without_text(&Event::InputReceived {
        at: stamp(),
        input: Input::Cancel
    })
    .is_none());
    let p = without_text(&Event::Progress {
        at: stamp(),
        name: "n".into(),
        index: 0,
        total: 2,
    })
    .unwrap();
    assert_eq!(p.kind(), LineKind::Progress);
    // 契約に違反する進行（index >= total）は行にしない。
    assert!(without_text(&Event::Progress {
        at: stamp(),
        name: "n".into(),
        index: 2,
        total: 2,
    })
    .is_none());
    assert!(line_node(
        1,
        &LogLine::Progress {
            stage: "n".into(),
            index: 5,
            total: 5
        }
    )
    .is_none());
    let s = without_text(&Event::Stopped {
        at: stamp(),
        new_generation: Generation(2),
    })
    .unwrap();
    assert_eq!(s, LogLine::Stopped);
    assert!(render(&line_node(1, &s).unwrap()).contains(STOPPED_TEXT));
    assert!(without_text(&Event::Accepted { at: stamp() }).is_none());
}

/// 文面を渡さない呼び出し（拒否以外のイベントは文面を読まない）。
fn without_text(e: &Event) -> Option<LogLine> {
    line_from_event(e, None)
}

#[test]
fn 拒否は文面が渡されたときだけ行になり_問い返しは写さない() {
    let na = NextAction::new("ls を打つ").expect("next action");
    let j = Event::Judged {
        at: stamp(),
        judgment: Judgment::Rejected(Rejection {
            category: RejectCategory::UnknownCommand,
            text: "r-1".into(),
            next_action: na.clone(),
        }),
    };
    let line = line_from_event(&j, Some("そのコマンドはありません")).unwrap();
    assert_eq!(
        line,
        LogLine::Rejected {
            category: RejectCategory::UnknownCommand,
            reason: "そのコマンドはありません".into(),
            next_action: na,
        }
    );
    assert_eq!(line.kind(), LineKind::Rejected);
    // 文面が無い・空白だけなら行にしない（文面を作らない）。
    assert!(line_from_event(&j, None).is_none());
    assert!(line_from_event(&j, Some("  ")).is_none());
}

#[test]
fn 再読み込みは固定の文言の_reloaded_行になる() {
    let r = line_from_event(
        &Event::Reloaded {
            at: stamp(),
            new_generation: Generation(2),
        },
        None,
    )
    .unwrap();
    assert_eq!(r, LogLine::Reloaded);
    assert_eq!(r.kind(), LineKind::Reloaded);
    let n = line_node(1, &r).unwrap();
    assert_eq!(attr(&n, "data-kind"), Some("reloaded"));
    assert_eq!(attr(&n, "data-gen"), Some("1"));
    let body = children_of(&n);
    assert_eq!(body.len(), 1);
    assert!(matches!(children_of(&body[0]), [Node::Text(t)] if t == RELOADED_TEXT));
    check_area(&area_of(vec![n])).unwrap();
}

/// 再読み込みの模擬を 1 回・2 回挟むと、右のログの `reloaded` の行がちょうど 1・2 件増える（SAVE-6）。
#[cfg(feature = "test-hooks")]
#[test]
fn 再読み込みを挟むと_reloaded_行が回数だけ増え_ほかの行は変わらない() {
    use fandhe_workshop_web::host::RAW_RELOAD;
    let mut host = begun_host();
    for op in sample_ops().into_iter().take(2) {
        host.update(HostAction::Engine(op));
    }
    let rows = |host: &EngineHost| -> (usize, usize) {
        let view = host.view();
        let list = find(&view, "part-log-list").expect("part-log-list");
        let lis = children_of(list);
        let reloaded = lis
            .iter()
            .filter(|li| attr(li, "data-kind") == Some("reloaded"))
            .count();
        (reloaded, lis.len() - reloaded)
    };
    let (r0, other0) = rows(&host);
    assert_eq!(r0, 0);
    for n in 1..=2 {
        host.update(EngineHost::decode_action("raw", RAW_RELOAD).unwrap());
        let (r, other) = rows(&host);
        assert_eq!(r, n);
        assert_eq!(other, other0);
        let view = host.view();
        let list = find(&view, "part-log-list").expect("part-log-list");
        check_area(&area_of(children_of(list).to_vec())).unwrap_or_else(|v| panic!("{v:?}"));
    }
}

#[test]
fn html_風の文字列は要素にならずエスケープされる() {
    let lines = vec![
        LogLine::Received {
            method: InputMethod::Text,
            body: HTML_LIKE.to_owned(),
        },
        LogLine::Approval {
            decision: Decision::Revise,
            target: Some(HTML_LIKE.to_owned()),
        },
        LogLine::Progress {
            stage: HTML_LIKE.to_owned(),
            index: 0,
            total: 1,
        },
    ];
    for n in nodes_of(&lines) {
        let html = render(&n);
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("&lt;img"), "{html}");
        // 部品の子は文字ノードだけ（属性にも入らない）。
        for c in children_of(&n) {
            assert!(children_of(c).iter().all(|g| matches!(g, Node::Text(_))));
            if let Node::Element { attrs, .. } = c {
                assert!(attrs.iter().all(|(_, v)| !v.contains('<')));
            }
        }
    }
    check_area(&area_of(nodes_of(&lines))).unwrap();
}

#[test]
fn 空の次の行動は型で作れない() {
    assert!(NextAction::new("  ").is_err());
    assert!(NextAction::new("").is_err());
}
