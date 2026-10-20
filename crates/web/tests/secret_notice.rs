//! 秘密情報らしい文字列の警告の試験（SAFE-27・SAFE-29・TASK-164 1/3）。
//!
//! 確かめるのは、(1) 状態を変える操作の入力に 10 形を入れると、保存の口（saver）が呼ばれず、形の ID だけが
//! `secret_warning` に入り、`secret-notice` に形の名前が出て値が出ないこと、(2) 対照 6 件では警告が空で saver が呼ばれること、
//! (3) 警告の後の保存も、履歴に値が残る間は止まること、(4) `secret-hold` の action が完全一致の payload だけを受けることの 4 点である。
//! assert のメッセージには形の ID だけを入れ、値を出さない。試験では `unwrap` を使ってよい。

#[path = "common/secret_values.rs"]
mod secret_values;

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_client::BoundValue;
use fandhe_workshop_core::secret_scan::SecretShape;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};

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

fn notice(h: &EngineHost) -> String {
    match h.registry().bound_value("secret_notice", h.model()) {
        Some(BoundValue::Text(t)) => t,
        other => panic!("secret_notice の束縛が無い: {other:?}"),
    }
}

fn warning_ids(h: &EngineHost) -> Vec<&'static str> {
    h.model().secret_warning().iter().map(|s| s.id()).collect()
}

#[test]
fn every_shape_stops_the_save_and_shows_only_its_name() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, calls) = setup();
        submit(&mut h, &value);
        assert_eq!(calls.get(), 0, "{id}: saver was called");
        assert!(
            warning_ids(&h) == [expect.as_str()],
            "{id}: warning differs"
        );
        let shape = SecretShape::from_id(&expect).unwrap();
        let text = notice(&h);
        assert!(text.contains(shape.label()), "{id}: name missing");
        assert!(!text.contains(&value), "{id}: value leaked to the notice");
        assert!(
            h.dirty_fields().contains(&"secret_notice"),
            "{id}: field not dirty"
        );
    }
}

#[test]
fn controls_save_without_a_warning() {
    let controls = secret_values::controls();
    assert_eq!(controls.len(), 6);
    for (id, text) in controls {
        let (mut h, calls) = setup();
        submit(&mut h, &text);
        assert!(warning_ids(&h).is_empty(), "{id}: false positive");
        assert_eq!(notice(&h), "", "{id}: notice not empty");
        assert_eq!(calls.get(), 1, "{id}: saver call count");
    }
}

#[test]
fn later_saves_stay_held_while_the_history_holds_the_value() {
    let (mut h, calls) = setup();
    let (_, _, value) = secret_values::shapes().remove(0);
    submit(&mut h, &value);
    submit(&mut h, "/make-note second-note");
    assert_eq!(calls.get(), 0, "saver called after a held value");
    assert!(!warning_ids(&h).is_empty());
}

#[test]
fn secret_hold_action_accepts_exact_payloads_only() {
    let (mut h, _) = setup();
    h.update(act("secret-hold", "jwt"));
    assert!(warning_ids(&h) == ["jwt"]);
    assert!(notice(&h).contains(SecretShape::Jwt.label()));
    for bad in ["unknown", "jwt,", " jwt", "jwt,jwt", "private_key,jwt"] {
        assert!(
            EngineHost::decode_action("secret-hold", bad).is_none(),
            "{bad:?} was accepted"
        );
    }
    h.update(act("secret-hold", ""));
    assert!(warning_ids(&h).is_empty());
    assert_eq!(notice(&h), "");
}

#[test]
fn stale_save_result_does_not_clear_the_warning_of_a_held_value() {
    let (mut h, _) = setup();
    submit(&mut h, "/make-note first-note");
    // 無害な値の遅延保存の結果が配送待ちの間に、秘密を含む即時保存が止められる。
    let epoch_before = h.save_epoch();
    let (_, _, value) = secret_values::shapes().remove(0);
    submit(&mut h, &value);
    assert!(!warning_ids(&h).is_empty());
    h.update(HostAction::SaveResult(epoch_before, None));
    assert!(!warning_ids(&h).is_empty(), "stale result cleared warning");
}
