//! 古いタブの「保存されていない」通知欄（`stale-notice`）の試験（SAVE-13・SHELL-23・D-56・TASK-325 1/2）。
//!
//! 2 つのタブは、同じ localStorage の偽物を共有する 2 つの `BootGuard` と、それを関所にした保存先で模す（`boot_guard.rs` と同じ形）。
//! 確かめるのは、(a) 追い越される前は出ない、(b) 追い越された後の最初の保存の試みと同じ `update` で出て、保存失敗の
//! `save-notice` は空のまま、(c) 新しいタブには出ない（陽性の対照）、(d) 出た後は消えず dirty に再び積まれない、
//! (e) probe が無い・偽のホストでは出ない、(f) 文に教材の値が含まれない、(g) 照合の無い保存先では出ない（対照）の 7 点である。
//! 文面は仮置きのため、文そのものでなく「空でない」を固定する。試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_contract::storage::HistoryStore;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::boot_guard::{BootGuard, Guarded};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::storage_select::HistoryStorage;
use storage_fakes::{key, FakeIdb, FakeLocal};

type Storage = HistoryStorage<Rc<FakeLocal>, Rc<FakeIdb>>;
type Guard = BootGuard<Rc<FakeLocal>>;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn page() -> EngineHost {
    let ir = load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn drain(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 100, "進行が終わらない");
    }
}

/// 状態を変える 1 手（保存を試みる）。
fn move_once(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    drain(h);
}

fn text(h: &EngineHost, field: &str) -> String {
    match h.registry().bound_value(field, h.model()) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("{field} の束縛が無い: {other:?}"),
    }
}

fn stale_dirty(h: &EngineHost) -> bool {
    h.dirty_fields().contains(&"stale_notice")
}

/// 照合つきの保存先と probe を結んだタブ。`guarded` が偽なら照合を通さない（対照）。
fn tab(local: &Rc<FakeLocal>, tag: u32, guarded: bool, probe: bool) -> (EngineHost, Rc<Guard>) {
    let k = key("pack-a");
    let guard = Rc::new(BootGuard::claim(Rc::clone(local), &k, tag));
    let st: Rc<Storage> = Rc::new(HistoryStorage::new(
        Rc::clone(local),
        None::<Rc<FakeIdb>>,
        BTreeMap::new(),
    ));
    let mut h = page();
    assert_eq!(h.boot(None), BootKind::Fresh);
    let (g, s) = (Rc::clone(&guard), Rc::clone(&st));
    h.set_history_saver(Box::new(move |v| {
        if guarded {
            Guarded {
                inner: &*s,
                guard: &*g,
            }
            .write(&k, v)
        } else {
            s.write(&k, v)
        }
    }));
    if probe {
        let g = Rc::clone(&guard);
        h.set_stale_probe(Box::new(move || g.is_stale()));
    }
    (h, guard)
}

#[test]
fn 追い越された古いタブだけが_同じ_update_で通知を出し_保存失敗の通知は空のまま() {
    let local = Rc::new(FakeLocal::default());
    let (mut a, _ga) = tab(&local, 0xa, true, true);
    assert_eq!(text(&a, "stale_notice"), "", "起動直後は出ない");
    let (mut b, _gb) = tab(&local, 0xb, true, true);

    move_once(&mut b);
    assert_eq!(
        text(&b, "stale_notice"),
        "",
        "新しいタブには出ない（陽性の対照）"
    );
    assert!(!b.model().stale_tab());

    let _ = a.dirty_fields();
    move_once(&mut a);
    assert!(a.model().stale_tab());
    let notice = text(&a, "stale_notice");
    assert!(!notice.is_empty(), "古いタブには出る");
    assert_eq!(text(&a, "save_notice"), "", "保存失敗の表示とは別の欄");
    assert_eq!(text(&b, "stale_notice"), "");
    assert!(a.model().save_failure().is_none());
}

#[test]
fn 出た通知は消えず_dirty_にも再び積まれない() {
    let local = Rc::new(FakeLocal::default());
    let (mut a, _ga) = tab(&local, 0xa, true, true);
    let (mut b, _gb) = tab(&local, 0xb, true, true);
    move_once(&mut b);
    // 追い越された後の最初の保存の試みで出る。
    a.update(act("draft", "/make-note first-note"));
    a.update(act("submit", ""));
    assert!(stale_dirty(&a), "初めて出たときに dirty へ積む");
    let first = text(&a, "stale_notice");
    drain(&mut a);
    a.update(act("draft", "x"));
    assert!(!stale_dirty(&a), "再び積まない");
    assert_eq!(text(&a, "stale_notice"), first, "消えない");
}

#[test]
fn 照合の口が無いホストでは出ない() {
    let local = Rc::new(FakeLocal::default());
    let (mut a, _ga) = tab(&local, 0xa, true, false);
    let (mut b, _gb) = tab(&local, 0xb, true, true);
    move_once(&mut b);
    move_once(&mut a);
    assert_eq!(text(&a, "stale_notice"), "", "probe を結ばなければ出ない");
    let mut plain = page();
    plain.boot(None);
    move_once(&mut plain);
    assert_eq!(text(&plain, "stale_notice"), "");
}

#[test]
fn 通知の文に教材の値が含まれない() {
    let local = Rc::new(FakeLocal::default());
    let (mut a, _ga) = tab(&local, 0xa, true, true);
    let (mut b, _gb) = tab(&local, 0xb, true, true);
    move_once(&mut b);
    move_once(&mut a);
    let notice = text(&a, "stale_notice");
    for leak in ["pack-a", "first-note", "make-note", "fandhe-workshop/"] {
        assert!(!notice.contains(leak), "文に値が入った: {leak}");
    }
}

#[test]
fn 照合の無い保存先では通知が出ない_照合の結果に依る対照() {
    let local = Rc::new(FakeLocal::default());
    let (mut a, ga) = tab(&local, 0xa, false, true);
    let (mut b, _gb) = tab(&local, 0xb, false, true);
    move_once(&mut b);
    move_once(&mut a);
    assert!(!ga.is_stale());
    assert_eq!(text(&a, "stale_notice"), "");
}
