//! 復帰前の進行の無視と処理中の再開の試験（TASK-214 3/3、仕様 EVT-12・性質テスト P-2・P-4・B-6r の native 版）。
//!
//! 3 Step の教材（`min-3step.json`、st-1 の処理は 5 段）で、`Control::Restore` の前の宛先（世代・Step・番号）を
//! 持つ進行 `Op::Tick` が復帰の後に届いても、`run` は変わらず `Ignored(Stale)` が 1 件だけ増えることを確かめる。
//! 復帰の後は現在の Step と処理中の番号が保存時点に戻るため、復帰前の進行は Step と番号が一致したまま世代だけが
//! 食い違う。世代の照合が外れるとこの進行が二重に効くので、試験はその場面を中心に置く。
//! 処理中の写しへの復帰は、新しい世代の宛先で保存時点の番号から最後まで進み、`(世代, 番号)` ごとに `Progress` が
//! 1 回ずつ出る。写しの控えと戻し・`Restored` の記録・復習の作り直しの試験は `checkpoint_restore.rs`・
//! `restore_review.rs`（TASK-214 1/3・2/3）。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::property::{assert_replay_eq, check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{ControlKind, Event, IgnoreReason, RefuseReason};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::session::{Busy, SessionState};
use fandhe_workshop_core::{apply, load_ir};
use proptest::prelude::*;
use std::collections::BTreeSet;

const ST1: &str = "main/st-1";
const ST2: &str = "main/st-2";

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn restore(n: u32) -> Op {
    Op::Control(Control::Restore(CheckpointId(n)))
}

fn stop() -> Op {
    Op::Control(Control::Stop)
}

fn go_to(s: &str) -> Op {
    Op::Control(Control::GoTo(uid(s)))
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    let state = apply(ir, load().initial, first_note()).0;
    tick_n(ir, state, ticks)
}

fn tick_n(ir: &Ir, mut state: SessionState, n: u32) -> SessionState {
    for _ in 0..n {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

fn tick_at(generation: u64, step: &str, index: u32) -> Op {
    Op::Tick(TickAddr {
        generation: Generation(generation),
        step: uid(step),
        index,
    })
}

/// 処理中の状態に残っている予約（`done..total` の番号、現在の世代・現在の Step）。
fn reserved_ticks(state: &SessionState) -> Vec<TickAddr> {
    let (Some(step), Some(busy)) = (state.run.current.clone(), state.run.busy.as_ref()) else {
        return Vec::new();
    };
    (busy.done..busy.total)
        .map(|index| TickAddr {
            generation: state.run.generation,
            step: step.clone(),
            index,
        })
        .collect()
}

fn progress_indices(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Progress { index, .. } => Some(*index),
            _ => None,
        })
        .collect()
}

/// 無視の 1 件だけが増え、`run` が変わらないことを確かめる（`at` は操作前の状態の世代・Step）。
fn assert_ignored(ir: &Ir, state: &SessionState, op: Op) -> SessionState {
    let current = state.run.current.clone().expect("current");
    let (next, events) = apply(ir, state.clone(), op);
    assert_eq!(next.run, state.run);
    assert_eq!(
        events,
        vec![Event::Ignored {
            at: Stamp {
                generation: state.run.generation,
                step: current
            },
            reason: IgnoreReason::Stale
        }]
    );
    assert_eq!(next.log.len(), state.log.len() + 1);
    next
}

#[test]
fn pre_restore_tick_with_saved_index_is_ignored() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 3), save()).0;
    // 4 段目まで進める（世代 1 の宛先 (1, st-1, 3) を届ける）。
    let old = next_tick(&saved).expect("next tick");
    assert_eq!(old.index, 3);
    let advanced = apply(&ir, saved, Op::Tick(old.clone())).0;
    assert_eq!(advanced.run.busy, Some(Busy { done: 4, total: 5 }));
    let restored = apply(&ir, advanced, restore(1)).0;
    assert_eq!(restored.run.generation, Generation(2));
    assert_eq!(restored.run.busy, Some(Busy { done: 3, total: 5 }));
    // 届け済みの宛先がもう一度届く（二重の届き）。Step と番号は一致し、世代だけが古い。
    let ignored = assert_ignored(&ir, &restored, Op::Tick(old));
    assert_eq!(ignored.run.busy, Some(Busy { done: 3, total: 5 }));
}

