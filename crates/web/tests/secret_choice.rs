//! 秘密情報らしい文字列の警告の選択の試験（SAFE-28・D-44。TASK-164 3/3）。
//!
//! 確かめるのは、(a) 「このまま保存する」で 10 形それぞれが保存され、書き出しが通ること、(b) 警告の後に別の形が入ったときは
//! 見ていない値を確認済みにしないこと、(c) 確認済みの値は続く保存でも警告が出ず、新しい値には再び警告が出ること、
//! (d) 「この入力を残さない」で保存先・入力欄・画面・記録した状態のどこにも値が残らないこと（送信の後も、下書きだけの検出の後も）、
//! (e) 保存済みの履歴が無い・読めないときの読み直しが新しく始めて保存を止めること、(f) 読み直しが確認済みを空にすること、
//! (g) ボタンが警告のあるときだけ出ること、(h) action の名前が完全一致だけで復号されることの 8 点である。
//! 保存先は、saver が書き reader が読む共有の写しで模す。assert のメッセージには ID だけを入れ、値を出さない。
//! 試験では `unwrap` を使ってよい。

#[path = "common/secret_values.rs"]
mod secret_values;

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::storage::StorageError;
use fandhe_workshop_core::secret_scan::SecretShape;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::secret_notice::{encode_secret_hold, secret_hold};

type Store = Rc<RefCell<Option<String>>>;

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

/// saver が書き reader が読む保存先の写しつきのホスト。戻りの `Cell` は saver が呼ばれた回数（値は持たない）。
fn setup() -> (EngineHost, Rc<Cell<u32>>, Store) {
    let mut h = page();
    let calls = Rc::new(Cell::new(0));
    let store: Store = Rc::new(RefCell::new(None));
    let (c2, s2) = (Rc::clone(&calls), Rc::clone(&store));
    h.set_history_saver(Box::new(move |v| {
        c2.set(c2.get() + 1);
        *s2.borrow_mut() = Some(v.to_owned());
        Ok(())
    }));
    let s3 = Rc::clone(&store);
    h.set_saved_reader(Box::new(move || Ok(s3.borrow().clone())));
    (h, calls, store)
}

fn submit(h: &mut EngineHost, text: &str) {
    h.update(act("draft", text));
    h.update(act("submit", ""));
}

/// 無害な 1 手で基準の保存を作り、その保存値を返す。
fn baseline(h: &mut EngineHost, store: &Store) -> String {
    submit(h, "/make-note first-note");
    store.borrow().clone().unwrap()
}

/// 保存値 `base` から起動（`boot`）して開いた状態の保存値。読み直しの結果の期待値（復元は再読み込みとして記録される）。
fn reopened(base: &str) -> Option<String> {
    let mut h = page();
    h.boot(Some(base.as_bytes()));
    h.history_to_save()
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn count(h: &EngineHost, needle: &str) -> usize {
    html(h).matches(needle).count()
}

fn warning_ids(h: &EngineHost) -> Vec<&'static str> {
    h.model().secret_warning().iter().map(|s| s.id()).collect()
}

fn shape_of(expect: &str) -> SecretShape {
    SecretShape::from_id(expect).unwrap()
}

#[test]
fn keep_saves_every_shape() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, calls, store) = setup();
        let base = baseline(&mut h, &store);
        submit(&mut h, &value);
        assert_eq!(calls.get(), 1, "{id}: saver was called before keep");
        assert!(warning_ids(&h) == [expect.as_str()], "{id}: no warning");
        assert!(
            matches!(h.export_live(), ExportGate::Held(_)),
            "{id}: export not held"
        );
        h.update(act("secret-keep", ""));
        assert_eq!(calls.get(), 2, "{id}: keep did not save");
        assert!(h.model().secret_warning().is_empty(), "{id}: warning left");
        let saved = store.borrow().clone().unwrap();
        assert!(saved != base, "{id}: store unchanged");
        // 保存先に値が入っている（確認した値は保存される）。形は走査で確かめ、値は出さない。
        assert!(
            secret_hold(&saved) == [shape_of(&expect)],
            "{id}: saved value lacks the shape"
        );
        assert!(
            matches!(h.export_live(), ExportGate::Ready(_)),
            "{id}: export not ready"
        );
        assert_eq!(count(&h, "data-testid=\"secret-keep\""), 0, "{id}: button");
    }
}

