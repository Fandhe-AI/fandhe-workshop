//! 右のターミナル風エリアの語の契約（`terminal_area`、TASK-431 1/2）の適合試験。
//! 値の固定・`a11y` との整合・正の見本・負の見本・違反の文面・深い木を確かめる。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::{el, render, text, Node};
use fandhe_workshop_contract::judgment::RejectCategory;
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_web::a11y::{self, Region, LOG_LIST_TESTID, TESTID_ATTR};
use fandhe_workshop_web::terminal_area::*;
use std::collections::HashSet;

fn part(id: &'static str, body: &str) -> Node {
    el("span", vec![(TESTID_ATTR, id)], vec![text(body)])
}

fn line(attrs: Vec<(&'static str, &'static str)>, parts: Vec<Node>) -> Node {
    let mut a = vec![("data-gen", "1")];
    a.extend(attrs);
    el("li", a, parts)
}

fn received(input: &'static str) -> Node {
    line(
        vec![("data-kind", "received"), ("data-input", input)],
        vec![part(LINE_BODY_TESTID, "ls")],
    )
}

fn approval(decision: &'static str) -> Node {
    line(
        vec![("data-kind", "received"), ("data-input", "approval")],
        vec![
            el(
                "span",
                vec![
                    (TESTID_ATTR, LINE_DECISION_TESTID),
                    ("data-decision", decision),
                ],
                vec![text("直す")],
            ),
            part(LINE_TARGET_TESTID, "<img src=x onerror=alert(1)>"),
        ],
    )
}

fn rejected() -> Node {
    line(
        vec![("data-kind", "rejected"), ("data-category", "wrong_order")],
        vec![
            part(LINE_REASON_TESTID, "順序"),
            part(LINE_NEXT_ACTION_TESTID, "先に進める"),
        ],
    )
}

fn progress(i: &'static str, t: &'static str) -> Node {
    line(
        vec![
            ("data-kind", "progress"),
            ("data-index", i),
            ("data-total", t),
        ],
        vec![part(LINE_STAGE_TESTID, "読込")],
    )
}

fn log(lines: Vec<Node>) -> Node {
    el("ul", vec![(TESTID_ATTR, LOG_LIST_TESTID)], lines)
}

fn area(children: Vec<Node>) -> Node {
    el(
        "section",
        vec![("aria-label", Region::GuideAndLog.aria_label())],
        children,
    )
}

fn status(s: RunStatus) -> Node {
    el(
        "span",
        vec![
            (TESTID_ATTR, RUN_STATUS_TESTID),
            ("data-state", s.data_state()),
        ],
        vec![text(s.word())],
    )
}

fn sample_lines() -> Vec<Node> {
    vec![
        received("command"),
        received("text"),
        received("choice"),
        received("ui"),
        approval("revise"),
        line(
            vec![("data-kind", "response")],
            vec![
                part(LINE_BODY_TESTID, "ok"),
                part(LINE_NEXT_ACTION_TESTID, "次へ"),
            ],
        ),
        rejected(),
        line(
            vec![("data-kind", "clarified")],
            vec![
                part(LINE_QUESTION_TESTID, "どれ"),
                part(LINE_NEXT_ACTION_TESTID, "選ぶ"),
            ],
        ),
        progress("0", "3"),
        line(
            vec![("data-kind", "stopped")],
            vec![part(LINE_BODY_TESTID, "停止")],
        ),
    ]
}

fn sample_view() -> Node {
    let v = vec![
        status(RunStatus::Idle),
        log(sample_lines()),
        el(
            "span",
            vec![(TESTID_ATTR, STEP_STATUS_TESTID)],
            vec![text(step_status_word(StepStatus::Accepted).unwrap())],
        ),
    ];
    el(
        "main",
        vec![],
        vec![
            area(v),
            el(
                "section",
                vec![("aria-label", Region::CommandInput.aria_label())],
                vec![status(RunStatus::Busy), status(RunStatus::Stopped)],
            ),
        ],
    )
}

fn first_rule(n: &Node) -> &'static str {
    match check_area(n).unwrap_err().into_iter().next().unwrap() {
        Violation::Element { rule, .. } => rule,
        _ => "other",
    }
}

fn all_rules(n: &Node) -> Vec<&'static str> {
    check_area(n)
        .unwrap_err()
        .into_iter()
        .filter_map(|v| match v {
            Violation::Element { rule, .. } => Some(rule),
            _ => None,
        })
        .collect()
}

fn one_line(l: Node) -> Node {
    area(vec![log(vec![l])])
}