#[test]
fn reserved_ticks_before_restore_are_ignored() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 3), save()).0;
    let advanced = tick_n(&ir, saved, 1);
    // 復帰前に残っていた予約（(1, st-1, 4) だけ）。
    let reserved = reserved_ticks(&advanced);
    assert_eq!(reserved.len(), 1);
    let mut state = apply(&ir, advanced, restore(1)).0;
    for addr in reserved {
        state = assert_ignored(&ir, &state, Op::Tick(addr));
    }
    assert_eq!(state.run.generation, Generation(2));
}

#[test]
fn restore_resumes_progress_in_new_generation() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 3), save()).0;
    let advanced = tick_n(&ir, saved, 1);
    let mut state = apply(&ir, advanced, restore(1)).0;
    assert_eq!(
        next_tick(&state),
        Some(TickAddr {
            generation: Generation(2),
            step: uid(ST1),
            index: 3
        })
    );
    // 古い世代の宛先を挟んでも、再開した進行は止まらず、二重にも効かない。
    let mut seen = Vec::new();
    for _ in 3..5 {
        state = assert_ignored(&ir, &state, tick_at(1, ST1, 4));
        let addr = next_tick(&state).expect("next tick");
        let (next, events) = apply(&ir, state, Op::Tick(addr));
        for e in &events {
            if let Event::Progress { at, index, .. } = e {
                assert_eq!(at.generation, Generation(2));
                seen.push(*index);
            }
        }
        state = next;
    }
    assert_eq!(seen, vec![3, 4]);
    assert_eq!(state.run.busy, None);
    assert_eq!(state.run.generation, Generation(2));
}

#[test]
fn pre_restore_tick_after_go_to_and_restore_is_ignored() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 2), save()).0;
    let gen1 = reserved_ticks(&saved);
    let moved = apply(&ir, saved, go_to(ST2)).0;
    assert_eq!(moved.run.generation, Generation(2));
    let restored = apply(&ir, moved, restore(1)).0;
    assert_eq!(restored.run.generation, Generation(3));
    assert_eq!(restored.run.current, Some(uid(ST1)));
    assert_eq!(restored.run.busy, Some(Busy { done: 2, total: 5 }));
    let mut state = restored;
    // 世代 1 の予約と、世代 2 の宛先（st-1・st-2 とも）はすべて無視される。
    for addr in gen1 {
        state = assert_ignored(&ir, &state, Op::Tick(addr));
    }
    for step in [ST1, ST2] {
        for index in 0..5 {
            state = assert_ignored(&ir, &state, tick_at(2, step, index));
        }
    }
    assert_eq!(state.run.busy, Some(Busy { done: 2, total: 5 }));
}

#[test]
fn pre_restore_tick_after_stop_reinput_and_restore_is_ignored() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 2), save()).0;
    let stopped = apply(&ir, saved, stop()).0;
    let reinput = apply(&ir, stopped, first_note()).0;
    let advanced = tick_n(&ir, reinput, 2);
    let gen_before = advanced.run.generation;
    // 保存時点の done（2）と番号が一致する、復帰前の予約を含む。
    let reserved = reserved_ticks(&advanced);
    assert_eq!(reserved.first().map(|a| a.index), Some(2));
    let restored = apply(&ir, advanced, restore(1)).0;
    assert_eq!(restored.run.generation.0, gen_before.0 + 1);
    assert_eq!(restored.run.busy, Some(Busy { done: 2, total: 5 }));
    let mut state = restored;
    for addr in reserved {
        state = assert_ignored(&ir, &state, Op::Tick(addr));
    }
}

#[test]
fn restore_to_idle_checkpoint_ignores_pre_restore_ticks() {
    let ir = ir();
    // 受付の前（処理中でない）で保存し、受け付けて処理中にした後に復帰する。
    let saved = apply(&ir, load().initial, save()).0;
    assert_eq!(saved.run.busy, None);
    let accepted = apply(&ir, saved, first_note()).0;
    let advanced = tick_n(&ir, accepted, 1);
    let reserved = reserved_ticks(&advanced);
    assert!(!reserved.is_empty());
    let restored = apply(&ir, advanced, restore(1)).0;
    assert_eq!(restored.run.busy, None);
    let mut state = restored;
    // 処理中でないが、世代の照合が先なので理由は NoProcessing ではなく Stale になる。
    for addr in reserved {
        state = assert_ignored(&ir, &state, Op::Tick(addr));
    }
}

