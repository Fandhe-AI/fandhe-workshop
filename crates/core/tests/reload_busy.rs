//! 処理中の再読み込みで世代を上げて続行する試験（TASK-320 1/2、SAVE-4・性質テスト H-5 の固定版）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1 を受け付けて処理中にし、再読み込みしても処理中のまま続き、
//! 世代が +1（2 回なら +2）になり、ログと `ops` に `Reloaded`・`reload` が残ることを確かめる。
//! 入口の写しは作り直さない。
//!
//! TASK-320 2/2 として、再読み込みの前に予約された宛先の進行が後から届いても状態が変わらず `Ignored(Stale)` が
//! 1 件だけ増えること（再読み込みを 2 回挟んでも同じ）と、再読み込みの後の停止が Step の入口へ戻ることを固定する。
//! 性質テスト H-6 は `history_verify.rs` の `reload_at_any_point` が持つ。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::record_path::RecordPath;
use common::tick::next_tick;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_replay::verify_envelope;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::session::{reload, Busy, Session, SessionState};
use fandhe_workshop_core::{apply, load_ir};

const ST1: &str = "main/st-1";

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn stamp(g: u64) -> Stamp {
    Stamp {
        generation: Generation(g),
        step: StepUid(ST1.to_string()),
    }
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた処理中の状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    let mut state = apply(ir, load().initial, first_note()).0;
    for _ in 0..ticks {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

/// 処理中でなくなるまで、いまの世代の宛先で進行を送る。
fn run_to_end(ir: &Ir, mut state: SessionState) -> SessionState {
    while let Some(addr) = next_tick(&state) {
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    state
}

/// 世代とログを除いた状態（世代以外が変わらないことを比べるため）。
fn without_generation_and_log(state: &SessionState) -> SessionState {
    let mut s = state.clone();
    s.run.generation = Generation(0);
    s.log.clear();
    s
}

#[test]
fn reload_while_busy_keeps_busy_and_bumps_generation() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    assert_eq!(before.run.busy, Some(Busy { done: 2, total: 5 }));
    assert_eq!(before.run.generation, Generation(1));
    let (after, events) = reload(before.clone());
    assert_eq!(
        events,
        [Event::Reloaded {
            at: stamp(1),
            new_generation: Generation(2)
        }]
    );
    assert_eq!(after.log.get(before.log.len()..), Some(events.as_slice()));
    assert_eq!(after.run.generation, Generation(2));
    // 世代以外は、実行の状態も入口の写しも下書きもそのまま。
    assert_eq!(
        without_generation_and_log(&after),
        without_generation_and_log(&before)
    );
    assert_eq!(after.run.busy, before.run.busy);
    assert_eq!(after.entry, before.entry);
}

#[test]
fn processing_continues_with_the_new_generation_to_the_end() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    let plain = run_to_end(&ir, before.clone());
    let (reloaded, _) = reload(before);
    // 再読み込みの後の宛先は新しい世代で作る。
    let addr = next_tick(&reloaded).expect("next tick");
    assert_eq!(addr.generation, Generation(2));
    let done = run_to_end(&ir, reloaded);
    assert_eq!(done.run.busy, None);
    assert_eq!(done.run.generation, Generation(plain.run.generation.0 + 1));
    assert_eq!(done.run.world, plain.run.world);
    assert_eq!(done.run.slices, plain.run.slices);
    assert_eq!(done.run.current, plain.run.current);
    assert_eq!(done.run.completed, plain.run.completed);
    let count = done
        .log
        .iter()
        .filter(|e| matches!(e, Event::Reloaded { .. }))
        .count();
    assert_eq!(count, 1);
}

#[test]
fn two_reloads_add_two_generations() {
    let ir = ir();
    for between in [false, true] {
        let before = busy_state(&ir, 1);
        let plain = run_to_end(&ir, before.clone());
        let (mut state, _) = reload(before);
        if between {
            let addr = next_tick(&state).expect("next tick");
            state = apply(&ir, state, Op::Tick(addr)).0;
        }
        state = reload(state).0;
        let done = run_to_end(&ir, state);
        assert_eq!(done.run.busy, None, "between={between}");
        assert_eq!(done.run.generation, Generation(plain.run.generation.0 + 2));
        assert_eq!(done.run.world, plain.run.world);
        let reloads: Vec<_> = done
            .log
            .iter()
            .filter_map(|e| match e {
                Event::Reloaded { at, new_generation } => Some((at.generation.0, new_generation.0)),
                _ => None,
            })
            .collect();
        assert_eq!(reloads.len(), 2, "between={between}");
        for (from, to) in reloads {
            assert_eq!(to, from + 1, "between={between}");
        }
    }
}

/// 右のログの `Reloaded` を指す行の数（SAVE-6）。
fn reloaded_rows(state: &SessionState) -> usize {
    Projection::from_events(&state.log)
        .log()
        .iter()
        .filter(|row| {
            usize::try_from(row.event)
                .ok()
                .and_then(|i| state.log.get(i))
                .is_some_and(|e| matches!(e, Event::Reloaded { .. }))
        })
        .count()
}

#[test]
fn each_reload_adds_one_log_row() {
    let ir = ir();
    let mut state = busy_state(&ir, 1);
    assert_eq!(reloaded_rows(&state), 0);
    state = reload(state).0;
    assert_eq!(reloaded_rows(&state), 1);
    state = reload(state).0;
    assert_eq!(reloaded_rows(&state), 2);
    // 完走の後も行は増えも減りもしない。
    let done = run_to_end(&ir, state);
    assert_eq!(reloaded_rows(&done), 2);
}

#[test]
fn pure_and_session_entries_agree() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    let (pure_state, pure_events) = reload(before.clone());
    let mut session = Session::from_state(ir, before);
    let events = session.reload().to_vec();
    assert_eq!(events, pure_events);
    assert_eq!(session.state(), &pure_state);
    assert_eq!(session.generation(), Generation(2));
}

