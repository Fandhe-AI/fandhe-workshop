//! ブラウザの案内（TASK-544。SHELL-30・D-74）の host の試験。表示・閉じる・閉じた記録・失敗時を固定する。
//! 失敗の文には件数と真偽だけを出す。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_contract::storage::StorageError;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::browser_notice::{BROWSER_NOTICE_KEY, DISMISSED};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::shell;
use fandhe_workshop_web::storage_select::SyncKv;
use storage_fakes::FakeLocal;

/// 共有できる偽の保存先（host が所有する箱の中身を試験からも見る）。
struct Shared(Rc<FakeLocal>);

impl SyncKv for Shared {
    fn get(&self, k: &str) -> Result<Option<String>, StorageError> {
        self.0.get(k)
    }
    fn set(&self, k: &str, v: &str) -> Result<(), StorageError> {
        self.0.set(k, v)
    }
    fn remove(&self, k: &str) -> Result<(), StorageError> {
        self.0.remove(k)
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        self.0.keys()
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        self.0.total_utf16()
    }
}

fn host() -> EngineHost {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let ir = load_ir(
        &std::fs::read(dir.join("engine-event-consistency/test-data/min-3step.json")).unwrap(),
    )
    .unwrap();
    let v: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("contract/event-sequence.json")).unwrap())
            .unwrap();
    let initial: SessionState = serde_json::from_value(v["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial), standard_registry()).unwrap()
}

fn count(n: &Node, kind: &str) -> usize {
    match n {
        Node::Element {
            attrs, children, ..
        } => {
            let is_item = attrs
                .iter()
                .any(|(k, v)| k == "data-testid" && v == shell::NOTICE_ITEM_TESTID);
            let is_kind = attrs
                .iter()
                .any(|(k, v)| k == shell::NOTICE_KIND_ATTR && v == kind);
            usize::from(is_item && is_kind) + children.iter().map(|c| count(c, kind)).sum::<usize>()
        }
        _ => 0,
    }
}

fn shown(h: &EngineHost) -> usize {
    count(&h.view(), "browser-notice")
}

fn dismiss(h: &mut EngineHost, kind: &str) {
    h.update(EngineHost::decode_action("dismiss-notice", kind).unwrap());
}

fn with_store(show: bool) -> (EngineHost, Rc<FakeLocal>) {
    let store = Rc::new(FakeLocal::default());
    let mut h = host();
    h.set_browser_notice(show, Some(Box::new(Shared(store.clone()))));
    (h, store)
}

#[test]
fn default_has_no_notice() {
    assert_eq!(shown(&host()), 0);
}

#[test]
fn shows_once_and_dismiss_records() {
    let (mut h, store) = with_store(true);
    assert_eq!(shown(&h), 1);
    dismiss(&mut h, "browser-notice");
    assert_eq!(shown(&h), 0);
    assert!(h.dirty_fields().contains(&"shell_notices"));
    assert_eq!(store.raw(BROWSER_NOTICE_KEY).as_deref(), Some(DISMISSED));
    assert_eq!(store.map.borrow().len(), 1);
}

#[test]
fn does_not_come_back_after_other_actions() {
    let (mut h, _s) = with_store(true);
    dismiss(&mut h, "browser-notice");
    h.update(HostAction::SelectPane(
        fandhe_workshop_web::a11y::PaneTab::Artifacts,
    ));
    dismiss(&mut h, "version-converted");
    assert_eq!(shown(&h), 0);
}

#[test]
fn dismiss_when_hidden_writes_nothing() {
    let (mut h, store) = with_store(false);
    dismiss(&mut h, "browser-notice");
    assert!(store.map.borrow().is_empty());
}

#[test]
fn other_kinds_do_not_close_it() {
    let (mut h, store) = with_store(true);
    dismiss(&mut h, "version-converted");
    dismiss(&mut h, "save-failed");
    assert_eq!(shown(&h), 1);
    assert!(store.map.borrow().is_empty());
}

#[test]
fn write_failure_still_closes() {
    let (mut h, store) = with_store(true);
    store.limit.set(Some(0));
    dismiss(&mut h, "browser-notice");
    assert_eq!(shown(&h), 0);
    assert!(store.map.borrow().is_empty());
}

#[test]
fn closes_without_a_store() {
    let mut h = host();
    h.set_browser_notice(true, None);
    assert_eq!(shown(&h), 1);
    dismiss(&mut h, "browser-notice");
    assert_eq!(shown(&h), 0);
}
