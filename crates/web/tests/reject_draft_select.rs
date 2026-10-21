//! 拒否の後の下書きが選択状態になり連結されないことの試験（TASK-284 2/2。JUDGE-27・TERM-22。決定 D-67）。
//!
//! 拒否で残した下書きは、全体を選択状態にして次に打った文字で置き換える。native では DOM の選択を見られないため、
//! ホストが出す選択の要求の回数と、選択の保留の印（`draft_select_pending`）で確かめる。DOM の選択そのものは
//! ブラウザ試験 `reject-draft-select` が確かめる。問い返しの後は選ばない（受け付けの後は空になり選ぶ下書きが無い。ST-1.4 には受け付けになる入力が無いため、ここでは確かめない）。
//! 試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
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

/// ST-1.4 のホストと、書き戻しの要求・選択の要求の回数。
fn host() -> (EngineHost, Rc<Cell<u32>>, Rc<Cell<u32>>) {
    let mut h =
        EngineHost::with_session(Session::from_state(ir(), st14_state()), standard_registry())
            .unwrap();
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    h.set_draft_sync_request(Box::new(move || c.set(c.get() + 1)));
    let s = Rc::new(Cell::new(0));
    let c = Rc::clone(&s);
    h.set_draft_select_request(Box::new(move || c.set(c.get() + 1)));
    (h, n, s)
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

#[test]
fn 拒否の後は選択の要求が出て_下書きは送った本文のまま残り_書き戻しは要求されない() {
    let (mut h, sync, select) = host();
    type_after_and_submit(&mut h, "/advance-phase");
    assert_eq!(h.draft(), "/advance-phase");
    assert_eq!(select.get(), 1);
    assert_eq!(sync.get(), 0);
    assert!(h.draft_select_pending());
}

#[test]
fn 問い返しの後は選択の要求が出ず_次の拒否で出る() {
    let (mut h, _, select) = host();
    type_after_and_submit(&mut h, "/new-idea");
    assert_eq!(h.draft(), "");
    assert_eq!(select.get(), 0);
    assert!(!h.draft_select_pending());
    type_after_and_submit(&mut h, "/advance-phase");
    assert_eq!(select.get(), 1);
    assert!(h.draft_select_pending());
}

#[test]
fn 拒否の後に打鍵が届くと選択の予約は取り下げられる() {
    let (mut h, _, _) = host();
    type_after_and_submit(&mut h, "/advance-phase");
    assert!(h.draft_select_pending());
    h.update(act("draft", "/advance-phase/x"));
    assert!(!h.draft_select_pending());
}

#[test]
fn 拒否の後に下書きを変えない操作が来ても選択の予約は残る() {
    let (mut h, _, _) = host();
    type_after_and_submit(&mut h, "/advance-phase");
    h.update(act("draft", "/advance-phase"));
    assert!(h.draft_select_pending());
}

#[test]
fn 通知先を結ばなくても拒否で落ちない() {
    let mut h =
        EngineHost::with_session(Session::from_state(ir(), st14_state()), standard_registry())
            .unwrap();
    type_after_and_submit(&mut h, "/advance-phase");
    assert_eq!(h.draft(), "/advance-phase");
    assert!(h.draft_select_pending());
}
