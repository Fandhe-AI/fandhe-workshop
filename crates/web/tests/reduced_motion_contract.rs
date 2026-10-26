//! 動きの抑制（`prefers-reduced-motion: reduce`）で段階の行を省くことの適合の試験（TASK-372 1/2。A11Y-11・EVT-19）。
//! 省くのは描画だけで、進行のイベント列・状態は動きを抑えない場合と同じであること、状態の語は変わらないことを確かめる。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::log_lines::LogLine;
use fandhe_workshop_web::motion::shows_line;
use fandhe_workshop_web::run_status::run_status;
use fandhe_workshop_web::terminal_area::RunStatus;

const TOTAL_TICKS: u32 = 5;

fn begun_host(reduced: bool) -> EngineHost {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let bytes =
        std::fs::read(root.join("engine-event-consistency/test-data/min-3step.json")).unwrap();
    let sample: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("contract/event-sequence.json")).unwrap())
            .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(&bytes).unwrap(), state);
    let mut host = EngineHost::with_session(session, standard_registry()).unwrap();
    host.set_reduced_motion(reduced);
    host
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

fn count_kind(n: &Node, kind: &str) -> usize {
    match n {
        Node::Element {
            attrs, children, ..
        } => {
            let own = attrs.iter().any(|(k, v)| k == "data-kind" && v == kind) as usize;
            own + children.iter().map(|c| count_kind(c, kind)).sum::<usize>()
        }
        _ => 0,
    }
}

/// 入力から最後の進行まで流し、各時点の段階の行の件数を返す。
fn run_all(host: &mut EngineHost) -> Vec<usize> {
    let mut seen = Vec::new();
    host.update(make_note());
    seen.push(count_kind(&host.view(), "progress"));
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
        seen.push(count_kind(&host.view(), "progress"));
    }
    seen
}

#[test]
fn shows_line_の表は_reduce_のとき段階だけを省く() {
    let progress = LogLine::Progress {
        stage: "s".to_owned(),
        index: 0,
        total: 5,
    };
    let others = [
        LogLine::Response {
            body: "b".to_owned(),
            next_action: None,
        },
        LogLine::Stopped,
        LogLine::Reloaded,
        LogLine::Limit { landings: 1 },
    ];
    assert!(!shows_line(true, &progress));
    assert!(shows_line(false, &progress));
    for l in &others {
        assert!(shows_line(true, l));
        assert!(shows_line(false, l));
    }
}

#[test]
fn reduce_では段階の行が_0_件で_結果の行は出る() {
    let mut host = begun_host(true);
    let seen = run_all(&mut host);
    assert!(seen.iter().all(|n| *n == 0), "{seen:?}");
    assert!(count_kind(&host.view(), "received") >= 1);
}

#[test]
fn 通常では段階の行が_5_件になる_対照() {
    let mut host = begun_host(false);
    let seen = run_all(&mut host);
    assert_eq!(seen.last(), Some(&(TOTAL_TICKS as usize)), "{seen:?}");
}

#[test]
fn reduce_でもイベント列と状態は通常と同じ() {
    let mut a = begun_host(false);
    let mut b = begun_host(true);
    run_all(&mut a);
    run_all(&mut b);
    let (sa, sb) = (a.model().session().unwrap(), b.model().session().unwrap());
    assert_eq!(format!("{:?}", sa.log()), format!("{:?}", sb.log()));
    assert_eq!(format!("{:?}", sa.state()), format!("{:?}", sb.state()));
}

#[test]
fn reduce_でも状態の語は待機中_処理中_待機中と遷移する() {
    let mut host = begun_host(true);
    assert_eq!(run_status(host.model().session()), RunStatus::Idle);
    host.update(make_note());
    assert_eq!(run_status(host.model().session()), RunStatus::Busy);
    for i in 0..TOTAL_TICKS {
        host.update(tick(i));
    }
    assert_eq!(run_status(host.model().session()), RunStatus::Idle);
}

// 静的な CSS の検査。ブラウザ試験（tools/browser/reduced-motion.test.mjs）と同じ判定を native でも持つのは、
// 変異注入（A11Y11-2・A11Y11-3）がブラウザなしで検出できるようにするため（A11Y-11。TASK-372 2/2）。
// コメントを除き小文字にしてから、`keyframes`（前置き付きを含む）と `animation`・`transition` 系のプロパティ名を数える。
fn motion_declarations(css: &str) -> usize {
    let mut plain = String::new();
    let mut rest = css;
    while let Some(start) = rest.find("/*") {
        plain.push_str(&rest[..start]);
        rest = match rest[start..].find("*/") {
            Some(end) => &rest[start + end + 2..],
            None => "",
        };
    }
    plain.push_str(rest);
    let lower = plain.to_lowercase();
    let keyframes = lower.matches("keyframes").count();
    let props = lower
        .split(['{', '}', ';'])
        .map(|d| {
            d.trim()
                .trim_start_matches("-webkit-")
                .trim_start_matches("-moz-")
        })
        .filter(|d| {
            let name = d.split(':').next().unwrap_or("").trim();
            d.contains(':') && (name.starts_with("animation") || name.starts_with("transition"))
        })
        .count();
    keyframes + props
}

#[test]
fn app_css_には動きの宣言が_0_件() {
    let css = include_str!("../static/app.css");
    assert_eq!(motion_declarations(css), 0);
}

#[test]
fn 動きの宣言の検出は注入を見つける_自己確認() {
    assert_eq!(
        motion_declarations("/* transition: x 1s; */ .a { color: red; }"),
        0
    );
    assert_eq!(motion_declarations(".a { transition: opacity 0.2s; }"), 1);
    assert_eq!(motion_declarations(".a { -webkit-animation-name: x; }"), 1);
    assert_eq!(motion_declarations("@keyframes k { from { top: 0; } }"), 1);
    assert_eq!(
        motion_declarations("@media (x) { .a { animation: k 1s; } }"),
        1
    );
}
