//! 停止が持ち主の根の切り身・位置と、持ち主が写しの後に書いた世界状態の経路だけを戻すことの試験（MOD-10、TASK-242 2/2）。
//!
//! 停止は教材から実体の表だけを読むので、手で組んだ複数の根（`r-a`・`r-b`・`r-c`。教材固有の名前は使わない）の状態で
//! 確かめる。他の根の切り身・他の根が書いた経路・`global_flags` は停止の直前のまま残り、持ち主の分と位置・
//! 案内レベルは写しの値へ戻る。写しのログ位置がログの長さを超えるときは固定文の `Warning` だけで状態を変えない。
//! 性質テストは `property-test.md` の M-1 のうち、停止 1 回に絞った部分を担う（操作列全体の縦断は TASK-243）。

mod common;

use std::collections::BTreeSet;

use common::contract_sample::{load, testdata_dir};
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::{Busy, EntryCopy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{apply, load_ir};
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

fn el(v: &str) -> Element {
    let mut e = Element::default();
    e.attrs.insert("v".to_string(), v.to_string());
    e
}

fn slice(flag: &str, step: &str, status: StepStatus) -> Slice {
    let mut sl = Slice {
        flags: BTreeSet::from([flag.to_string()]),
        ..Slice::default()
    };
    sl.status.insert(uid(step), status);
    sl
}

fn stop() -> Op {
    Op::Control(Control::Stop)
}

fn stamp(step: &str) -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: uid(step),
    }
}

fn changed(step: &str, path: &str) -> Event {
    Event::Changed {
        at: stamp(step),
        path: path.to_string(),
        before: None,
        after: None,
        cause: Cause::Input,
    }
}

/// 入口の時点の実行状態（`r-a/x` を現在の Step とし、3 つの根が切り身を持つ）。
fn copy_run() -> RunState {
    let mut r = RunState {
        current: Some(uid("r-a/x")),
        ..RunState::default()
    };
    r.slices
        .insert(root("r-a"), slice("a0", "r-a/x", StepStatus::Entered));
    r.slices
        .insert(root("r-b"), slice("b0", "r-b/x", StepStatus::Entered));
    r.slices
        .insert(root("r-c"), slice("c0", "r-c/x", StepStatus::Entered));
    r.world.insert("pa".into(), el("0"));
    r.world.insert("pb".into(), el("0"));
    r.world.insert("pe".into(), el("0"));
    r.world.insert("pz".into(), el("0"));
    r.global_flags.insert("g0".into());
    r.guidance_level = GuidanceLevel::B;
    r
}

/// 停止の直前の状態。写しの後に、持ち主・他の根・宛先なしがそれぞれ経路を書いている。
fn stopping_state() -> SessionState {
    let mut st = SessionState {
        entry: Some(EntryCopy {
            run: copy_run(),
            log_len: 1,
        }),
        ..SessionState::default()
    };
    st.log = vec![
        // `log_len` より前の持ち主の `Changed` は戻す対象に入らない。
        changed("r-a/x", "pz"),
        changed("r-a/x", "pa"),
        changed("r-a/x", "pa-new"),
        changed("r-b/y", "pb"),
        Event::Changed {
            at: Stamp {
                generation: Generation::INITIAL,
                step: uid(""),
            },
            path: "pe".to_string(),
            before: None,
            after: None,
            cause: Cause::Input,
        },
    ];
    let mut run = copy_run();
    run.busy = Some(Busy { done: 1, total: 4 });
    run.completed = true;
    run.slices
        .insert(root("r-a"), slice("a1", "r-a/x", StepStatus::Accepted));
    run.slices
        .insert(root("r-b"), slice("b1", "r-b/y", StepStatus::Achieved));
    run.slices
        .insert(root("r-c"), slice("c1", "r-c/y", StepStatus::Skipped));
    run.world.insert("pa".into(), el("1"));
    run.world.insert("pa-new".into(), el("1"));
    run.world.insert("pb".into(), el("1"));
    run.world.insert("pe".into(), el("1"));
    run.world.insert("pz".into(), el("1"));
    run.global_flags.insert("g1".into());
    run.guidance_level = GuidanceLevel::A;
    st.run = run;
    st
}