#[test]
fn failed_restore_keeps_reservation() {
    let ir = ir();
    let busy = busy_state(&ir, 2);
    let addr = next_tick(&busy).expect("next tick");
    let (refused, events) = apply(&ir, busy, restore(9));
    assert!(matches!(
        events.as_slice(),
        [Event::Refused {
            op: ControlKind::Restore,
            reason: RefuseReason::UnknownCheckpoint,
            ..
        }]
    ));
    assert_eq!(refused.run.generation, Generation(1));
    let (done, events) = apply(&ir, refused, Op::Tick(addr));
    assert_eq!(progress_indices(&events), vec![2]);
    assert_eq!(done.run.busy, Some(Busy { done: 3, total: 5 }));
}

#[test]
fn each_generation_index_progresses_once() {
    let ir = ir();
    let mut state = apply(&ir, busy_state(&ir, 2), save()).0;
    let old = next_tick(&state).expect("next tick");
    let mut pairs: Vec<(u64, u32)> = Vec::new();
    let mut run = |state: SessionState, op: Op| {
        let (next, events) = apply(&ir, state, op);
        for e in &events {
            if let Event::Progress { at, index, .. } = e {
                pairs.push((at.generation.0, *index));
            }
        }
        next
    };
    // 保存・古い進行の届き・復帰・再開した進行を繰り返す。同じ宛先の二重の届きも混ぜる。
    for _ in 0..3 {
        state = run(state, Op::Tick(old.clone()));
        state = run(state, restore(1));
        state = run(state, Op::Tick(old.clone()));
        let addr = next_tick(&state).expect("next tick");
        state = run(state, Op::Tick(addr.clone()));
        state = run(state, Op::Tick(addr));
    }
    let unique: BTreeSet<_> = pairs.iter().copied().collect();
    assert_eq!(unique.len(), pairs.len(), "duplicate progress: {pairs:?}");
    // 世代 1 で 1 回、復帰ごとの新しい世代で 1 回ずつ。
    assert_eq!(pairs, vec![(1, 2), (2, 2), (3, 2), (4, 2)]);
}

#[derive(Debug, Clone)]
enum Before {
    Next,
    Resend,
    Stop,
    GoTo,
}

fn before_strategy() -> impl Strategy<Value = Before> {
    prop_oneof![
        Just(Before::Next),
        Just(Before::Resend),
        Just(Before::Stop),
        Just(Before::GoTo),
    ]
}

#[derive(Debug, Clone)]
enum Post {
    Old(usize),
    Next,
    Resend,
    Restore,
}

fn post_strategy() -> impl Strategy<Value = Post> {
    prop_oneof![
        (0usize..16).prop_map(Post::Old),
        Just(Post::Next),
        Just(Post::Resend),
        Just(Post::Restore),
    ]
}

type Scenario = (u32, Vec<Before>, Vec<Post>);

fn scenario() -> impl Strategy<Value = Scenario> {
    (
        0u32..5,
        op_sequence(before_strategy()),
        op_sequence(post_strategy()),
    )
}

/// 進行を進めて保存し、保存の後に操作列を流して復帰する。(保存した状態, 復帰した状態, 復帰前に見た宛先)。
fn run_until_restore(ticks: u32, before: &[Before]) -> (SessionState, SessionState, Vec<TickAddr>) {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, ticks), save()).0;
    let mut state = saved.clone();
    let mut seen = reserved_ticks(&state);
    for op in before {
        state = match op {
            Before::Next => match next_tick(&state) {
                Some(addr) => apply(&ir, state, Op::Tick(addr)).0,
                None => state,
            },
            Before::Resend => apply(&ir, state, first_note()).0,
            Before::Stop => apply(&ir, state, stop()).0,
            Before::GoTo => apply(&ir, state, go_to(ST2)).0,
        };
        seen.extend(reserved_ticks(&state));
    }
    let restored = apply(&ir, state, restore(1)).0;
    (saved, restored, seen)
}

