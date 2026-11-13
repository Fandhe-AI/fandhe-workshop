//! ライブの欄（`live-response`・`live-left`）の host の層の試験（A11Y-6。TASK-370 1/2）。
//!
//! `EngineHost::view` が 2 つの欄を root の直下に属性つきで描くこと、応答（契約 `a11y::counts_for_live_response`
//! が真の行を含む 1 回の更新）ごとに `live_response` がちょうど 1 回 dirty に積まれ値が変わること、
//! 受領・進捗だけの更新では積まれないことを確かめる。ブラウザでの MutationObserver による数え直しは
//! `tools/browser/live-response.test.mjs` の担当で、読み上げの証拠にはならない（TASK-381）。
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
use fandhe_workshop_web::a11y::{self, LiveRegion, TESTID_ATTR};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::live_region::LIVE_RESPONSE_FIELD;
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

fn live_text(h: &EngineHost) -> String {
    match h.bound_value(LIVE_RESPONSE_FIELD.name) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("live_response は文字の束縛点: {other:?}"),
    }
}

fn live_dirty(h: &EngineHost) -> usize {
    h.dirty_fields()
        .iter()
        .filter(|f| **f == LIVE_RESPONSE_FIELD.name)
        .count()
}

#[test]
fn 二つの欄が_root_の直下に属性つきで描かれる() {
    let h = host_at("main/st-1-1");
    let view = h.view();
    assert_eq!(a11y::check_no_undefined(&view), Ok(()));
    for region in LiveRegion::ALL {
        let mut found = Vec::new();
        find_all(&view, region.testid(), &mut found);
        assert_eq!(found.len(), 1, "{} は 1 個", region.testid());
        let n = found[0];
        assert!(
            kids(&view).iter().any(|c| std::ptr::eq(c, n)),
            "root の直接の子"
        );
        for (k, v) in a11y::LIVE_REGION_ATTRS {
            assert_eq!(attr(n, k), Some(v), "{k} は直接の属性");
        }
        assert_eq!(attr(n, "role"), None);
        assert_eq!(attr(n, "hidden"), None, "hidden は AX ツリーから消える");
        assert!(
            matches!(kids(n), [Node::Text(_)]),
            "子は文字ノード 1 つだけ"
        );
        assert_ne!(
            attr(n, "data-bind-attr").map(|v| v.contains("aria")),
            Some(true)
        );
    }
    // tabpanel の外にある。
    let mut panels = Vec::new();
    find_all(&view, "pane-ops", &mut panels);
    for p in panels {
        let mut inside = Vec::new();
        find_all(p, "live-response", &mut inside);
        find_all(p, "live-left", &mut inside);
        assert!(inside.is_empty(), "欄は tabpanel の中に無い");
    }
}

#[test]
fn 拒否は_1_回だけ更新し_同じ拒否が続いても通し番号で値が変わる() {
    let mut h = host_at("main/st-1-1");
    assert_eq!(live_text(&h), "");
    h.update(cmd("/hepl"));
    assert_eq!(live_dirty(&h), 1, "1 回だけ積む");
    let first = live_text(&h);
    assert!(first.starts_with("応答 1: "), "{first}");
    assert!(first.len() > "応答 1: ".len(), "理由と次の行動を含む");
    h.update(cmd("/hepl"));
    assert_eq!(live_dirty(&h), 1);
    let second = live_text(&h);
    assert!(second.starts_with("応答 2: "), "{second}");
    assert_ne!(first, second, "同じ拒否でも値が変わる");
}

#[test]
fn 問い返しと停止はそれぞれ_1_回更新する() {
    let mut h = host_at("main/st-1-4");
    h.update(cmd("/new-idea"));
    assert_eq!(live_dirty(&h), 1);
    assert!(live_text(&h).starts_with("応答 1: "));
    h.update(HostAction::Engine(Op::Control(Control::Stop)));
    // 処理中でない Stop は Cancel の入力として扱われる。更新があれば 1 回、無ければ 0 回で、2 回以上にはならない。
    assert!(live_dirty(&h) <= 1);
}

#[test]
fn 受領だけの更新では欄を触らない() {
    let mut h = host_at("main/st-1-1");
    h.update(cmd("/hepl"));
    let before = live_text(&h);
    // 注記の無い受付（`/help`）は行にならず、`received` だけが増える。
    h.update(cmd("/help"));
    assert_eq!(live_dirty(&h), 0, "received だけでは積まない");
    assert_eq!(live_text(&h), before);
    // 自然文の受付は注記つきの応答の行になり、1 回更新する。
    h.update(HostAction::Engine(Op::Input(Input::Text(
        "こんにちは".into(),
    ))));
    assert_eq!(live_dirty(&h), 1);
}

#[test]
fn html_風の文字列は文字ノードのまま() {
    let mut h = host_at("main/st-1-1");
    h.update(cmd("/<img src=x onerror=alert(1)>"));
    let view = h.view();
    let mut found = Vec::new();
    find_all(&view, "live-response", &mut found);
    assert!(matches!(kids(found[0]), [Node::Text(_)]));
    let mut imgs = Vec::new();
    find_all(&view, "img", &mut imgs);
    assert!(imgs.is_empty());
}