#[test]
fn stop_keeps_other_root_slices() {
    let ir = ir();
    let st = stopping_state();
    let before = st.run.clone();
    let (next, _) = apply(&ir, st, stop());
    assert_eq!(
        next.run.slices.get(&root("r-a")),
        copy_run().slices.get(&root("r-a"))
    );
    assert_eq!(
        next.run.slices.get(&root("r-b")),
        before.slices.get(&root("r-b"))
    );
    assert_eq!(
        next.run.slices.get(&root("r-c")),
        before.slices.get(&root("r-c"))
    );
}

#[test]
fn stop_reverts_only_paths_written_by_owner() {
    let ir = ir();
    let (next, _) = apply(&ir, stopping_state(), stop());
    assert_eq!(next.run.world.get("pa"), Some(&el("0")));
    // 写しに無い経路は取り除く。
    assert!(!next.run.world.contains_key("pa-new"));
    // 他の根・宛先なしが書いた経路は停止の直前のまま。
    assert_eq!(next.run.world.get("pb"), Some(&el("1")));
    assert_eq!(next.run.world.get("pe"), Some(&el("1")));
    // 写しのログ位置より前の変更は戻す対象に入らない。
    assert_eq!(next.run.world.get("pz"), Some(&el("1")));
}

#[test]
fn stop_restores_position_and_bumps_generation() {
    let ir = ir();
    let st = stopping_state();
    let gen = st.run.generation;
    let (next, events) = apply(&ir, st, stop());
    assert_eq!(next.run.current, Some(uid("r-a/x")));
    assert!(!next.run.completed);
    assert!(next.run.busy.is_none());
    assert_eq!(next.run.generation, Generation(gen.0 + 1));
    let stopped = events
        .iter()
        .filter(|e| matches!(e, Event::Stopped { .. }))
        .count();
    assert_eq!(stopped, 1);
}

#[test]
fn stop_keeps_global_flags_and_restores_guidance_level() {
    let ir = ir();
    let (next, _) = apply(&ir, stopping_state(), stop());
    assert!(next.run.global_flags.contains("g1"));
    assert!(next.run.global_flags.contains("g0"));
    assert_eq!(next.run.guidance_level, GuidanceLevel::B);
}

#[test]
fn stop_removes_owner_slice_absent_in_copy() {
    let ir = ir();
    let mut st = stopping_state();
    if let Some(e) = st.entry.as_mut() {
        e.run.slices.remove(&root("r-a"));
    }
    let (next, _) = apply(&ir, st, stop());
    assert!(!next.run.slices.contains_key(&root("r-a")));
    assert!(next.run.slices.contains_key(&root("r-b")));
}

/// 状態の写しが無く、ログから導く場合でも、他の実体が書いた経路と切り身は巻き戻らない。
#[test]
fn derived_entry_keeps_other_root_writes() {
    let ir = ir();
    let mut st = SessionState::default();
    let mut run = copy_run();
    run.busy = Some(Busy { done: 1, total: 4 });
    run.slices
        .insert(root("r-a"), slice("a0", "r-a/x", StepStatus::Accepted));
    run.slices
        .insert(root("r-b"), slice("b1", "r-b/y", StepStatus::Achieved));
    run.world.insert("pa".into(), el("1"));
    run.world.insert("pb".into(), el("1"));
    st.run = run;
    let ch = |step: &str, path: &str| Event::Changed {
        at: stamp(step),
        path: path.to_string(),
        before: Some(el("0")),
        after: Some(el("1")),
        cause: Cause::Input,
    };
    st.log = vec![
        Event::InputReceived {
            at: stamp("r-a/x"),
            input: Input::Cancel,
        },
        Event::Accepted { at: stamp("r-a/x") },
        ch("r-a/x", "pa"),
        ch("r-b/y", "pb"),
    ];
    let (next, events) = apply(&ir, st, stop());
    assert!(events.iter().any(|e| matches!(e, Event::Stopped { .. })));
    assert_eq!(next.run.world.get("pa"), Some(&el("0")));
    assert_eq!(next.run.world.get("pb"), Some(&el("1")));
    assert_eq!(
        next.run.slices.get(&root("r-b")),
        Some(&slice("b1", "r-b/y", StepStatus::Achieved))
    );
    assert_eq!(
        next.run
            .slices
            .get(&root("r-a"))
            .map(|s| s.status.get(&uid("r-a/x"))),
        Some(Some(&StepStatus::Entered))
    );
}