/// 復帰の後に操作列を流す。復帰前の宛先の進行ごとに (操作前, 操作後, イベント列) を返す。
fn run_post_restore(
    state: SessionState,
    seen: &[TickAddr],
    post: &[Post],
) -> (SessionState, Vec<(SessionState, SessionState, Vec<Event>)>) {
    let ir = ir();
    let mut state = state;
    let mut stale = Vec::new();
    for op in post {
        match op {
            Post::Old(k) => {
                if let Some(addr) = seen.get(k % seen.len().max(1)) {
                    let (next, events) = apply(&ir, state.clone(), Op::Tick(addr.clone()));
                    stale.push((state.clone(), next.clone(), events));
                    state = next;
                }
            }
            Post::Next => {
                if let Some(addr) = next_tick(&state) {
                    state = apply(&ir, state, Op::Tick(addr)).0;
                }
            }
            Post::Resend => state = apply(&ir, state, first_note()).0,
            Post::Restore => state = apply(&ir, state, restore(1)).0,
        }
    }
    (state, stale)
}

#[test]
fn p2_pre_restore_ticks_never_change_run() {
    check(
        "p2-pre-restore-ticks-never-change-run",
        DEFAULT_CASES,
        scenario(),
        |(ticks, before, post)| {
            let (_, restored, seen) = run_until_restore(ticks, &before);
            let (_, stale) = run_post_restore(restored, &seen, &post);
            // 正解は judge を使わず、宛先の世代が操作前の世代より小さいことだけで決める。
            for (state_before, state_after, events) in &stale {
                prop_assert_eq!(&state_after.run, &state_before.run);
                let Some(current) = state_before.run.current.clone() else {
                    return Err(TestCaseError::fail("current is None after restore"));
                };
                prop_assert_eq!(
                    events,
                    &vec![Event::Ignored {
                        at: Stamp {
                            generation: state_before.run.generation,
                            step: current
                        },
                        reason: IgnoreReason::Stale
                    }]
                );
                prop_assert_eq!(state_after.log.len(), state_before.log.len() + 1);
            }
            Ok(())
        },
    );
}

#[test]
fn p4_restore_resumes_from_saved_done() {
    check(
        "p4-restore-resumes-from-saved-done",
        DEFAULT_CASES,
        (0u32..5, op_sequence(before_strategy())),
        |(ticks, before)| {
            let ir = ir();
            let (saved, restored, _) = run_until_restore(ticks, &before);
            let Some(cp_busy) = saved.run.busy.clone() else {
                prop_assert_eq!(restored.run.busy, None);
                return Ok(());
            };
            let Some(current) = saved.run.current.clone() else {
                return Err(TestCaseError::fail("current is None in checkpoint"));
            };
            prop_assert_eq!(
                next_tick(&restored),
                Some(TickAddr {
                    generation: restored.run.generation,
                    step: current,
                    index: cp_busy.done
                })
            );
            let mut state = restored;
            let mut indices = Vec::new();
            for _ in cp_busy.done..cp_busy.total {
                let Some(addr) = next_tick(&state) else {
                    return Err(TestCaseError::fail("no next tick while busy"));
                };
                let (next, events) = apply(&ir, state, Op::Tick(addr));
                indices.extend(progress_indices(&events));
                state = next;
            }
            prop_assert_eq!(indices, (cp_busy.done..cp_busy.total).collect::<Vec<_>>());
            prop_assert_eq!(state.run.busy, None);
            Ok(())
        },
    );
}

#[test]
fn post_restore_sequence_is_deterministic() {
    check(
        "post-restore-sequence-is-deterministic",
        DEFAULT_CASES,
        scenario(),
        |(ticks, before, post)| {
            assert_replay_eq(
                |p: &[Scenario]| {
                    let (t, b, o) = &p[0];
                    let (_, restored, seen) = run_until_restore(*t, b);
                    let (last, stale) = run_post_restore(restored, &seen, o);
                    let events: Vec<Vec<Event>> = stale.into_iter().map(|x| x.2).collect();
                    (last, events)
                },
                std::slice::from_ref(&(ticks, before, post)),
            )
        },
    );
}
