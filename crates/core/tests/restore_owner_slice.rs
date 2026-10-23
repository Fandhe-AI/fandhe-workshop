//! 復帰が持ち主の根の切り身と位置だけを戻すことの試験（MOD-9、TASK-241 1/2）。
//!
//! 復帰は IR を読まないので、手で組んだ複数の根（`r-a`・`r-b`・`r-c`。教材固有の名前は使わない）の状態で確かめる。
//! 持ち主の根の切り身と `current`・`busy`・`completed` は保存の時点に戻り、他の根の切り身は復帰の直前のまま残る。
//! 世界状態・`global_flags`・案内レベルの戻り方は暫定で、TASK-241 2/2 で確定するためここでは表明しない。
//! 性質テストは `property-test.md` の S-5 のうち切り身と位置の部分を担う。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::session::{Busy, EntryCopy, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use proptest::collection::btree_set;
use proptest::prelude::*;

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn root(s: &str) -> Root {
    Root(s.to_string())
}

fn slice(flags: &[&str], step: &str, status: StepStatus) -> Slice {
    let mut sl = Slice {
        flags: flags.iter().map(|f| (*f).to_string()).collect(),
        ..Slice::default()
    };
    sl.status.insert(uid(step), status);
    sl
}

/// 3 つの根の切り身を持ち、`r-a/x` を現在の Step とする状態。
fn base_state() -> SessionState {
    let mut st = SessionState::default();
    st.run.current = Some(uid("r-a/x"));
    st.run
        .slices
        .insert(root("r-a"), slice(&["a0"], "r-a/x", StepStatus::Entered));
    st.run
        .slices
        .insert(root("r-b"), slice(&["b0"], "r-b/x", StepStatus::Achieved));
    st.run
        .slices
        .insert(root("r-c"), slice(&["c0"], "r-c/x", StepStatus::Achieved));
    st
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn restore() -> Op {
    Op::Control(Control::Restore(CheckpointId(1)))
}

#[test]
fn restore_replaces_only_owner_slice() {
    let ir = ir();
    let (mut st, _) = apply(&ir, base_state(), save());
    let saved = st
        .checkpoints
        .get(&CheckpointId(1))
        .expect("cp")
        .run
        .clone();
    st.run
        .slices
        .insert(root("r-a"), slice(&["a1"], "r-a/x", StepStatus::Accepted));
    st.run
        .slices
        .insert(root("r-b"), slice(&["b1"], "r-b/y", StepStatus::Entered));
    st.run
        .slices
        .insert(root("r-c"), slice(&["c1"], "r-c/y", StepStatus::Skipped));
    st.run.current = Some(uid("r-b/y"));
    st.run.busy = Some(Busy { done: 1, total: 4 });
    st.run.completed = true;
    let before = st.clone();
    let (next, events) = apply(&ir, st, restore());
    assert_eq!(
        next.run.slices.get(&root("r-a")),
        saved.slices.get(&root("r-a"))
    );
    assert_eq!(
        next.run.slices.get(&root("r-b")),
        before.run.slices.get(&root("r-b"))
    );
    assert_eq!(
        next.run.slices.get(&root("r-c")),
        before.run.slices.get(&root("r-c"))
    );
    assert_eq!(next.run.current, saved.current);
    assert_eq!(next.run.busy, saved.busy);
    assert_eq!(next.run.completed, saved.completed);
    assert_eq!(next.run.generation, Generation(before.run.generation.0 + 1));
    let restored = events
        .iter()
        .filter(|e| matches!(e, Event::Restored { .. }))
        .count();
    assert_eq!(restored, 1);
}

#[test]
fn restore_removes_owner_slice_absent_in_copy() {
    let ir = ir();
    let mut st = base_state();
    st.run.slices.remove(&root("r-a"));
    let (mut st, _) = apply(&ir, st, save());
    st.run
        .slices
        .insert(root("r-a"), slice(&["a1"], "r-a/x", StepStatus::Entered));
    let (next, _) = apply(&ir, st, restore());
    assert!(!next.run.slices.contains_key(&root("r-a")));
    assert!(next.run.slices.contains_key(&root("r-b")));
}

#[test]
fn restore_with_missing_owner_falls_back_to_current_root() {
    let ir = ir();
    let (mut st, _) = apply(&ir, base_state(), save());
    st.checkpoints.get_mut(&CheckpointId(1)).expect("cp").owner = None;
    st.run
        .slices
        .insert(root("r-a"), slice(&["a1"], "r-a/x", StepStatus::Accepted));
    st.run
        .slices
        .insert(root("r-b"), slice(&["b1"], "r-b/x", StepStatus::Entered));
    let (next, _) = apply(&ir, st, restore());
    assert_eq!(
        next.run.slices.get(&root("r-a")),
        Some(&slice(&["a0"], "r-a/x", StepStatus::Entered))
    );
    assert_eq!(
        next.run.slices.get(&root("r-b")),
        Some(&slice(&["b1"], "r-b/x", StepStatus::Entered))
    );
}

#[test]
fn restore_keeps_drafts_and_hints() {
    let ir = ir();
    let (mut st, _) = apply(&ir, base_state(), save());
    st.drafts.insert(root("r-a"), "draft".to_string());
    st.hints_opened.insert(uid("r-a/x"), 2);
    let (next, _) = apply(&ir, st.clone(), restore());
    assert_eq!(next.drafts, st.drafts);
    assert_eq!(next.hints_opened, st.hints_opened);
}

/// 処理中の保存、別の根の変更、復帰、停止の順で、他の根の進行が保存時点へ巻き戻らない（保存済みの入口の写しがある場合）。
#[test]
fn stop_after_restore_keeps_other_roots() {
    let ir = ir();
    let mut st = base_state();
    st.run.busy = Some(Busy { done: 1, total: 4 });
    st.entry = Some(EntryCopy {
        run: {
            let mut r = st.run.clone();
            r.busy = None;
            r
        },
        log_len: 0,
    });
    let (mut st, _) = apply(&ir, st, save());
    st.run
        .slices
        .insert(root("r-b"), slice(&["b1"], "r-b/y", StepStatus::Entered));
    let (st, _) = apply(&ir, st, restore());
    assert_eq!(
        st.run.slices.get(&root("r-b")),
        Some(&slice(&["b1"], "r-b/y", StepStatus::Entered))
    );
    let (stopped, events) = apply(&ir, st, Op::Control(Control::Stop));
    assert!(events.iter().any(|e| matches!(e, Event::Stopped { .. })));
    assert_eq!(
        stopped.run.slices.get(&root("r-b")),
        Some(&slice(&["b1"], "r-b/y", StepStatus::Entered))
    );
    assert_eq!(
        stopped.run.slices.get(&root("r-a")),
        Some(&slice(&["a0"], "r-a/x", StepStatus::Entered))
    );
    assert!(stopped.run.busy.is_none());
}

fn flags() -> impl Strategy<Value = BTreeSet<String>> {
    btree_set("[a-z]{1,3}", 0..4)
}

#[test]
fn restore_slice_and_position_properties() {
    let ir = ir();
    let strat = (
        proptest::collection::vec(flags(), 3),
        proptest::collection::vec(flags(), 3),
        0usize..3,
        any::<bool>(),
        proptest::option::of((0u32..5, 5u32..9)),
    );
    check(
        "restore-owner-slice",
        DEFAULT_CASES,
        strat,
        |(before_save, after_save, owner_ix, done, busy)| {
            let names = ["r-a", "r-b", "r-c"];
            let mut st = SessionState::default();
            st.run.current = Some(uid(&format!("{}/x", names[owner_ix])));
            st.run.completed = done;
            st.run.busy = busy.map(|(d, t)| Busy { done: d, total: t });
            for (n, f) in names.iter().zip(&before_save) {
                let sl = Slice {
                    flags: f.clone(),
                    ..Slice::default()
                };
                st.run.slices.insert(root(n), sl);
            }
            let (mut st, _) = apply(&ir, st, save());
            let saved = st
                .checkpoints
                .get(&CheckpointId(1))
                .expect("cp")
                .run
                .clone();
            for (n, f) in names.iter().zip(&after_save) {
                let sl = Slice {
                    flags: f.clone(),
                    ..Slice::default()
                };
                st.run.slices.insert(root(n), sl);
            }
            st.run.current = Some(uid("r-c/y"));
            st.run.busy = None;
            st.run.completed = !done;
            let before = st.clone();
            let (next, _) = apply(&ir, st, restore());
            let owner = root(names[owner_ix]);
            for n in names {
                let r = root(n);
                if r == owner {
                    prop_assert_eq!(next.run.slices.get(&r), saved.slices.get(&r));
                } else {
                    prop_assert_eq!(next.run.slices.get(&r), before.run.slices.get(&r));
                }
            }
            prop_assert_eq!(&next.run.current, &saved.current);
            prop_assert_eq!(&next.run.busy, &saved.busy);
            prop_assert_eq!(next.run.completed, saved.completed);
            Ok(())
        },
    );
}