#[test]
fn out_of_range_log_position_is_fail_closed() {
    let ir = ir();
    let mut st = stopping_state();
    if let Some(e) = st.entry.as_mut() {
        e.log_len = 99;
    }
    let before = st.clone();
    let (next, events) = apply(&ir, st, stop());
    assert_eq!(events.len(), 1, "{events:?}");
    assert!(matches!(events[0], Event::Warning { .. }));
    assert_eq!(next.run, before.run);
}

/// 根が 1 つなら、持ち主の経路だけを戻す結果は全体を戻す結果と一致する。
#[test]
fn single_root_matches_whole_copy() {
    let ir = ir();
    let mut copy = RunState {
        current: Some(uid("r-a/x")),
        ..RunState::default()
    };
    copy.slices
        .insert(root("r-a"), slice("a0", "r-a/x", StepStatus::Entered));
    copy.world.insert("pa".into(), el("0"));
    let mut st = SessionState {
        entry: Some(EntryCopy {
            run: copy.clone(),
            log_len: 0,
        }),
        ..SessionState::default()
    };
    st.log = vec![changed("r-a/x", "pa"), changed("r-a/x", "pn")];
    let mut run = copy.clone();
    run.busy = Some(Busy { done: 1, total: 2 });
    run.world.insert("pa".into(), el("1"));
    run.world.insert("pn".into(), el("1"));
    st.run = run;
    let gen = st.run.generation;
    let (next, _) = apply(&ir, st, stop());
    let mut expected = copy;
    expected.generation = Generation(gen.0 + 1);
    assert_eq!(next.run, expected);
}

#[test]
fn stop_owner_properties() {
    let ir = ir();
    let strat = (
        proptest::collection::vec(0u8..3, 1..8),
        proptest::collection::vec(any::<bool>(), 3),
        0usize..3,
    );
    check(
        "stop-owner-slice",
        DEFAULT_CASES,
        strat,
        |(writers, differ, owner_ix)| {
            let names = ["r-a", "r-b", "r-c"];
            let owner = names[owner_ix];
            let mut copy = RunState {
                current: Some(uid(&format!("{owner}/x"))),
                ..RunState::default()
            };
            let mut run = copy.clone();
            run.busy = Some(Busy { done: 1, total: 2 });
            let mut log = Vec::new();
            for (n, d) in names.iter().zip(&differ) {
                copy.slices
                    .insert(root(n), slice("c", &format!("{n}/x"), StepStatus::Entered));
                let now = if *d { "n" } else { "c" };
                run.slices
                    .insert(root(n), slice(now, &format!("{n}/y"), StepStatus::Achieved));
            }
            for (i, w) in writers.iter().enumerate() {
                let path = format!("p{i}");
                copy.world.insert(path.clone(), el("0"));
                run.world.insert(path.clone(), el("1"));
                log.push(changed(&format!("{}/y", names[usize::from(*w)]), &path));
            }
            let st = SessionState {
                run: run.clone(),
                entry: Some(EntryCopy {
                    run: copy.clone(),
                    log_len: 0,
                }),
                log,
                ..SessionState::default()
            };
            let (next, _) = apply(&ir, st, stop());
            for n in names {
                let r = root(n);
                if n == owner {
                    prop_assert_eq!(next.run.slices.get(&r), copy.slices.get(&r));
                } else {
                    prop_assert_eq!(next.run.slices.get(&r), run.slices.get(&r));
                }
            }
            for (i, w) in writers.iter().enumerate() {
                let path = format!("p{i}");
                let expected = if names[usize::from(*w)] == owner {
                    copy.world.get(&path)
                } else {
                    run.world.get(&path)
                };
                prop_assert_eq!(next.run.world.get(&path), expected);
            }
            prop_assert!(next.run.busy.is_none());
            Ok(())
        },
    );
}