#[test]
fn reload_when_not_busy_only_bumps_the_generation() {
    let ir = ir();
    let before = apply(&ir, load().initial, Op::Control(Control::Save)).0;
    assert_eq!(before.run.busy, None);
    let g = before.run.generation.0;
    let (after, events) = reload(before.clone());
    assert!(matches!(events.as_slice(), [Event::Reloaded { .. }]));
    assert_eq!(after.run.generation, Generation(g + 1));
    assert_eq!(
        without_generation_and_log(&after),
        without_generation_and_log(&before)
    );
}

#[test]
fn reload_without_a_current_step_changes_nothing() {
    let mut state = load().initial;
    state.run.current = None;
    let (after, events) = reload(state.clone());
    assert!(events.is_empty());
    assert_eq!(after, state);
}

#[test]
fn generation_overflow_is_a_warning_only() {
    let ir = ir();
    let mut before = busy_state(&ir, 1);
    before.run.generation = Generation(u64::MAX);
    let (after, events) = reload(before.clone());
    let [Event::Warning { at, message }] = events.as_slice() else {
        panic!("expected one warning: {events:?}");
    };
    assert_eq!(at.generation, Generation(u64::MAX));
    assert_eq!(message, "generation overflow; reload ignored");
    assert_eq!(after.run, before.run);
    assert_eq!(after.log.len(), before.log.len() + 1);
}

#[test]
fn recorded_reload_replays_to_the_same_state() {
    let ir = ir();
    let mut path = RecordPath::new(&ir, &load().initial);
    path.op(first_note());
    for _ in 0..2 {
        let addr = next_tick(&path.state).expect("next tick");
        path.op(Op::Tick(addr));
    }
    path.reload();
    let addr = next_tick(&path.state).expect("next tick");
    path.op(Op::Tick(addr));
    let RecordPath { state, rec, .. } = path;
    let reloads = rec
        .ops()
        .iter()
        .filter(|o| matches!(o, HistoryOp::Reload))
        .count();
    assert_eq!(reloads, 1);
    let env = build_envelope(&ir, "s-1", rec.into_ops(), state.clone()).expect("build");
    assert_eq!(verify_envelope(&ir, load().initial, &env), Ok(state));
}