#[test]
fn keep_after_draft_only_warning() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, calls, store) = setup();
        baseline(&mut h, &store);
        h.update(act("draft", &value));
        let payload = encode_secret_hold(h.save_epoch(), &[shape_of(&expect)]);
        h.update(act("secret-hold", &payload));
        assert!(warning_ids(&h) == [expect.as_str()], "{id}: no warning");
        let before = calls.get();
        h.update(act("secret-keep", ""));
        assert_eq!(calls.get(), before + 1, "{id}: keep did not save");
        assert!(h.model().secret_warning().is_empty(), "{id}: warning left");
        assert!(
            secret_hold(store.borrow().as_deref().unwrap()) == [shape_of(&expect)],
            "{id}: draft not saved"
        );
    }
}

#[test]
fn keep_is_ignored_without_warning() {
    let (mut h, calls, store) = setup();
    baseline(&mut h, &store);
    let before = (calls.get(), store.borrow().clone());
    h.update(act("secret-keep", ""));
    assert_eq!(calls.get(), before.0, "saver was called");
    assert!(*store.borrow() == before.1, "store changed");
    assert!(h.model().secret_warning().is_empty(), "warning appeared");
}

#[test]
fn keep_does_not_acknowledge_unseen_shapes() {
    let shapes = secret_values::shapes();
    let (a_id, a_expect, a_value) = shapes[0].clone();
    let (b_id, b_expect, b_value) = shapes[1].clone();
    assert!(a_expect != b_expect, "fixture shapes must differ");
    let (mut h, calls, store) = setup();
    baseline(&mut h, &store);
    submit(&mut h, &a_value);
    assert!(warning_ids(&h) == [a_expect.as_str()], "{a_id}: no warning");
    // 警告の後に別の形を入れる。keep は、見ていない形を確認済みにせず、警告を新しい形へ置き換える。
    h.update(act("draft", &b_value));
    let before = calls.get();
    h.update(act("secret-keep", ""));
    assert_eq!(calls.get(), before, "{b_id}: saved an unseen shape");
    let mut want = vec![a_expect.as_str(), b_expect.as_str()];
    want.sort_by_key(|id| {
        SecretShape::ALL
            .iter()
            .position(|s| s.id() == *id)
            .unwrap_or(usize::MAX)
    });
    assert!(warning_ids(&h) == want, "{b_id}: warning not replaced");
    assert!(
        matches!(h.export_live(), ExportGate::Held(_)),
        "{b_id}: export not held"
    );
    // 新しい警告を見た上で選べば、両方が保存される。
    h.update(act("secret-keep", ""));
    assert_eq!(calls.get(), before + 1, "{b_id}: second keep did not save");
    assert!(
        h.model().secret_warning().is_empty(),
        "{b_id}: warning left"
    );
}

#[test]
fn acknowledged_value_stays_saved_on_later_changes() {
    let shapes = secret_values::shapes();
    let (a_id, _, a_value) = shapes[0].clone();
    let (b_id, b_expect, b_value) = shapes[1].clone();
    let (mut h, calls, store) = setup();
    baseline(&mut h, &store);
    submit(&mut h, &a_value);
    h.update(act("secret-keep", ""));
    let before = calls.get();
    submit(&mut h, "/make-note second-note");
    assert_eq!(calls.get(), before + 1, "{a_id}: later save was held");
    assert!(h.model().secret_warning().is_empty(), "{a_id}: re-warned");
    assert!(
        matches!(h.export_live(), ExportGate::Ready(_)),
        "{a_id}: export not ready"
    );
    // 別の新しい秘密情報には再び警告が出る。
    submit(&mut h, &b_value);
    assert_eq!(calls.get(), before + 1, "{b_id}: new value was saved");
    assert!(warning_ids(&h) == [b_expect.as_str()], "{b_id}: no warning");
}

