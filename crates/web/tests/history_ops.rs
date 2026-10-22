//! 枠の履歴の書き出し・読み込みの操作（TASK-531 1/2。SHELL-15）の試験。
//!
//! 完成した画面が `shell`・`a11y` の検査を通ること、書き出しの 2 段（押す→リンク）、リンクを外す時機、出せない場合の固定の文、
//! 読み込みの結線を固定する。ブラウザの `Download`・`setInputFiles` は `tools/browser/history-ops.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_core::Node;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::{file_name, EMPTY_STATUS, HELD_STATUS};
use fandhe_workshop_web::host::{EngineHost, ExportLink, HostAction};
use fandhe_workshop_web::parts::{Component, DirtyTracked};
use fandhe_workshop_web::{a11y, shell};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir() -> Ir {
    load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn host() -> EngineHost {
    EngineHost::with_session(Session::from_state(ir(), initial()), standard_registry()).unwrap()
}

/// 偽の URL の口の観察。作った回数と破棄した回数を数える。
struct Probe {
    made: Rc<Cell<u32>>,
    dropped: Rc<Cell<u32>>,
}

struct Guard(Rc<Cell<u32>>);

impl Drop for Guard {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

fn with_maker(h: &mut EngineHost) -> Probe {
    let made = Rc::new(Cell::new(0));
    let dropped = Rc::new(Cell::new(0));
    let (m, d) = (Rc::clone(&made), Rc::clone(&dropped));
    h.set_export_url_maker(Box::new(move |_| {
        m.set(m.get() + 1);
        Some(ExportLink {
            url: format!("blob:test/{}", m.get()),
            guard: Box::new(Guard(Rc::clone(&d))),
        })
    }));
    Probe { made, dropped }
}

fn act(h: &mut EngineHost, name: &str, payload: &str) {
    h.update(EngineHost::decode_action(name, payload).unwrap());
}

fn go(h: &mut EngineHost) {
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-2".to_owned(),
    )))));
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

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    if let Node::Element { children, .. } = n {
        for c in children {
            find(c, testid, out);
        }
    }
}

fn all<'a>(n: &'a Node, testid: &str) -> Vec<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    v
}

fn loadable() -> String {
    let ir = ir();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    String::from_utf8(export_bytes(&built).unwrap()).unwrap()
}

#[test]
fn 完成した画面は契約を通り操作が1つずつある() {
    let v = host().view();
    shell::check_complete(&v).unwrap();
    a11y::check_no_undefined(&v).unwrap();
    assert_eq!(all(&v, "shell-export").len(), 1);
    assert_eq!(all(&v, "shell-import").len(), 1);
}

#[test]
fn 読み込みの入れ物は_payload_を持たず中の部品は_action_を持たない() {
    let v = host().view();
    let imp = all(&v, "shell-import")[0];
    assert_eq!(attr(imp, "data-action"), Some("import-history"));
    assert_eq!(attr(imp, "data-payload"), None);
    let s = format!("{imp:?}");
    assert_eq!(s.matches("data-action").count(), 1, "{s}");
    assert!(s.contains("\"file\""), "{s}");
}

#[test]
fn 書き出しを押すとリンクが1件出てもう一度押すと作り直す() {
    let mut h = host();
    let p = with_maker(&mut h);
    go(&mut h);
    act(&mut h, "export-history", "");
    assert!(h.dirty_fields().contains(&"export_result"));
    let v = h.view();
    let links = all(&v, "export-download");
    assert_eq!(links.len(), 1);
    assert_eq!(attr(links[0], "data-export-url"), Some("blob:test/1"));
    assert_eq!(
        attr(links[0], "download"),
        Some(file_name(&ir().pack.id).as_str())
    );
    shell::check_complete(&v).unwrap();
    a11y::check_no_undefined(&v).unwrap();
    act(&mut h, "export-history", "");
    assert_eq!(p.made.get(), 2);
    assert_eq!(p.dropped.get(), 1, "前のハンドルは破棄される");
    let v = h.view();
    assert_eq!(
        attr(all(&v, "export-download")[0], "data-export-url"),
        Some("blob:test/2")
    );
}

#[test]
fn 状態を変える操作の後はリンクが消え選択の切り替えでは消えない() {
    let mut h = host();
    let p = with_maker(&mut h);
    go(&mut h);
    act(&mut h, "export-history", "");
    act(&mut h, "select-pane", "artifacts");
    assert_eq!(all(&h.view(), "export-download").len(), 1);
    assert_eq!(p.dropped.get(), 0);
    act(&mut h, "draft", "/make-note x");
    assert_eq!(all(&h.view(), "export-download").len(), 0);
    assert_eq!(p.dropped.get(), 1);
    assert!(h.dirty_fields().contains(&"export_result"));
}

#[test]
fn 履歴が無いときは文だけでリンクは出ない() {
    let mut h = host();
    with_maker(&mut h);
    act(&mut h, "export-history", "");
    let v = h.view();
    assert_eq!(all(&v, "export-download").len(), 0);
    let s = format!("{:?}", all(&v, "export-status"));
    assert!(s.contains(EMPTY_STATUS), "{s}");
}

#[test]
fn 警告の間は文だけで値を含まない() {
    let mut h = host();
    with_maker(&mut h);
    go(&mut h);
    // 検知パターンに一致する値は実行時に組み立てる。
    let value = format!("{}{}", "AKIA", "IOSFODNN7EXAMPLE");
    act(&mut h, "draft", &value);
    act(&mut h, "export-history", "");
    let v = h.view();
    assert_eq!(all(&v, "export-download").len(), 0);
    let s = format!("{:?}", all(&v, "export-status"));
    assert!(s.contains(HELD_STATUS), "{s}");
    assert!(!s.contains(&value));
}

#[test]
fn url_を作る口が無ければリンクを出さない() {
    let mut h = host();
    go(&mut h);
    act(&mut h, "export-history", "");
    let v = h.view();
    assert_eq!(all(&v, "export-download").len(), 0);
    assert_eq!(all(&v, "export-status").len(), 1);
}

#[test]
fn 読み込みは開く本文で状態が変わり壊れた本文と空は変えず通知する() {
    let mut h = host();
    act(&mut h, "import-history", &loadable());
    let before = h.model().session().unwrap().state().clone();
    for bad in ["", "{not json"] {
        act(&mut h, "import-history", bad);
        assert_eq!(h.model().session().unwrap().state(), &before);
        assert_eq!(h.dirty_fields(), vec!["shell_notices"]);
    }
}

#[test]
fn 開いた履歴では前のリンクが外れる() {
    let mut h = host();
    let p = with_maker(&mut h);
    go(&mut h);
    act(&mut h, "export-history", "");
    act(&mut h, "import-history", &loadable());
    assert_eq!(all(&h.view(), "export-download").len(), 0);
    assert_eq!(p.dropped.get(), 1);
}

#[test]
fn 枠の操作の復号は書き出しと読み込みだけを受ける() {
    assert!(EngineHost::decode_action("export-history", "x").is_none());
    assert!(EngineHost::decode_action("discard-and-start", "").is_none());
    assert!(EngineHost::decode_action("import-history", "").is_some());
}