/// 古い宛先を送り、`Ignored(Stale)` がちょうど 1 件で、ログ以外の状態が変わらないことを確かめて新しい状態を返す。
fn assert_stale_ignored(ir: &Ir, state: SessionState, old: TickAddr) -> SessionState {
    let (after, events) = apply(ir, state.clone(), Op::Tick(old));
    // `at` は操作前の状態から作り、外から来た宛先の世代を写さない。
    let step = state.run.current.clone().expect("current");
    assert_eq!(
        events,
        [Event::Ignored {
            at: Stamp {
                generation: state.run.generation,
                step,
            },
            reason: IgnoreReason::Stale
        }]
    );
    assert_eq!(after.log.len(), state.log.len() + 1);
    assert_eq!(after.log.get(state.log.len()..), Some(events.as_slice()));
    let mut expected = state;
    expected.log = after.log.clone();
    assert_eq!(after, expected);
    after
}

#[test]
fn stale_tick_after_reload_is_ignored_once() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    let old = next_tick(&before).expect("next tick");
    assert_eq!(old.generation, Generation(1));
    let (reloaded, _) = reload(before);
    let after = assert_stale_ignored(&ir, reloaded.clone(), old);
    assert_eq!(after.run.generation, Generation(2));
    assert_eq!(after.run.busy, reloaded.run.busy);
    // 古い進行が効いていなければ、新しい宛先で最後まで流した結果は再読み込みなしの経路と同じになる。
    let plain = run_to_end(&ir, busy_state(&ir, 2));
    let done = run_to_end(&ir, after);
    assert_eq!(done.run.busy, None);
    assert_eq!(done.run.world, plain.run.world);
    assert_eq!(done.run.slices, plain.run.slices);
    assert_eq!(done.run.completed, plain.run.completed);
}

#[test]
fn stale_ticks_from_before_two_reloads_are_each_ignored() {
    let ir = ir();
    for between in [false, true] {
        let mut state = busy_state(&ir, 1);
        let old0 = next_tick(&state).expect("next tick");
        state = reload(state).0;
        if between {
            let addr = next_tick(&state).expect("next tick");
            state = apply(&ir, state, Op::Tick(addr)).0;
        }
        let old1 = next_tick(&state).expect("next tick");
        state = reload(state).0;
        assert!(state.run.generation.0 >= 3, "between={between}");
        let start = state.log.len();
        state = assert_stale_ignored(&ir, state, old0);
        state = assert_stale_ignored(&ir, state, old1);
        assert_eq!(state.log.len(), start + 2, "between={between}");
    }
}

#[test]
fn stale_tick_after_reload_when_not_busy_is_stale() {
    let ir = ir();
    let busy = busy_state(&ir, 1);
    let old = next_tick(&busy).expect("next tick");
    let done = run_to_end(&ir, busy);
    assert_eq!(done.run.busy, None);
    let (reloaded, _) = reload(done);
    assert_stale_ignored(&ir, reloaded, old);
}

#[test]
fn stop_after_reload_returns_to_step_entry() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    let entry = before.entry.clone().expect("entry");
    let old0 = next_tick(&before).expect("next tick");
    let (reloaded, _) = reload(before);
    let old1 = next_tick(&reloaded).expect("next tick");
    let (stopped, events) = apply(&ir, reloaded, Op::Control(Control::Stop));
    assert!(matches!(
        events.first(),
        Some(Event::Stopped { at, new_generation })
            if *at == stamp(2) && *new_generation == Generation(3)
    ));
    // 入口の写しは再読み込みで作り直されないので、停止は再読み込みの前の入口へ戻る。
    assert_eq!(stopped.entry, Some(entry.clone()));
    let mut expected = entry.run;
    expected.generation = Generation(3);
    expected.busy = None;
    assert_eq!(stopped.run, expected);
    // 停止の前の宛先は、再読み込みの前のものも後のものも無視される。
    let s = assert_stale_ignored(&ir, stopped, old0);
    assert_stale_ignored(&ir, s, old1);
}

#[test]
fn session_reload_then_apply_ignores_stale_tick() {
    let ir = ir();
    let before = busy_state(&ir, 2);
    let old = next_tick(&before).expect("next tick");
    let (pure_state, _) = reload(before.clone());
    let expected = assert_stale_ignored(&ir, pure_state, old.clone());
    let mut session = Session::from_state(ir, before);
    session.reload();
    let events = session.apply(Op::Tick(old)).to_vec();
    assert!(matches!(
        events.as_slice(),
        [Event::Ignored {
            reason: IgnoreReason::Stale,
            ..
        }]
    ));
    assert_eq!(session.state(), &expected);
}
