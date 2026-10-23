//! 復帰が持ち主の根の切り身と位置だけを戻すことの試験（MOD-9、TASK-241 1/2）。
//!
//! 復帰は IR を読まないので、手で組んだ複数の根（`r-a`・`r-b`・`r-c`。教材固有の名前は使わない）の状態で確かめる。
//! 持ち主の根の切り身と `current`・`busy`・`completed` は保存の時点に戻り、他の根の切り身は復帰の直前のまま残る。
//! 世界状態は保存の後に持ち主が書いた経路（ログの `Changed` の宛先の持ち主で決める）だけが写しへ戻り、
//! `global_flags` は復帰の直前のまま、案内レベルは写しの値に戻る（TASK-241 2/2）。写しのログ位置がログの長さを超えるときは
//! 固定文の `Warning` だけで状態を変えない（core の fail-closed。`Projection` は同じ状況で写し無しに倒す fail-soft）。
//! 性質テストは `property-test.md` の S-5 のうち切り身・位置・世界状態の部分を担う。
//! 呼び出しの Step の持ち主の試験は `restore_call_step_owner.rs`。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::session::{Busy, EntryCopy, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
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

fn el(v: &str) -> Element {
    let mut e = Element::default();
    e.attrs.insert("v".to_string(), v.to_string());
    e
}

/// `step` を宛先とする `Changed` を `st.log` へ積み、`st.run.world` を書き換える（`apply` が作る形の再現）。
fn write(st: &mut SessionState, step: &str, path: &str, value: Option<&str>) {
    let before = st.run.world.get(path).cloned();
    let after = value.map(el);
    match &after {
        Some(a) => {
            st.run.world.insert(path.to_string(), a.clone());
        }
        None => {
            st.run.world.remove(path);
        }
    }
    st.log.push(Event::Changed {
        at: Stamp {
            generation: st.run.generation,
            step: uid(step),
        },
        path: path.to_string(),
        before,
        after,
        cause: Cause::Input,
    });
}

#[test]
fn restore_reverts_only_paths_written_by_owner() {
    let ir = ir();
    let mut st = base_state();
    st.run.world.insert("pa".into(), el("0"));
    st.run.world.insert("pa-del".into(), el("0"));
    st.run.world.insert("pb".into(), el("0"));
    st.run.world.insert("pz".into(), el("0"));
    let (mut st, _) = apply(&ir, st, save());
    write(&mut st, "r-a/x", "pa", Some("1"));
    write(&mut st, "r-a/x", "pa-new", Some("1"));
    write(&mut st, "r-a/x", "pa-del", None);
    write(&mut st, "r-b/y", "pb", Some("1"));
    let (next, _) = apply(&ir, st, restore());
    assert_eq!(next.run.world.get("pa"), Some(&el("0")));
    assert_eq!(next.run.world.get("pa-del"), Some(&el("0")));
    assert!(!next.run.world.contains_key("pa-new"));
    assert_eq!(next.run.world.get("pb"), Some(&el("1")));
    assert_eq!(next.run.world.get("pz"), Some(&el("0")));
}

#[test]
fn restore_reverts_shared_path_written_by_owner_and_other() {
    let ir = ir();
    let mut st = base_state();
    st.run.world.insert("p".into(), el("0"));
    let (mut st, _) = apply(&ir, st, save());
    write(&mut st, "r-a/x", "p", Some("1"));
    write(&mut st, "r-b/y", "p", Some("2"));
    let (next, _) = apply(&ir, st, restore());
    assert_eq!(next.run.world.get("p"), Some(&el("0")));
}

#[test]
fn restore_ignores_changes_before_save_and_unstamped() {
    let ir = ir();
    let mut st = base_state();
    write(&mut st, "r-a/x", "before", Some("1"));
    let (mut st, _) = apply(&ir, st, save());
    write(&mut st, "", "unstamped", Some("1"));
    let (next, _) = apply(&ir, st, restore());
    assert_eq!(next.run.world.get("before"), Some(&el("1")));
    assert_eq!(next.run.world.get("unstamped"), Some(&el("1")));
}

#[test]
fn restore_keeps_global_flags_and_returns_guidance_level() {
    let ir = ir();
    let (mut st, _) = apply(&ir, base_state(), save());
    st.run.global_flags.insert("g".to_string());
    st.run.guidance_level = GuidanceLevel::B;
    let (next, _) = apply(&ir, st, restore());
    assert!(next.run.global_flags.contains("g"));
    assert_eq!(next.run.guidance_level, GuidanceLevel::A);
}

#[test]
fn restore_with_log_len_beyond_log_warns_without_change() {
    let ir = ir();
    let (mut st, _) = apply(&ir, base_state(), save());
    st.checkpoints
        .get_mut(&CheckpointId(1))
        .expect("cp")
        .log_len = 1000;
    st.run.world.insert("p".into(), el("1"));
    let before = st.run.clone();
    let (next, events) = apply(&ir, st, restore());
    assert_eq!(next.run, before);
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], Event::Warning { .. }));
}

#[test]
fn restore_world_and_flags_properties() {
    let ir = ir();
    // 経路 0..6 ごとに、保存の後に書いた主体（0 なし・1 持ち主・2 他の根・3 両方）と書く値の有無を決める。
    let strat = (
        proptest::collection::vec(0u8..4, 6),
        proptest::collection::vec(proptest::option::of(0u8..3), 6),
        proptest::collection::vec(proptest::option::of(0u8..3), 6),
        flags(),
        any::<bool>(),
    );
    check(
        "restore-owner-world",
        DEFAULT_CASES,
        strat,
        |(writers, saved_vals, new_vals, gflags, level_b)| {
            let path = |i: usize| format!("p{i}");
            let mut st = base_state();
            for (i, v) in saved_vals.iter().enumerate() {
                if let Some(v) = v {
                    st.run.world.insert(path(i), el(&v.to_string()));
                }
            }
            let (mut st, _) = apply(&ir, st, save());
            for (i, w) in writers.iter().enumerate() {
                let nv = new_vals[i].map(|v| format!("n{v}"));
                if *w == 1 || *w == 3 {
                    write(&mut st, "r-a/x", &path(i), nv.as_deref());
                }
                if *w == 2 || *w == 3 {
                    write(&mut st, "r-b/y", &path(i), nv.as_deref());
                }
            }
            st.run.global_flags = gflags.iter().cloned().collect();
            if level_b {
                st.run.guidance_level = GuidanceLevel::B;
            }
            let before = st.clone();
            let (next, _) = apply(&ir, st, restore());
            for (i, w) in writers.iter().enumerate() {
                let p = path(i);
                if *w == 1 || *w == 3 {
                    let want = saved_vals[i].map(|v| el(&v.to_string()));
                    prop_assert_eq!(next.run.world.get(&p), want.as_ref());
                } else {
                    prop_assert_eq!(next.run.world.get(&p), before.run.world.get(&p));
                }
            }
            prop_assert_eq!(&next.run.global_flags, &before.run.global_flags);
            prop_assert_eq!(next.run.guidance_level, GuidanceLevel::A);
            Ok(())
        },
    );
}