#[test]
fn discard_leaves_no_value() {
    for (id, _, value) in secret_values::shapes() {
        let (mut h, calls, store) = setup();
        let base = baseline(&mut h, &store);
        submit(&mut h, &value);
        assert!(!h.model().secret_warning().is_empty(), "{id}: no warning");
        let before = calls.get();
        h.update(act("secret-discard", ""));
        assert_eq!(calls.get(), before, "{id}: discard wrote");
        assert!(
            store.borrow().as_deref() == Some(base.as_str()),
            "{id}: store"
        );
        assert_eq!(h.draft(), "", "{id}: draft left");
        assert!(h.model().secret_warning().is_empty(), "{id}: warning left");
        let page = html(&h);
        assert!(!page.contains(&value), "{id}: value in page");
        assert!(secret_hold(&page).is_empty(), "{id}: shape in page");
        // 記録した状態は、保存先の基準から開いた状態と等しい。
        assert!(
            h.history_to_save() == reopened(&base),
            "{id}: state differs from the saved baseline"
        );
        match h.export_live() {
            ExportGate::Ready(bytes) => {
                assert!(
                    Some(String::from_utf8(bytes).unwrap()) == reopened(&base),
                    "{id}: export differs"
                );
            }
            _ => panic!("{id}: export not ready"),
        }
        assert_eq!(
            count(&h, "data-testid=\"secret-discard\""),
            0,
            "{id}: button"
        );
    }
}

#[test]
fn discard_draft_only() {
    for (id, expect, value) in secret_values::shapes() {
        let (mut h, calls, store) = setup();
        let base = baseline(&mut h, &store);
        h.update(act("draft", &value));
        let payload = encode_secret_hold(h.save_epoch(), &[shape_of(&expect)]);
        h.update(act("secret-hold", &payload));
        assert!(!h.model().secret_warning().is_empty(), "{id}: no warning");
        let before = calls.get();
        h.update(act("secret-discard", ""));
        assert_eq!(calls.get(), before, "{id}: discard wrote");
        assert_eq!(h.draft(), "", "{id}: draft left");
        let page = html(&h);
        assert!(!page.contains(&value), "{id}: value in page");
        assert!(
            h.history_to_save() == reopened(&base),
            "{id}: state differs from the saved baseline"
        );
        assert!(
            store.borrow().as_deref() == Some(base.as_str()),
            "{id}: store"
        );
    }
}

#[test]
fn discard_without_saved_history_starts_fresh() {
    let (id, _, value) = secret_values::shapes().remove(0);
    let (mut h, calls, store) = setup();
    submit(&mut h, &value);
    assert_eq!(calls.get(), 0, "{id}: saver was called");
    h.update(act("secret-discard", ""));
    assert!(store.borrow().is_none(), "{id}: store written");
    assert!(h.history_to_save().is_none(), "{id}: state not fresh");
    assert!(!html(&h).contains(&value), "{id}: value in page");
    // 新しく始めた後は、以後の操作が保存される。
    submit(&mut h, "/make-note first-note");
    assert_eq!(calls.get(), 1, "{id}: later save missing");
}

#[test]
fn discard_with_unreadable_store_starts_fresh_and_blocks_saving() {
    let (id, _, value) = secret_values::shapes().remove(0);
    let mut h = page();
    let calls = Rc::new(Cell::new(0));
    let c2 = Rc::clone(&calls);
    h.set_history_saver(Box::new(move |_| {
        c2.set(c2.get() + 1);
        Ok(())
    }));
    h.set_saved_reader(Box::new(|| Err(StorageError::Unavailable)));
    submit(&mut h, &value);
    h.update(act("secret-discard", ""));
    assert!(!html(&h).contains(&value), "{id}: value in page");
    submit(&mut h, "/make-note first-note");
    assert_eq!(calls.get(), 0, "{id}: saved after an unreadable read");
    assert!(h.history_to_save().is_none(), "{id}: saving not blocked");
}

