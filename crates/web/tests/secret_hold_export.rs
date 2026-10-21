//! 警告の間の書き出しの停止の試験（SAFE-27・SAFE-28・D-44。TASK-164 2/3）。
//!
//! 確かめるのは、(a) 10 形の送信で `export_live()` が `Held` になり saver が呼ばれないこと、
//! (b) 遅延保存の検出が届く前の下書きだけの値も `export_live()` が自分で走査して止めること、
//! (c) 対照 6 件は `Ready` で、バイト列が保存する値と一致すること、(d) 警告の後の進行も止まったままであること、
//! (e) 未操作は `Empty` であること、(f) `Held` の表示に値が含まれないことの 6 点である。
//! assert のメッセージには ID だけを入れ、値を出さない。試験では `unwrap` を使ってよい。

#[path = "common/secret_values.rs"]
mod secret_values;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_core::secret_scan::SecretShape;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::secret_notice::secret_notice_text;

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

/// saver つきのホスト。戻りの `Cell` は saver が呼ばれた回数（値は持たない）。
fn setup() -> (EngineHost, Rc<Cell<u32>>) {
    let mut h = page();
    let calls = Rc::new(Cell::new(0));
    let c2 = Rc::clone(&calls);
    h.set_history_saver(Box::new(move |_| {
        c2.set(c2.get() + 1);
        Ok(())
    }));
    (h, calls)
}

fn submit(h: &mut EngineHost, text: &str) {
    h.update(act("draft", text));
    h.update(act("submit", ""));
}

#[test]
fn every_shape_holds_the_export() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, calls) = setup();
        submit(&mut h, &value);
        assert_eq!(calls.get(), 0, "{id}: saver was called");
        let want = SecretShape::from_id(&expect).unwrap();
        assert!(
            h.export_live() == ExportGate::Held(vec![want]),
            "{id}: gate differs"
        );
    }
}

#[test]
fn draft_only_value_is_held_before_the_deferred_detection_arrives() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, _) = setup();
        // 一度送信して履歴を作り、その後に下書きだけへ入れる。遅延保存の検出は届かない。
        submit(&mut h, "/make-note first-note");
        h.update(act("draft", &value));
        assert!(h.model().secret_warning().is_empty(), "{id}: warned early");
        let want = SecretShape::from_id(&expect).unwrap();
        assert!(
            h.export_live() == ExportGate::Held(vec![want]),
            "{id}: gate differs"
        );
    }
}

#[test]
fn controls_are_ready_and_match_the_saved_value() {
    for (id, text) in secret_values::controls() {
        let (mut h, calls) = setup();
        submit(&mut h, &text);
        assert_eq!(calls.get(), 1, "{id}: saver call count");
        let saved = h.history_to_save().unwrap();
        match h.export_live() {
            ExportGate::Ready(bytes) => {
                assert!(bytes == saved.into_bytes(), "{id}: bytes differ");
            }
            _ => panic!("{id}: gate not ready"),
        }
    }
}

#[test]
fn progress_after_a_warning_stays_unsaved_and_unexported() {
    let (mut h, calls) = setup();
    let (_, _, value) = secret_values::shapes().remove(0);
    submit(&mut h, &value);
    submit(&mut h, "/make-note second-note");
    assert_eq!(calls.get(), 0, "saver called after a held value");
    assert!(matches!(h.export_live(), ExportGate::Held(_)));
}

#[test]
fn an_untouched_page_has_nothing_to_export() {
    let (h, _) = setup();
    assert!(h.export_live() == ExportGate::Empty);
}

#[test]
fn held_output_carries_no_value() {
    for (id, _, value) in secret_values::shapes() {
        let (mut h, _) = setup();
        submit(&mut h, &value);
        let ExportGate::Held(shapes) = h.export_live() else {
            panic!("{id}: gate not held");
        };
        assert!(
            !format!("{shapes:?}").contains(&value),
            "{id}: debug leaked"
        );
        assert!(
            !secret_notice_text(&shapes).contains(&value),
            "{id}: notice leaked"
        );
    }
}