#[test]
fn version_and_values_are_fixed() {
    assert_eq!(TERMINAL_AREA_CONTRACT_VERSION, 1);
    let kinds: Vec<_> = LineKind::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "received",
            "response",
            "rejected",
            "clarified",
            "progress",
            "stopped"
        ]
    );
    let inputs: Vec<_> = InputMethod::ALL.iter().map(|k| k.as_str()).collect();
    assert_eq!(inputs, ["command", "text", "choice", "ui", "approval"]);
    for k in LineKind::ALL {
        assert_eq!(LineKind::parse(k.as_str()), Some(k));
    }
    for k in InputMethod::ALL {
        assert_eq!(InputMethod::parse(k.as_str()), Some(k));
    }
    assert_eq!(LineKind::parse("nope"), None);
    let st: Vec<_> = RunStatus::ALL
        .iter()
        .map(|s| (s.data_state(), s.word()))
        .collect();
    assert_eq!(
        st,
        [("idle", "待機中"), ("busy", "処理中"), ("stopped", "停止")]
    );
    for s in RunStatus::ALL {
        assert_eq!(RunStatus::parse(s.data_state()), Some(s));
    }
    assert_eq!(
        step_status_word(StepStatus::Accepted),
        Some("受付済み（確認待ち）")
    );
    assert_eq!(step_status_word(StepStatus::Achieved), Some("達成"));
    for s in [
        StepStatus::NotStarted,
        StepStatus::Entered,
        StepStatus::Skipped,
    ] {
        assert_eq!(step_status_word(s), None);
    }
    assert_eq!(decision_value(Decision::Approve), "approve");
    assert_eq!(decision_value(Decision::Reject), "reject");
    assert_eq!(decision_value(Decision::Revise), "revise");
}

#[test]
fn identifiers_are_unique_reserved_and_delivery_safe() {
    let set: HashSet<_> = TERMINAL_AREA_TESTIDS.iter().collect();
    assert_eq!(set.len(), TERMINAL_AREA_TESTIDS.len());
    for id in TERMINAL_AREA_TESTIDS {
        assert!(id.starts_with(LINE_PREFIX) || id == RUN_STATUS_TESTID || id == STEP_STATUS_TESTID);
        assert!(!id.starts_with("live-") && !id.starts_with("mark-"));
        assert!(is_defined_testid(id));
    }
    assert!(!is_defined_testid("made-up"));
    let mut all: Vec<&str> = TERMINAL_AREA_TESTIDS.to_vec();
    all.extend(DECISION_VALUES);
    all.extend(LineKind::ALL.map(|k| k.as_str()));
    all.extend(InputMethod::ALL.map(|k| k.as_str()));
    all.extend(RunStatus::ALL.map(|k| k.data_state()));
    for v in all {
        assert!(!v.contains("debug_") && !v.contains("raw-trigger"), "{v}");
    }
}

#[test]
fn consistent_with_a11y() {
    let mut excluded: Vec<&str> = a11y::LIVE_EXCLUDED_LINE_KINDS.to_vec();
    excluded.sort_unstable();
    let mut mine = vec![LineKind::Received.as_str(), LineKind::Progress.as_str()];
    mine.sort_unstable();
    assert_eq!(excluded, mine);
    assert!(a11y::counts_for_live_response(LineKind::Response.as_str()));
    assert!(!a11y::counts_for_live_response(LineKind::Received.as_str()));
    assert!(!TERMINAL_AREA_TESTIDS.contains(&LOG_LIST_TESTID));
}

#[test]
fn input_method_maps_every_input() {
    use InputMethod as M;
    assert_eq!(
        InputMethod::from_input(&Input::Command("x".into())),
        Some(M::Command)
    );
    assert_eq!(
        InputMethod::from_input(&Input::Text("x".into())),
        Some(M::Text)
    );
    assert_eq!(
        InputMethod::from_input(&Input::Choice("x".into())),
        Some(M::Choice)
    );
    let ui = Input::Ui {
        view: "v".into(),
        path: "p".into(),
    };
    assert_eq!(InputMethod::from_input(&ui), Some(M::Ui));
    let ap = Input::Approval {
        decision: Decision::Revise,
        target: None,
    };
    assert_eq!(InputMethod::from_input(&ap), Some(M::Approval));
    assert_eq!(InputMethod::from_input(&Input::Cancel), None);
}

#[test]
fn reject_categories_are_accepted() {
    for c in RejectCategory::ALL {
        let c: &'static str = c.as_str();
        let l = line(
            vec![("data-kind", "rejected"), ("data-category", c)],
            vec![
                part(LINE_REASON_TESTID, "r"),
                part(LINE_NEXT_ACTION_TESTID, "n"),
            ],
        );
        assert!(check_area(&one_line(l)).is_ok(), "{c}");
    }
}

