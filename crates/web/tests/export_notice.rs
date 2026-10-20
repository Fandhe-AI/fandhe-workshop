//! 履歴の書き出しの案内（`export-notice`）の試験（SAVE-29・D-57・TASK-340）。
//!
//! 確かめるのは、(1) 保存済みの履歴が無い起動（Fresh）の最初の状態を変える操作で「初回の保存時」の案内が出て 2 手目では動かないこと、
//! (2) 下書きだけの変更・復元（Restored）では出ないこと、(3) 章の最後の Step の達成で「章の終わり」の案内が出ること、
//! (4) 復元した履歴の章の終わりは起動の直後には出ず、履歴の差し替えで消えること、(5) 文に教材の値が含まれないことの 5 点である。
//! 文面・回数は仮置きのため、文そのものでなく「空でない・種類で違う」を固定する。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::export_notice::ExportHint;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};

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

/// st-1 の 1 手（状態を変える）。
fn first_move(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    drain(h);
}

/// st-1 から st-3 の承認まで進める。
fn finish_chapter(h: &mut EngineHost) {
    first_move(h);
    h.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-files".to_owned(),
        path: "notes/draft".to_owned(),
    })));
    drain(h);
    h.update(act("choose", "opt-memo"));
    drain(h);
    for _ in 0..4 {
        h.update(act("decide", "approve"));
        drain(h);
    }
}

fn notice(h: &EngineHost) -> String {
    match h.registry().bound_value("export_notice", h.model()) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("export_notice の束縛が無い: {other:?}"),
    }
}

fn dirty(h: &EngineHost) -> bool {
    h.dirty_fields().contains(&"export_notice")
}

#[test]
fn 新しい起動の最初の操作で初回の案内が出て_二手目では動かない() {
    let mut h = page();
    assert_eq!(h.boot(None), BootKind::Fresh);
    assert_eq!(notice(&h), "");
    h.update(act("draft", "/make-note first-note"));
    assert_eq!(notice(&h), "", "下書きだけでは出ない");
    h.update(act("submit", ""));
    assert_eq!(h.model().export_hint(), Some(ExportHint::FirstSave));
    let first = notice(&h);
    assert!(!first.is_empty());
    assert!(dirty(&h));
    drain(&mut h);
    h.update(act("choose", "opt-memo"));
    assert_eq!(notice(&h), first);
    assert!(!dirty(&h));
}

#[test]
fn 保存済みを復元した起動では操作しても初回の案内は出ない() {
    let mut a = page();
    a.boot(None);
    first_move(&mut a);
    let saved = a.history_to_save().unwrap();
    let mut b = page();
    assert_eq!(b.boot(Some(saved.as_bytes())), BootKind::Restored);
    assert_eq!(notice(&b), "");
    b.update(act("choose", "opt-memo"));
    assert_eq!(notice(&b), "");
    assert_eq!(b.model().export_hint(), None);
}

#[test]
fn 保存を試せない起動でも最初の操作で案内が出る() {
    // 読めない保存先は `boot(None)` で Fresh になる（persist が続けて保存を止める）。案内は保存の可否に依らない。
    let mut h = page();
    assert_eq!(h.boot(None), BootKind::Fresh);
    first_move(&mut h);
    assert_eq!(h.model().export_hint(), Some(ExportHint::FirstSave));
}

#[test]
fn 章の最後の_step_の達成で章の終わりの案内になる() {
    let mut h = page();
    h.boot(None);
    finish_chapter(&mut h);
    assert_eq!(h.model().export_hint(), Some(ExportHint::ChapterEnd));
    let end = notice(&h);
    assert!(!end.is_empty());
    h.update(act("draft", ""));
    assert_eq!(notice(&h), end);
}

#[test]
fn 復元した章の終わりは起動の直後に出ず_履歴の差し替えで案内が消える() {
    let mut a = page();
    a.boot(None);
    finish_chapter(&mut a);
    let saved = a.history_to_save().unwrap();
    let mut b = page();
    assert_eq!(b.boot(Some(saved.as_bytes())), BootKind::Restored);
    assert_eq!(notice(&b), "");

    let mut c = page();
    c.boot(None);
    first_move(&mut c);
    assert_ne!(notice(&c), "");
    c.open_history(saved.as_bytes());
    assert_eq!(notice(&c), "", "差し替えた履歴では案内を持ち越さない");
}

#[test]
fn 途中の_step_の達成では章の終わりの案内にならない() {
    let mut h = page();
    h.boot(None);
    first_move(&mut h);
    h.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-files".to_owned(),
        path: "notes/draft".to_owned(),
    })));
    drain(&mut h);
    let log = h.model().session().unwrap().log();
    assert!(
        log.iter()
            .any(|e| matches!(e, fandhe_workshop_core::event::Event::Achieved { .. })),
        "st-1 が達成になっている"
    );
    assert_eq!(h.model().export_hint(), Some(ExportHint::FirstSave));
}

#[test]
fn 文に教材の値が含まれない() {
    let mut h = page();
    h.boot(None);
    first_move(&mut h);
    let t = notice(&h);
    for v in ["note", "min-3step", "ノート"] {
        assert!(!t.contains(v), "{v}");
    }
}
