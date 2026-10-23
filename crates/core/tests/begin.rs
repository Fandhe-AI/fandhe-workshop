//! 次の周回の開始 `Control::Begin` の試験（TASK-237 1/2、MOD-3・MOD-5）。
//!
//! 3 Step の教材（`min-3step.json`）で、成功した開始が世代を +1 して `Begun` に続けて開始の節点へ入り、
//! 不明な開始 ID は `Refused` 1 件だけで状態を変えず、処理中・入口の写しを解き、下書きとヒントを引き継ぐことを確かめる。
//! 開始条件の拒否と周回の初期状態は `crates/pack/tests/round_slice_init.rs` で確かめる。

mod common;

use common::contract_sample::{load, testdata_dir};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::{apply, Busy, EntryCopy, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

#[test]
fn begin_enters_the_start_node_and_advances_generation() {
    let ir = ir();
    let state = SessionState::default();
    let before = state.run.generation;
    let (next_state, events) = apply(&ir, state, begin("s-1"));
    let state = next_state;
    assert!(
        matches!(
            events.as_slice(),
            [Event::Begun { .. }, Event::Entered { .. }, ..]
        ),
        "{events:?}"
    );
    let Some(Event::Begun {
        start,
        new_generation,
        ..
    }) = events.first()
    else {
        panic!("begun");
    };
    assert_eq!(start, "s-1");
    assert_eq!(Some(*new_generation), before.next());
    assert_eq!(state.run.generation, *new_generation);
    assert_eq!(state.run.current, Some(StepUid("main/st-1".into())));
    let u = StepUid("main/st-1".into());
    assert_eq!(
        state
            .run
            .slices
            .get(&u.root())
            .and_then(|s| s.status.get(&u)),
        Some(&StepStatus::Entered)
    );
}

#[test]
fn begin_with_unknown_id_is_refused_and_changes_nothing() {
    let ir = ir();
    let state = SessionState::default();
    let run = state.run.clone();
    let (next_state, events) = apply(&ir, state, begin("nope"));
    let state = next_state;
    assert!(
        matches!(
            events.as_slice(),
            [Event::Refused {
                op: ControlKind::Begin,
                reason: RefuseReason::UnknownStart,
                ..
            }]
        ),
        "{events:?}"
    );
    assert_eq!(state.run, run);
    assert!(state.entry.is_none());
}

#[test]
fn begin_clears_busy_and_entry_and_keeps_drafts_and_hints() {
    let ir = ir();
    let state = SessionState::default();
    let (mut state, _) = apply(&ir, state, begin("s-1"));
    state.run.busy = Some(Busy { total: 3, done: 1 });
    state.entry = Some(EntryCopy {
        run: state.run.clone(),
        log_len: 0,
    });
    state.drafts.insert(Root("main".into()), "draft".into());
    state.hints_opened.insert(StepUid("main/st-1".into()), 1);
    let (drafts, hints) = (state.drafts.clone(), state.hints_opened.clone());
    let (state, _) = apply(&ir, state, begin("s-1"));
    assert!(state.run.busy.is_none());
    assert!(state.entry.is_none());
    assert_eq!(state.drafts, drafts);
    assert_eq!(state.hints_opened, hints);
}

#[test]
fn three_entrances_agree() {
    let ir = ir();
    let (a, ev_a) = apply(&ir, SessionState::default(), begin("s-1"));
    let mut s = Session::new(ir.clone());
    let ev_s = s.apply(begin("s-1")).to_vec();
    assert_eq!(ev_a, ev_s);
    assert_eq!(&a, s.state());
}
