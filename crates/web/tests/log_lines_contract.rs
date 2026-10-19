//! 右のログの行（`log_lines`、TASK-434 1/2）が、TASK-431 の行の構造（`terminal_area`）に適合することの試験。
//! 合成した行と、実際の `LogPart` の出力を `log-list` の親に載せ替えた木の両方を `check_area` で照合する。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_contract::judgment::{
    Clarification, Judgment, NextAction, RejectCategory, Rejection,
};
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::{check_no_undefined, Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::log_lines::{
    group_digits, limit_body, line_from_event, line_node, LogLine, LIMIT_HEADING, RELOADED_TEXT,
    STOPPED_TEXT,
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
    v.push(LogLine::Limit { landings: 1000 });
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
fn 拒否と問い返しは文面が渡されたときだけ行になり_種類で区別される() {
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

    // 問い返しは `clarified` の行になる。`data-category` は付かない。
    let c = Event::Judged {
        at: stamp(),
        judgment: Judgment::Clarified(Clarification {
            text: "cl-1".into(),
            next_action: NextAction::new("名前を付ける").expect("next action"),
        }),
    };
    let line = line_from_event(&c, Some("名前を教えてください")).unwrap();
    assert_eq!(line.kind(), LineKind::Clarified);
    let node = line_node(1, &line).unwrap();
    assert_eq!(attr(&node, "data-kind"), Some("clarified"));
    assert_eq!(attr(&node, "data-category"), None);
    assert!(line_from_event(&c, None).is_none());
    assert!(line_from_event(&c, Some("  ")).is_none());
}

#[test]
fn 長い理由と問いは切り詰めず文字ノード_1_つで出る() {
    // 212 文字の実文は未公開の試作パックにあるため、それを超える合成の文面で代える。
    let long: String = "あ".repeat(240);
    let na = NextAction::new("ls を打つ").expect("next action");
    let r = Event::Judged {
        at: stamp(),
        judgment: Judgment::Rejected(Rejection {
            category: RejectCategory::WrongOrder,
            text: "r-1".into(),
            next_action: na.clone(),
        }),
    };
    let c = Event::Judged {
        at: stamp(),
        judgment: Judgment::Clarified(Clarification {
            text: "cl-1".into(),
            next_action: na,
        }),
    };
    for (ev, id) in [(&r, "line-reason"), (&c, "line-question")] {
        let node = line_node(1, &line_from_event(ev, Some(&long)).unwrap()).unwrap();
        let body = children_of(&node)
            .iter()
            .find(|n| attr(n, "data-testid") == Some(id))
            .unwrap();
        match children_of(body) {
            [Node::Text(t)] => assert_eq!(t, &long, "{id}: 切り詰めない"),
            other => panic!("{id}: 文字ノード 1 つではない: {other:?}"),
        }
    }
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

/// 遷移の上限で止めた理由は、専用のイベント 1 件が見出しと本文の `limit` の行 1 件になる（SAFE-34）。
#[test]
fn 上限で止めた理由は_limit_行になり_見出しと本文が固定の文言で出る() {
    let line = line_from_event(
        &Event::TransitionLimitReached {
            at: stamp(),
            landings: 1000,
        },
        None,
    )
    .unwrap();
    assert_eq!(line, LogLine::Limit { landings: 1000 });
    assert_eq!(line.kind(), LineKind::Limit);
    let n = line_node(1, &line).unwrap();
    assert_eq!(attr(&n, "data-kind"), Some("limit"));
    assert_eq!(attr(&n, "data-gen"), Some("1"));
    let heading = find(&n, "line-heading").expect("line-heading");
    assert!(matches!(children_of(heading), [Node::Text(t)] if t == LIMIT_HEADING));
    let body = find(&n, "line-body").expect("line-body");
    assert!(matches!(children_of(body), [Node::Text(t)] if t == &limit_body(1000)));
    assert!(limit_body(1000).contains("1,000"));
    check_area(&area_of(vec![n])).unwrap();
}

#[test]
fn 数は三桁ごとに区切る() {
    for (n, s) in [
        (0, "0"),
        (7, "7"),
        (999, "999"),
        (1000, "1,000"),
        (12_345, "12,345"),
        (1_000_000, "1,000,000"),
        (u32::MAX, "4,294,967,295"),
    ] {
        assert_eq!(group_digits(n), s);
    }
}

/// 個々の着地・遷移は行にしない。上限までの同じ遷移の行を 1,000 行並べず、止めた 1 行にまとめる（SAFE-34）。
/// これらを行にする後続のタスクは、まとめの規則を一緒に持ち込む必要がある。
#[test]
fn 個々の着地と遷移は行にならず_まとめは上限の_1_行だけである() {
    let entered = Event::ModuleEntered {
        at: stamp(),
        instance: "m".into(),
    };
    let exited = Event::ModuleExited {
        at: stamp(),
        instance: "m".into(),
    };
    let transitioned = Event::Transitioned {
        at: stamp(),
        to: StepUid("main/st-2".to_owned()),
    };
    let warning = Event::Warning {
        at: stamp(),
        message: "internal transitions reached the per-input limit; run stopped".into(),
    };
    let limit = Event::TransitionLimitReached {
        at: stamp(),
        landings: 1000,
    };
    let mut events = Vec::new();
    for _ in 0..1000 {
        events.push(entered.clone());
    }
    events.push(exited.clone());
    events.push(transitioned.clone());
    events.push(warning.clone());
    events.push(limit);
    let lines: Vec<_> = events
        .iter()
        .filter_map(|e| line_from_event(e, None))
        .collect();
    assert_eq!(lines, vec![LogLine::Limit { landings: 1000 }]);
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

fn plain_text(n: &Node) -> String {
    match n {
        Node::Text(s) => s.clone(),
        Node::Element { children, .. } => children.iter().map(plain_text).collect(),
        _ => String::new(),
    }
}

#[test]
fn 承認の操作の行は文言になり_観察点の文字は変わらない() {
    for (d, word) in [
        (Decision::Approve, "はい"),
        (Decision::Reject, "いいえ"),
        (Decision::Revise, "直す"),
    ] {
        let bare = line_node(
            7,
            &LogLine::Approval {
                decision: d,
                target: None,
            },
        )
        .unwrap();
        assert_eq!(plain_text(&bare), format!("承認の操作: {word}"));
        assert!(find(&bare, "line-target").is_none());
        let targeted = line_node(
            7,
            &LogLine::Approval {
                decision: d,
                target: Some("t-alpha".to_owned()),
            },
        )
        .unwrap();
        assert_eq!(
            plain_text(&targeted),
            format!("承認の操作: {word}（対象: t-alpha）")
        );
        assert_eq!(plain_text(find(&targeted, "line-decision").unwrap()), word);
        assert_eq!(
            plain_text(find(&targeted, "line-target").unwrap()),
            "t-alpha"
        );
    }
}

#[test]
fn 空の次の行動は型で作れない() {
    assert!(NextAction::new("  ").is_err());
    assert!(NextAction::new("").is_err());
}
