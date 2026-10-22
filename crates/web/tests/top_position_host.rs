//! 上部の教材名・章の ID・Step の位置（TASK-524。SHELL-8・D-72）の host の層の試験。
//!
//! 期待値は IR から試験の中で独自に数え（`top_position` を呼ばない）、画面の文字・属性と照らす。
//! ブラウザでの再読み込み・書き出し→読み込み・幅は `tools/browser/shell-position.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::{self, TopSection};
use fandhe_workshop_web::shell_frame::TOP_PLACE_FIELD;
use serde_json::Value;

fn data_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json")
}

fn ir() -> Ir {
    load_ir(&std::fs::read(data_path()).unwrap()).unwrap()
}

fn state_at(uid: &str) -> SessionState {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    // 開始の節点の文字列を置き換えて、スライス・現在の Step の整合を保つ（`next_chapter.rs` と同じ手）。
    let text = serde_json::to_string(&s["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{uid}\""));
    let _ = StepUid(String::new());
    serde_json::from_str(&text).unwrap()
}

fn host_at(uid: &str) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir(), state_at(uid)),
        standard_registry(),
    )
    .unwrap()
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

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    for c in kids(n) {
        find(c, testid, out);
    }
}

fn only<'a>(n: &'a Node, testid: &str) -> Option<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    assert!(v.len() <= 1, "{testid} は多くて 1 個");
    v.pop()
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        _ => kids(n).iter().map(text_of).collect(),
    }
}

/// IR から独自に数えた（章の ID, M, K）。
fn expected(uid: &str) -> (String, usize, usize) {
    let ir = ir();
    let chapter = ir.nodes.iter().find(|n| n.uid == uid).unwrap().unit.clone();
    let list: Vec<_> = ir
        .nodes
        .iter()
        .filter(|n| n.scope == "main" && n.unit == chapter)
        .map(|n| n.uid.clone())
        .collect();
    let m = list.iter().position(|u| u == uid).unwrap() + 1;
    (chapter, m, list.len())
}

fn assert_frame_ok(v: &Node) {
    assert_eq!(shell::check_complete_v2(v), Ok(()));
    assert_eq!(shell::check_no_undefined(v), Ok(()));
    assert_eq!(a11y::check_no_undefined(v), Ok(()));
}

fn assert_place(h: &EngineHost, uid: &str) {
    let v = h.view();
    assert_frame_ok(&v);
    let (c, m, k) = expected(uid);
    let pos = only(&v, TopSection::Position.testid()).expect("位置が出る");
    assert_eq!(text_of(pos), format!("{c}\u{3000}Step {m} / {k}"));
    assert_eq!(attr(pos, "data-position-m"), Some(m.to_string().as_str()));
    assert_eq!(attr(pos, "data-position-k"), Some(k.to_string().as_str()));
    let title = only(&v, TopSection::Title.testid()).expect("教材名が出る");
    assert_eq!(text_of(title), ir().pack.title);
    // 上部の中にあり、Step の ID は出ない。
    let top = only(&v, TopSection::Top.testid()).unwrap();
    assert!(only(top, TopSection::Position.testid()).is_some());
    assert!(!text_of(pos).contains(uid));
}

#[test]
fn 全_step_で教材名と章の_id_と位置が決めた形で出る() {
    for n in ir().nodes.iter().filter(|n| n.scope == "main") {
        assert_place(&host_at(&n.uid), &n.uid);
    }
}

#[test]
fn ch1_の位置は_6_件中の_1_から_6_になる() {
    let ms: Vec<usize> = (1..=6)
        .map(|i| expected(&format!("main/st-1-{i}")).1)
        .collect();
    assert_eq!(ms, vec![1, 2, 3, 4, 5, 6]);
    assert_eq!(expected("main/st-1-3").2, 6);
    assert_eq!(expected("main/st-2-1").0, "ch-2");
}

#[test]
fn 教材が無いホストは位置を出さず枠の適合を保つ() {
    let v = EngineHost::new().view();
    assert_frame_ok(&v);
    assert!(only(&v, TopSection::Position.testid()).is_none());
}

#[test]
fn 位置を変えない更新では_top_place_を_dirty_にしない() {
    let mut h = host_at("main/st-1-1");
    let _ = h.dirty_fields();
    h.update(HostAction::SelectPane(a11y::PaneTab::ALL[0]));
    assert!(!h.dirty_fields().contains(&TOP_PLACE_FIELD.name));
}

#[test]
fn 履歴を開くと_top_place_が_dirty_になり復元した位置が出る() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_web::host::ImportOutcome;

    let ir = ir();
    let env = build_envelope(&ir, &ir.starts[0].id, vec![], state_at("main/st-1-4")).unwrap();
    let bytes = export_bytes(&env).unwrap();
    let mut h = host_at("main/st-1-4");
    let _ = h.dirty_fields();
    let outcome = h.open_history(&bytes);
    assert!(
        !matches!(outcome, ImportOutcome::Refused(_)),
        "前提: 履歴が開く {outcome:?}"
    );
    assert!(h.dirty_fields().contains(&TOP_PLACE_FIELD.name));
    assert_place(&h, "main/st-1-4");
}

#[test]
fn モジュールの_step_は呼び出した章の位置になる() {
    use fandhe_workshop_web::top_position::top_position;

    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-d07-flat.json",
    );
    let ir = load_ir(&std::fs::read(p).unwrap()).unwrap();
    let direct = top_position(&ir, "main/u-impl").expect("呼び出した use の Step");
    let nested = ir
        .nodes
        .iter()
        .find(|n| n.scope.contains("merge-gate@gate"))
        .expect("入れ子の節点");
    assert_eq!(top_position(&ir, &nested.uid), Some(direct));
    assert!(top_position(&ir, "unknown/uid").is_none());
}
