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

/// `app.js` が動きの設定を読み込み時に 1 回だけ読む形を損なう記述の一覧（A11Y-12・D-82。既知の不足 H-12）。
/// `matchMedia` の呼び出しが 1 回でなければ、または `matchMedia` と同じ行の `change` の購読・`addListener(`・
/// `onchange` があれば、開いたままの切り替えに追随しうるため数える。ブラウザ試験
/// （`reduced-motion-st15.test.mjs` の E）が実際の画面で確かめる内容の、ソース側の固定である。
fn follow_up_hazards(js: &str) -> Vec<String> {
    let mut found = Vec::new();
    let calls = js.matches("matchMedia").count();
    if calls != 1 {
        found.push(format!("matchMedia の出現が {calls} 回（1 回のはず）"));
    }
    for needle in [".addListener(", "onchange", "MediaQueryListEvent"] {
        if js.contains(needle) {
            found.push(format!("change の購読: {needle}"));
        }
    }
    // 同じ行で matchMedia と change を扱う購読。ファイル入力の change（document の購読）とは区別する。
    // 変数へ受けてから別の行で購読する書き方は検出できない（近似。ブラウザ試験の E が実際の挙動で補う）。
    for line in js.lines() {
        if line.contains("matchMedia") && line.contains("change") {
            found.push(format!("matchMedia と change が同じ行: {}", line.trim()));
        }
    }
    found
}

#[test]
fn app_js_は動きの設定を読み込み時に_1_回だけ読み変化に追随しない() {
    let js = include_str!("../static/app.js");
    assert_eq!(follow_up_hazards(js), Vec::<String>::new());
    let read = js.find("matchMedia").expect("matchMedia の読み取りがある");
    let mount = js
        .find("mount_app(\"app\"")
        .expect("mount_app の呼び出しがある");
    assert!(read < mount, "設定の読み取りは mount_app の呼び出しより前");
}

#[test]
fn 追随の検出は注入を見つける_自己確認() {
    assert_eq!(follow_up_hazards("const a = 1;").len(), 1);
    assert!(follow_up_hazards("const m = window.matchMedia('x');").is_empty());
    let sub = "const m = window.matchMedia('x'); m.addEventListener(\"change\", f);\ndocument.addEventListener(\"change\", g);";
    assert_eq!(follow_up_hazards(sub).len(), 1);
    assert_eq!(
        follow_up_hazards("matchMedia('x').addListener(f);\nm.onchange = f;").len(),
        2
    );
    assert_eq!(
        follow_up_hazards("matchMedia('a'); matchMedia('b');").len(),
        1
    );
}