#[test]
fn conforming_sample_passes_and_matches_golden() {
    let v = sample_view();
    check_view(&v).unwrap();
    check_area(&area(vec![status(RunStatus::Idle), log(sample_lines())])).unwrap();
    let golden = include_str!("data/terminal_area_1/sample.html");
    assert_eq!(render(&v), golden.trim_end_matches('\n'));
    assert!(render(&v).contains("&lt;img src=x onerror=alert(1)&gt;"));
}

#[test]
fn undefined_testid_fails() {
    let bad = area(vec![el("div", vec![(TESTID_ATTR, "made-up")], vec![])]);
    assert_eq!(first_rule(&bad), "undefined-testid");
    let bad = area(vec![el(
        "div",
        vec![(TESTID_ATTR, "line-undefined")],
        vec![],
    )]);
    assert!(all_rules(&bad).contains(&"undefined-testid"));
    assert!(check_view(&el("main", vec![], vec![bad])).is_err());
}

#[test]
fn line_attribute_violations() {
    let k = |l: Node| first_rule(&one_line(l));
    assert_eq!(
        k(line(vec![("data-kind", "weird")], vec![])),
        "line-kind-unknown"
    );
    assert_eq!(k(line(vec![], vec![])), "line-kind-missing");
    let no_gen = el(
        "li",
        vec![("data-kind", "stopped")],
        vec![part(LINE_BODY_TESTID, "x")],
    );
    assert_eq!(k(no_gen), "line-gen");
    let bad_gen = el(
        "li",
        vec![("data-kind", "stopped"), ("data-gen", "1a")],
        vec![part(LINE_BODY_TESTID, "x")],
    );
    assert_eq!(k(bad_gen), "line-gen");
    let bad_cat = line(
        vec![("data-kind", "rejected"), ("data-category", "x")],
        vec![
            part(LINE_REASON_TESTID, "r"),
            part(LINE_NEXT_ACTION_TESTID, "n"),
        ],
    );
    assert_eq!(k(bad_cat), "line-category");
    let no_input = line(
        vec![("data-kind", "received")],
        vec![part(LINE_BODY_TESTID, "x")],
    );
    assert_eq!(k(no_input), "line-input");
    let bad_input = line(
        vec![("data-kind", "received"), ("data-input", "voice")],
        vec![part(LINE_BODY_TESTID, "x")],
    );
    assert_eq!(k(bad_input), "line-input");
    assert_eq!(k(progress("3", "3")), "line-progress");
    assert_eq!(k(progress("x", "3")), "line-progress");
}

#[test]
fn line_part_violations() {
    let k = |l: Node| first_rule(&one_line(l));
    let blank = line(
        vec![("data-kind", "rejected"), ("data-category", "busy")],
        vec![
            part(LINE_REASON_TESTID, "r"),
            part(LINE_NEXT_ACTION_TESTID, " \n"),
        ],
    );
    assert_eq!(k(blank), "next-action-empty");
    let missing = line(
        vec![("data-kind", "rejected"), ("data-category", "busy")],
        vec![part(LINE_REASON_TESTID, "r")],
    );
    assert_eq!(k(missing), "line-part-missing");
    let clar = line(
        vec![("data-kind", "clarified")],
        vec![part(LINE_QUESTION_TESTID, "q")],
    );
    assert_eq!(k(clar), "line-part-missing");
    let no_dec = line(
        vec![("data-kind", "received"), ("data-input", "approval")],
        vec![],
    );
    assert_eq!(k(no_dec), "line-part-missing");
    assert_eq!(k(approval("maybe")), "line-decision-value");
    let forbidden = line(
        vec![("data-kind", "received"), ("data-input", "text")],
        vec![part(LINE_BODY_TESTID, "x"), part(LINE_REASON_TESTID, "y")],
    );
    assert_eq!(k(forbidden), "line-part-forbidden");
    let dup = line(
        vec![("data-kind", "stopped")],
        vec![part(LINE_BODY_TESTID, "x"), part(LINE_BODY_TESTID, "y")],
    );
    assert_eq!(k(dup), "line-part-duplicate");
    let nested = line(
        vec![("data-kind", "stopped")],
        vec![el(
            "span",
            vec![(TESTID_ATTR, LINE_BODY_TESTID)],
            vec![part(LINE_REASON_TESTID, "y")],
        )],
    );
    assert_eq!(k(nested), "line-part-nested");
    let elem_body = line(
        vec![("data-kind", "received"), ("data-input", "text")],
        vec![el(
            "span",
            vec![(TESTID_ATTR, LINE_BODY_TESTID)],
            vec![el("b", vec![], vec![text("x")])],
        )],
    );
    assert_eq!(k(elem_body), "learner-text-children");
}

