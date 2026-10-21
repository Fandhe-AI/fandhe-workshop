//! 問い返しの後の下書きが連結されないことの試験（TASK-284 1/2。JUDGE-27・TERM-22。決定 D-67）。
//!
//! 引数なしの `/new-idea`（ST-1.4 の問い返し）の後は入力欄を空にする。残ったままだと、続けて打った
//! `/advance-phase` が `/new-idea/advance-phase` につながり、期待した順序違い（`wrong_order`）ではなく
//! 未知のコマンドで拒否される。`clarify_lines` は `HostAction::Engine` で流すため入力欄を通らない。
//! 本試験は画面から届く `draft`・`submit` の action で流し、入力欄の値を「いまの下書き + 打った文字」
//! として模す。拒否の後に下書きを選択状態にする処理は TASK-284 2/2 の範囲で、ここでは確かめない。
//! 試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn ir() -> Ir {
    let dir = root().join("testdata/input-field-ime/test-data/ch1-pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

/// `main/st-1-4` に入った直後の状態（`clarify_lines` の `st14_state` と同じ作り方）。
fn st14_state() -> SessionState {
    let uid = StepUid("main/st-1-4".to_owned());
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

/// ST-1.4 のホストと、書き戻しの要求の回数。
fn host() -> (EngineHost, Rc<Cell<u32>>) {
    let mut h =
        EngineHost::with_session(Session::from_state(ir(), st14_state()), standard_registry())
            .unwrap();
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    h.set_draft_sync_request(Box::new(move || c.set(c.get() + 1)));
    (h, n)
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// 全選択せずに続けて打つ（欄の値 = いまの下書き + 打った文字）。送信は Enter と同じ `submit`。
fn type_after_and_submit(h: &mut EngineHost, typed: &str) {
    let value = format!("{}{}", h.draft(), typed);
    h.update(act("draft", &value));
    h.update(act("submit", ""));
}

fn judgments(h: &EngineHost) -> Vec<Judgment> {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter_map(|e| match e {
            Event::Judged { judgment, .. } => Some(judgment.clone()),
            _ => None,
        })
        .collect()
}

fn rejected_count(h: &EngineHost) -> usize {
    judgments(h)
        .iter()
        .filter(|j| matches!(j, Judgment::Rejected(_)))
        .count()
}

fn last_input(h: &EngineHost) -> Input {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::InputReceived { input, .. } => Some(input.clone()),
            _ => None,
        })
        .expect("InputReceived")
}

#[test]
fn 問い返しの後は下書きが空になり_書き戻しが要求される() {
    let (mut h, sync) = host();
    type_after_and_submit(&mut h, "/new-idea");
    assert!(matches!(judgments(&h).last(), Some(Judgment::Clarified(_))));
    assert_eq!(rejected_count(&h), 0);
    assert_eq!(h.draft(), "");
    assert_eq!(sync.get(), 1);
}

#[test]
fn 問い返しの後に続けて打った入力は順序違いの拒否になり_未知のコマンドにならない() {
    let (mut h, _) = host();
    type_after_and_submit(&mut h, "/new-idea");
    type_after_and_submit(&mut h, "/advance-phase");
    assert_eq!(
        last_input(&h),
        Input::Command("/advance-phase".to_owned()),
        "前の下書きにつながらない"
    );
    let Some(Judgment::Rejected(r)) = judgments(&h).last().cloned() else {
        panic!("拒否ではない: {:?}", judgments(&h));
    };
    assert_eq!(r.category, RejectCategory::WrongOrder);
    assert_ne!(r.category, RejectCategory::UnknownCommand);
    let cur = h.model().session().unwrap().state().run.current.clone();
    assert_eq!(cur, Some(StepUid("main/st-1-4".to_owned())));
}

#[test]
fn 問い返しの後の拒否では下書きが残り_書き戻しは増えない() {
    let (mut h, sync) = host();
    type_after_and_submit(&mut h, "/new-idea");
    let before = sync.get();
    type_after_and_submit(&mut h, "/advance-phase");
    assert_eq!(rejected_count(&h), 1);
    assert_eq!(h.draft(), "/advance-phase");
    assert_eq!(sync.get(), before);
}