#[test]
fn discard_clears_acknowledged() {
    let shapes = secret_values::shapes();
    let (a_id, a_expect, a_value) = shapes[0].clone();
    let (b_id, _, b_value) = shapes[1].clone();
    let (mut h, calls, store) = setup();
    baseline(&mut h, &store);
    submit(&mut h, &a_value);
    h.update(act("secret-keep", ""));
    submit(&mut h, &b_value);
    assert!(!h.model().secret_warning().is_empty(), "{b_id}: no warning");
    h.update(act("secret-discard", ""));
    // 読み直した状態は a を含む保存から作られる。a は確認済みではなくなり、保存の前に再び警告が出る。
    let before = calls.get();
    submit(&mut h, "/make-note second-note");
    assert_eq!(calls.get(), before, "{a_id}: acknowledgement survived");
    assert!(warning_ids(&h) == [a_expect.as_str()], "{a_id}: no warning");
}

#[test]
fn discard_is_ignored_without_warning() {
    let (mut h, calls, store) = setup();
    let base = baseline(&mut h, &store);
    h.update(act("draft", "typing"));
    h.update(act("secret-discard", ""));
    assert_eq!(h.draft(), "typing", "draft was reset");
    assert_eq!(calls.get(), 1, "saver was called");
    assert!(store.borrow().as_deref() == Some(base.as_str()), "store");
}

#[test]
fn buttons_appear_only_with_a_warning() {
    let (id, _, value) = secret_values::shapes().remove(0);
    let (mut h, _, store) = setup();
    baseline(&mut h, &store);
    assert_eq!(count(&h, "data-testid=\"secret-choice\""), 1, "{id}: list");
    for t in ["secret-keep", "secret-discard"] {
        assert_eq!(count(&h, &format!("data-testid=\"{t}\"")), 0, "{id}: {t}");
    }
    submit(&mut h, &value);
    assert!(
        h.dirty_fields().contains(&"secret_choice"),
        "{id}: choice not dirty"
    );
    for t in ["secret-keep", "secret-discard"] {
        assert_eq!(count(&h, &format!("data-testid=\"{t}\"")), 1, "{id}: {t}");
        assert_eq!(count(&h, &format!("data-action=\"{t}\"")), 1, "{id}: {t}");
    }
    h.update(act("secret-keep", ""));
    assert!(
        h.dirty_fields().contains(&"secret_choice"),
        "{id}: choice not dirty after keep"
    );
    assert_eq!(count(&h, "data-testid=\"secret-keep\""), 0, "{id}: left");
}

#[test]
fn controls_show_no_buttons() {
    for (id, text) in secret_values::controls() {
        let (mut h, _, _) = setup();
        submit(&mut h, &text);
        assert_eq!(count(&h, "data-testid=\"secret-keep\""), 0, "{id}: keep");
        assert_eq!(
            count(&h, "data-testid=\"secret-discard\""),
            0,
            "{id}: discard"
        );
    }
}

#[test]
fn decode_exact_names_only() {
    assert!(matches!(
        EngineHost::decode_action("secret-keep", ""),
        Some(HostAction::SecretKeep)
    ));
    assert!(matches!(
        EngineHost::decode_action("secret-discard", "ignored"),
        Some(HostAction::SecretDiscard)
    ));
    for bad in [
        "Secret-keep",
        "secret_keep",
        "secret-keep ",
        "secret-discard2",
        "secret",
    ] {
        assert!(
            EngineHost::decode_action(bad, "").is_none(),
            "{bad:?} decoded"
        );
    }
}