#[test]
fn structure_violations() {
    let outside = area(vec![part(LINE_BODY_TESTID, "x")]);
    assert_eq!(first_rule(&outside), "line-part-outside-line");
    let non_li = area(vec![log(vec![el("div", vec![], vec![])])]);
    assert_eq!(first_rule(&non_li), "log-list-child");
    let text_child = area(vec![log(vec![text("不正な行")])]);
    assert_eq!(first_rule(&text_child), "log-list-child");
    let raw = area(vec![Node::RawHtml("<b>".into())]);
    assert!(matches!(
        check_area(&raw).unwrap_err()[0],
        Violation::RawHtml { .. }
    ));
}

#[test]
fn run_status_violations() {
    let bad_state = el(
        "span",
        vec![(TESTID_ATTR, RUN_STATUS_TESTID), ("data-state", "x")],
        vec![text("待機中")],
    );
    assert_eq!(first_rule(&area(vec![bad_state])), "run-status-state");
    let bad_word = el(
        "span",
        vec![(TESTID_ATTR, RUN_STATUS_TESTID), ("data-state", "idle")],
        vec![text("処理中")],
    );
    assert_eq!(first_rule(&area(vec![bad_word])), "run-status-word");
    let split_word = el(
        "span",
        vec![(TESTID_ATTR, RUN_STATUS_TESTID), ("data-state", "idle")],
        vec![text("待機中"), el("span", vec![], vec![text("処理中")])],
    );
    assert_eq!(first_rule(&area(vec![split_word])), "run-status-word");
    let wrapped_word = el(
        "span",
        vec![(TESTID_ATTR, RUN_STATUS_TESTID), ("data-state", "idle")],
        vec![el("span", vec![], vec![text("待機中")])],
    );
    assert_eq!(first_rule(&area(vec![wrapped_word])), "run-status-word");
}

#[test]
fn attribute_names_are_case_insensitive() {
    let l = el(
        "li",
        vec![("DATA-KIND", "stopped"), ("Data-Gen", "2")],
        vec![el(
            "span",
            vec![("DATA-TESTID", LINE_BODY_TESTID)],
            vec![text("x")],
        )],
    );
    check_area(&one_line(l)).unwrap();
}

#[test]
fn check_view_fails_closed_and_ignores_outside() {
    let none = el(
        "main",
        vec![],
        vec![el("div", vec![(TESTID_ATTR, "made-up")], vec![])],
    );
    assert!(matches!(
        check_view(&none).unwrap_err()[0],
        Violation::Count { .. }
    ));
    let outside = el(
        "main",
        vec![],
        vec![
            el(
                "section",
                vec![("aria-label", Region::Artifacts.aria_label())],
                vec![el("div", vec![(TESTID_ATTR, "made-up")], vec![])],
            ),
            area(vec![]),
        ],
    );
    check_view(&outside).unwrap();
}

#[test]
fn violation_text_has_no_learner_strings() {
    let secret = "<img src=x onerror=SECRET>";
    let l = line(
        vec![("data-kind", "received"), ("data-input", "text")],
        vec![
            el(
                "span",
                vec![(TESTID_ATTR, LINE_BODY_TESTID)],
                vec![text(secret), el("b", vec![], vec![])],
            ),
            part(LINE_REASON_TESTID, secret),
        ],
    );
    let vs = check_area(&one_line(l)).unwrap_err();
    assert!(vs.len() >= 2);
    for v in vs {
        let s = v.to_string();
        assert!(!s.contains("SECRET") && !s.contains('<'), "{s}");
        assert!(s.is_ascii(), "{s}");
        assert!(s.contains('['), "{s}");
    }
}

#[test]
fn deep_tree_is_linear_and_does_not_overflow() {
    let mut n = el("span", vec![(TESTID_ATTR, "made-up")], vec![]);
    for _ in 0..100_000 {
        n = el("div", vec![], vec![n]);
    }
    let a = area(vec![n]);
    let vs = check_area(&a).unwrap_err();
    match &vs[0] {
        Violation::Element { path, .. } => assert_eq!(path.len(), 100_001),
        other => panic!("{other:?}"),
    }
    let view = el("main", vec![], vec![a]);
    assert!(check_view(&view).is_err());
    // Drop の再帰で落ちないよう、解放は線形に行う（子の末尾を 1 つずつ辿って外す）。
    let mut cur = view;
    while let Node::Element { mut children, .. } = cur {
        match children.pop() {
            Some(c) => cur = c,
            None => break,
        }
    }
}
