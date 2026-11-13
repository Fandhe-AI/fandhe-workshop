//! `live-left`（左の画面の変化の要約）の host の層の試験（A11Y-8。TASK-370 2/2）。
//!
//! 左を変える受付で `live_left` が「「<題>」: 左の画面が N 件変わりました」になりちょうど 1 回 dirty に積まれること、
//! 変えない拒否では積まれないこと、確認を省く Step で変化と移動が同じ更新に入っても変えた側の題で知らせること、
//! 移動で空に戻ること、題の HTML 風の文字列が文字ノードのままであることを確かめる。
//! ブラウザでの観察は `tools/browser/live-response.test.mjs` の担当で、読み上げの証拠にはならない（TASK-381）。
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_client::{BindingSource, BoundValue};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::a11y::TESTID_ATTR;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::live_region::{left_text, LIVE_LEFT_FIELD};
use fandhe_workshop_web::parts::Component;

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

fn kids(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn find_all<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, TESTID_ATTR) == Some(testid) {
        out.push(n);
    }
    for c in kids(n) {
        find_all(c, testid, out);
    }
}

fn ir() -> Ir {
    let dir = root().join("testdata/input-field-ime/test-data/ch1-pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

fn state_at(uid: &str) -> SessionState {
    let uid = StepUid(uid.to_owned());
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

fn host_at(uid: &str) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir(), state_at(uid)),
        standard_registry(),
    )
    .unwrap()
}

fn cmd(c: &str) -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(c.to_owned())))
}

fn left_value(h: &EngineHost) -> String {
    match h.bound_value(LIVE_LEFT_FIELD.name) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("live_left は文字の束縛点: {other:?}"),
    }
}

fn left_dirty(h: &EngineHost) -> usize {
    h.dirty_fields()
        .iter()
        .filter(|f| **f == LIVE_LEFT_FIELD.name)
        .count()
}

fn title_of(uid: &str) -> String {
    ir().nodes
        .iter()
        .find(|n| n.uid == uid)
        .map(|n| n.step.title.clone())
        .unwrap()
}

#[test]
fn 左を変える受付は題と件数の文を_1_回だけ積む() {
    let mut h = host_at("main/st-1-1");
    assert_eq!(left_value(&h), "");
    h.update(cmd("/help"));
    assert_eq!(left_dirty(&h), 1);
    assert_eq!(left_value(&h), left_text(&title_of("main/st-1-1"), 1));
    assert!(left_value(&h).ends_with("左の画面が 1 件変わりました"));
    assert!(!left_value(&h).contains("cards/"), "経路は文に入れない");
}

#[test]
fn 左を変えない拒否は積まず_文も変わらない() {
    let mut h = host_at("main/st-1-1");
    h.update(cmd("/hepl"));
    assert_eq!(left_dirty(&h), 0);
    assert_eq!(left_value(&h), "");
    h.update(cmd("/help"));
    let after = left_value(&h);
    h.update(cmd("/hepl"));
    assert_eq!(left_dirty(&h), 0, "同じ文は積まない");
    assert_eq!(left_value(&h), after);
}

#[test]
fn 確認を省く_step_は変えた側の題で知らせる() {
    let mut h = host_at("main/st-1-2");
    h.update(cmd("/help"));
    assert_eq!(left_dirty(&h), 1);
    assert_eq!(
        left_value(&h),
        left_text(&title_of("main/st-1-2"), 1),
        "次の Step に入っても空にならない"
    );
}

#[test]
fn 移動すると空に戻る() {
    let mut h = host_at("main/st-1-1");
    h.update(cmd("/help"));
    assert!(!left_value(&h).is_empty());
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-1-4".to_owned(),
    )))));
    assert_eq!(left_value(&h), "");
    assert_eq!(left_dirty(&h), 1);
}

#[test]
fn 題の_html_風の文字列は文字ノードのまま() {
    let mut ir = ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-1-1" {
            n.step.title = "<img src=x onerror=alert(1)>".to_owned();
        }
    }
    let mut h = EngineHost::with_session(
        Session::from_state(ir, state_at("main/st-1-1")),
        standard_registry(),
    )
    .unwrap();
    h.update(cmd("/help"));
    let view = h.view();
    let mut found = Vec::new();
    find_all(&view, "live-left", &mut found);
    assert!(matches!(kids(found[0]), [Node::Text(t)] if t.contains("<img")));
    let mut imgs = Vec::new();
    find_all(&view, "img", &mut imgs);
    assert!(imgs.is_empty());
}
