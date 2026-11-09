//! 進行の連続適用 `fast_forward` の試験（TASK-227 1/2、EVT-17）。
//!
//! 3 Step の教材（`min-3step.json`）の st-1（進行 5 つ）で、早送りの入（連続適用）と切（タイマー相当の 1 件ずつの適用）が、
//! 最終の状態・イベント列・履歴の `ops`・`Progress` の数で一致することを固定する。性質テストは、任意の操作列の
//! 各操作の後に両側へ同じ進行の扱いをして一致を確かめる。TASK-227 2/2 で、早送りを `limit` で区切った間の停止（EVT-9）と、確認・選択・承認・復帰の場面を早送りが削らないことの試験を足した。
//! 呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{guided_op_specs, op_specs, Harness, OpSpec, Which};
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use common::tick::next_tick;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::fast_forward::{fast_forward, FastForwardEnd};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;
use serde_json::json;

const ST1: &str = "main/st-1";

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// st-1 が `Entered`・処理中でない・世代 1 の状態。
fn entered() -> SessionState {
    let u = uid(ST1);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "created".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn busy_session(ir: Ir) -> Session {
    let mut s = Session::from_state(ir, entered());
    s.apply(first_note());
    assert_eq!(s.state().run.busy, Some(Busy { done: 0, total: 5 }));
    s
}

fn busy_recorded(ir: Ir) -> RecordedSession {
    let mut r = RecordedSession::new(Session::from_state(ir, entered()), "s-1");
    r.apply(first_note());
    assert_eq!(
        r.session().state().run.busy,
        Some(Busy { done: 0, total: 5 })
    );
    r
}

/// 宛先が進まない（世代・Step が変わった、または番号が増えない）か。
fn is_stuck(state: &SessionState, prev: &TickAddr) -> bool {
    next_tick(state).is_some_and(|n| {
        n.generation != prev.generation || n.step != prev.step || n.index <= prev.index
    })
}

/// 切の基準（タイマー相当）。処理中が解けるか番号が進まなくなるまで 1 件ずつ適用する。
fn tick_by_tick(s: &mut Session) -> (Vec<TickAddr>, Vec<Event>) {
    let (mut ticks, mut events) = (Vec::new(), Vec::new());
    while let Some(addr) = next_tick(s.state()) {
        events.extend(s.apply(Op::Tick(addr.clone())).to_vec());
        let stuck = is_stuck(s.state(), &addr);
        ticks.push(addr);
        if stuck {
            break;
        }
    }
    (ticks, events)
}

fn tick_by_tick_recorded(r: &mut RecordedSession) {
    while let Some(addr) = next_tick(r.session().state()) {
        r.apply(Op::Tick(addr.clone()));
        if is_stuck(r.session().state(), &addr) {
            break;
        }
    }
}

fn progress_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

#[test]
fn fast_forward_matches_tick_by_tick() {
    let mut off = busy_session(base_ir());
    let mut on = off.clone();
    let (off_ticks, off_events) = tick_by_tick(&mut off);
    let ff = fast_forward(&mut on, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.ticks.len(), 5);
    assert_eq!(ff.ticks, off_ticks);
    assert_eq!(ff.events, off_events);
    assert_eq!(progress_count(&ff.events), 5);
    assert_eq!(on.state(), off.state());
    assert!(on.state().run.busy.is_none());
}

#[test]
fn fast_forward_records_same_ops() {
    let mut off = busy_recorded(base_ir());
    let mut on = off.clone();
    tick_by_tick_recorded(&mut off);
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(on.ops(), off.ops());
    assert_eq!(on.ops().len(), 6, "受付 1 + 進行 5");
    assert_eq!(on.envelope_for_save(), off.envelope_for_save());
    assert_eq!(on.session().state(), off.session().state());
    let replayed = replay(on.session().ir(), entered(), on.ops()).expect("replay");
    assert_eq!(&replayed, on.session().state());
}

#[test]
fn fast_forward_not_busy_is_noop() {
    let mut s = Session::from_state(base_ir(), entered());
    let before = s.state().clone();
    let ff = fast_forward(&mut s, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::NotBusy);
    assert!(ff.ticks.is_empty() && ff.events.is_empty());
    assert_eq!(s.state(), &before);

    let mut r = RecordedSession::new(Session::from_state(base_ir(), entered()), "s-1");
    let ff = r.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::NotBusy);
    assert!(r.ops().is_empty());
    assert_eq!(r.session().state(), &before);
}

#[test]
fn fast_forward_limit_splits_without_difference() {
    let mut whole = busy_recorded(base_ir());
    let mut split = whole.clone();
    let mut off = whole.clone();
    whole.fast_forward(u32::MAX);
    tick_by_tick_recorded(&mut off);

    let zero = split.fast_forward(0);
    assert_eq!(zero.end, FastForwardEnd::LimitReached);
    assert!(zero.ticks.is_empty());
    assert_eq!(split.ops().len(), 1);

    let part = split.fast_forward(2);
    assert_eq!(part.end, FastForwardEnd::LimitReached);
    assert_eq!(part.ticks.len(), 2);
    assert_eq!(
        split.session().state().run.busy,
        Some(Busy { done: 2, total: 5 })
    );
    let rest = split.fast_forward(u32::MAX);
    assert_eq!(rest.end, FastForwardEnd::Finished);
    assert_eq!(rest.ticks.len(), 3);

    assert_eq!(split.session().state(), whole.session().state());
    assert_eq!(split.session().state(), off.session().state());
    assert_eq!(split.ops(), whole.ops());
    assert_eq!(split.ops(), off.ops());
}

#[test]
fn fast_forward_confirm_none_achieves_same_as_ticks() {
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == ST1) {
        n.step.confirm = Some(json!({ "kind": "none", "reason": "memo only" }));
    }
    let mut off = busy_session(ir);
    let mut on = off.clone();
    let (_, off_events) = tick_by_tick(&mut off);
    let ff = fast_forward(&mut on, u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.events, off_events);
    assert_eq!(on.state(), off.state());
    assert!(ff
        .events
        .iter()
        .any(|e| matches!(e, Event::Achieved { .. })));
    assert!(ff
        .events
        .iter()
        .any(|e| matches!(e, Event::Transitioned { .. })));
    assert!(ff.events.iter().any(|e| matches!(e, Event::Entered { .. })));
    assert!(on.state().run.busy.is_none());
    assert_ne!(on.state().run.current, Some(uid(ST1)));
}

#[test]
fn fast_forward_stalls_on_unreadable_processing() {
    let mut state = busy_session(base_ir()).into_state();
    state.run.busy = Some(Busy { done: 0, total: 4 });
    let mut off = Session::from_state(base_ir(), state);
    let mut on = off.clone();
    let (off_ticks, off_events) = tick_by_tick(&mut off);
    assert_eq!(off_ticks.len(), 1);
    let ff = fast_forward(&mut on, 100);
    assert_eq!(ff.end, FastForwardEnd::Stalled);
    assert_eq!(ff.ticks.len(), 1);
    assert!(matches!(ff.events.as_slice(), [Event::Warning { .. }]));
    assert_eq!(ff.events, off_events);
    assert_eq!(on.state(), off.state());
}

/// 任意の操作列の各操作の後に、早送りの入と切で最終の状態・ログ・`ops`・`Progress` の数が一致する。
///
/// 操作は切の側の `Harness` が解決し、同じ `Op` を両側へ渡す。切の側は各操作の後に進行を 1 件ずつ、入の側は連続適用で進める。
fn on_off_property(specs: &[OpSpec]) -> Result<(), TestCaseError> {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    let ir = load_ir(&bytes).expect("load ir");
    let initial = sample.initial;
    let mut harness = Harness::via_session(&ir, initial.clone());
    let mut off = RecordedSession::new(Session::from_state(ir.clone(), initial), "s-1");
    let mut on = off.clone();
    let (mut off_progress, mut on_progress) = (0usize, 0usize);
    for spec in specs {
        let applied = harness.step(spec);
        match &applied.op {
            Some(op) => {
                off_progress += progress_count(off.apply(op.clone()));
                on_progress += progress_count(on.apply(op.clone()));
            }
            None => {
                off_progress += progress_count(off.reload());
                on_progress += progress_count(on.reload());
            }
        }
        // 切: 処理中が解けるか番号が進まなくなるまで、タイマー相当で 1 件ずつ。
        let mut guard = 0u32;
        while let Some(addr) = next_tick(harness.state()) {
            let a = harness.step(&OpSpec::Tick(Which::Current));
            if let Some(op) = a.op {
                off_progress += progress_count(off.apply(op));
            }
            guard += 1;
            if is_stuck(harness.state(), &addr) || guard > 10_000 {
                break;
            }
        }
        // 入: 連続適用。
        on_progress += progress_count(&on.fast_forward(u32::MAX).events);
        prop_assert_eq!(on.session().state(), off.session().state());
        prop_assert_eq!(on.ops(), off.ops());
        prop_assert_eq!(on_progress, off_progress);
    }
    prop_assert_eq!(on.session().state(), harness.state());
    Ok(())
}

#[test]
fn fast_forward_on_off_same_final_state() {
    check("fast-forward-on-off", DEFAULT_CASES, op_specs(), |specs| {
        on_off_property(&specs)
    });
}

#[test]
fn fast_forward_on_off_same_final_state_guided() {
    check(
        "fast-forward-on-off-guided",
        DEFAULT_CASES,
        guided_op_specs(),
        |specs| on_off_property(&specs),
    );
}

fn stop() -> Op {
    Op::Control(Control::Stop)
}

fn stamp(g: u64, step: &str) -> Stamp {
    Stamp {
        generation: Generation(g),
        step: uid(step),
    }
}

/// 早送りの入（`fast_forward(k)` の後に停止）と切（1 件ずつ `k` 件の後に停止）の組を作る。
fn stopped_pair(k: u32) -> (RecordedSession, RecordedSession, Vec<Event>) {
    let mut off = busy_recorded(base_ir());
    let mut on = off.clone();
    for _ in 0..k {
        let addr = next_tick(off.session().state()).expect("next tick");
        off.apply(Op::Tick(addr));
    }
    on.fast_forward(k);
    let events = on.apply(stop()).to_vec();
    let off_events = off.apply(stop()).to_vec();
    assert_eq!(events, off_events);
    (on, off, events)
}

/// 早送りを `limit` で区切った途中の停止は、切の側の停止と同じ結果になり、入口の写しへ戻る（EVT-9）。
#[test]
fn fast_forward_stop_midway_restores_entry() {
    for k in 0..=4u32 {
        let (on, off, events) = stopped_pair(k);
        assert!(
            matches!(events.first(), Some(Event::Stopped { .. })),
            "k={k}: {events:?}"
        );
        let run = &on.session().state().run;
        assert_eq!(run.generation, Generation(2), "k={k}");
        assert_eq!(run.busy, None, "k={k}");
        assert_eq!(on.session().step_status(&uid(ST1)), StepStatus::Entered);
        let draft = run.world.get("notes/draft").expect("draft");
        assert_eq!(
            draft.attrs.get("state"),
            Some(&"created".to_string()),
            "k={k}"
        );
        assert_eq!(on.session().state(), off.session().state(), "k={k}");
        assert_eq!(on.ops(), off.ops(), "k={k}");
    }
}

/// 停止の後の早送りは何も適用せず、`ops` も増えない。
#[test]
fn fast_forward_after_stop_is_not_busy() {
    let (mut on, _, _) = stopped_pair(2);
    let (ops, before) = (on.ops().len(), on.session().state().clone());
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::NotBusy);
    assert!(ff.ticks.is_empty() && ff.events.is_empty());
    assert_eq!(on.ops().len(), ops);
    assert_eq!(on.session().state(), &before);
}

/// 停止の前の世代の宛先は、停止の直後も送り直した受付の後も、状態を変えず `Ignored(Stale)` が 1 件増えるだけである。
#[test]
fn fast_forward_stale_ticks_after_stop_ignored() {
    let mut r = busy_recorded(base_ir());
    let ff = r.fast_forward(2);
    let applied = ff.ticks.last().cloned().expect("applied tick");
    let pending = next_tick(r.session().state()).expect("pending tick");
    assert_eq!(pending.generation, Generation(1));
    r.apply(stop());
    for stale in [&applied, &pending] {
        let run = r.session().state().run.clone();
        let log = r.session().state().log.len();
        let events = r.apply(Op::Tick(stale.clone())).to_vec();
        assert_eq!(
            events,
            vec![Event::Ignored {
                at: stamp(2, ST1),
                reason: IgnoreReason::Stale
            }]
        );
        assert_eq!(r.session().state().run, run);
        assert_eq!(r.session().state().log.len(), log + 1);
    }
    r.apply(first_note());
    assert_eq!(
        r.session().state().run.busy,
        Some(Busy { done: 0, total: 5 })
    );
    for stale in [&applied, &pending] {
        let run = r.session().state().run.clone();
        let log = r.session().state().log.len();
        let events = r.apply(Op::Tick(stale.clone())).to_vec();
        assert!(matches!(
            events.as_slice(),
            [Event::Ignored {
                reason: IgnoreReason::Stale,
                ..
            }]
        ));
        assert_eq!(r.session().state().run, run);
        assert_eq!(r.session().state().log.len(), log + 1);
    }
}

/// 停止の後に同じ入力を送り直すと再び受け付け、早送りで最後まで進み、切の側と一致する。
#[test]
fn fast_forward_resume_after_stop_matches() {
    let (mut on, mut off, _) = stopped_pair(2);
    on.apply(first_note());
    off.apply(first_note());
    assert_eq!(
        on.session().state().run.busy,
        Some(Busy { done: 0, total: 5 })
    );
    tick_by_tick_recorded(&mut off);
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.ticks.len(), 5);
    assert_eq!(on.session().state(), off.session().state());
    assert_eq!(on.ops(), off.ops());
    assert_eq!(on.envelope_for_save(), off.envelope_for_save());
}

fn at_step(step: &str, status: StepStatus) -> SessionState {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), status);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn approve_ir() -> Ir {
    let mut ir = base_ir();
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == ST1) {
        n.step.confirm = Some(json!({ "kind": "approve" }));
    }
    ir
}

fn has_achievement(events: &[Event]) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Achieved { .. } | Event::Transitioned { .. }))
}

fn point(path: &str) -> Op {
    Op::Input(Input::Ui {
        view: "v-files".into(),
        path: path.into(),
    })
}

fn approval(decision: Decision) -> Op {
    Op::Input(Input::Approval {
        decision,
        target: None,
    })
}

/// 確認（左の変化を指す）の Step は、早送りの後も受付済みのままで、外れた指し先では達成せず、正しい指し先で初めて達成する。
///
/// `confirm: none` の自動の達成は教材の宣言による省略であり、場面を削ることとは別である。
#[test]
fn fast_forward_keeps_point_at_scene() {
    let mut r = busy_recorded(base_ir());
    let ff = r.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert!(!has_achievement(&ff.events));
    assert_eq!(r.session().step_status(&uid(ST1)), StepStatus::Accepted);
    assert_eq!(r.session().state().run.current, Some(uid(ST1)));
    assert_eq!(r.fast_forward(u32::MAX).end, FastForwardEnd::NotBusy);

    let before = r.session().state().run.clone();
    let events = r.apply(point("notes/other")).to_vec();
    assert!(!has_achievement(&events));
    assert_eq!(r.session().state().run, before);

    let events = r.apply(point("notes/draft")).to_vec();
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    assert_eq!(r.session().state().run.current, Some(uid("main/st-2")));
}

/// 選択（st-2）・承認（st-3）の Step は処理中でないので早送りは何も適用せず、処理を持つ承認の Step は承認待ちで止まる。
#[test]
fn fast_forward_keeps_choice_and_approval_scenes() {
    for step in ["main/st-2", "main/st-3"] {
        let mut r = RecordedSession::new(
            Session::from_state(base_ir(), at_step(step, StepStatus::Entered)),
            "s-1",
        );
        let ff = r.fast_forward(u32::MAX);
        assert_eq!(ff.end, FastForwardEnd::NotBusy, "{step}");
        assert!(r.ops().is_empty(), "{step}");
    }

    let mut on = busy_recorded(approve_ir());
    let mut off = on.clone();
    tick_by_tick_recorded(&mut off);
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert!(!has_achievement(&ff.events));
    assert_eq!(on.session().state(), off.session().state());
    assert_eq!(on.session().step_status(&uid(ST1)), StepStatus::Accepted);
    for d in [Decision::Reject, Decision::Revise] {
        let before = on.session().state().run.clone();
        let events = on.apply(approval(d)).to_vec();
        assert!(!has_achievement(&events));
        assert_eq!(on.session().state().run, before);
        assert_eq!(on.session().step_status(&uid(ST1)), StepStatus::Accepted);
    }
    let events = on.apply(approval(Decision::Approve)).to_vec();
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    assert_eq!(on.session().step_status(&uid(ST1)), StepStatus::Achieved);
}

/// 復帰の場面。処理中より前の保存への復帰は処理中を解き、処理中の保存への復帰は処理中を再開して早送りで最後まで進む。
#[test]
fn fast_forward_keeps_restore_scene() {
    let save = || Op::Control(Control::Save);
    let restore = |n| Op::Control(Control::Restore(CheckpointId(n)));

    // (a) 処理中より前の保存へ戻る。
    let mut on = RecordedSession::new(Session::from_state(base_ir(), entered()), "s-1");
    let mut off = on.clone();
    on.apply(save());
    off.apply(save());
    on.apply(first_note());
    off.apply(first_note());
    on.fast_forward(2);
    for _ in 0..2 {
        let addr = next_tick(off.session().state()).expect("tick");
        off.apply(Op::Tick(addr));
    }
    on.apply(restore(1));
    off.apply(restore(1));
    let run = &on.session().state().run;
    assert_eq!(run.generation, Generation(2));
    assert_eq!(run.busy, None);
    assert_eq!(on.session().step_status(&uid(ST1)), StepStatus::Entered);
    assert_eq!(
        run.world
            .get("notes/draft")
            .and_then(|e| e.attrs.get("state")),
        Some(&"created".to_string())
    );
    assert_eq!(on.fast_forward(u32::MAX).end, FastForwardEnd::NotBusy);
    assert_eq!(on.session().state(), off.session().state());
    assert_eq!(on.ops(), off.ops());

    // (b) 処理中の保存へ戻る。
    let mut on = busy_recorded(base_ir());
    let mut off = on.clone();
    on.fast_forward(2);
    for _ in 0..2 {
        let addr = next_tick(off.session().state()).expect("tick");
        off.apply(Op::Tick(addr));
    }
    on.apply(save());
    off.apply(save());
    on.fast_forward(u32::MAX);
    tick_by_tick_recorded(&mut off);
    on.apply(restore(1));
    off.apply(restore(1));
    let run = &on.session().state().run;
    assert_eq!(run.generation, Generation(2));
    assert_eq!(run.busy, Some(Busy { done: 2, total: 5 }));
    let ff = on.fast_forward(u32::MAX);
    assert_eq!(ff.end, FastForwardEnd::Finished);
    assert_eq!(ff.ticks.len(), 3);
    assert!(ff.ticks.iter().all(|t| t.generation == Generation(2)));
    tick_by_tick_recorded(&mut off);
    assert_eq!(on.session().state(), off.session().state());
    assert_eq!(on.ops(), off.ops());
}

/// 任意の操作列の各操作の後に、入（`fast_forward(k)` → 停止）と切（1 件ずつ `k` 件 → 停止）が一致する。
/// 停止の後に直前の宛先を送っても一致し、早送りは `Op::Tick` 以外を `ops` に足さず、受講者の入力を補わない。
fn stop_midway_property(specs: &[OpSpec], splits: &[(u32, bool)]) -> Result<(), TestCaseError> {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    let ir = load_ir(&bytes).expect("load ir");
    let initial = sample.initial;
    let mut harness = Harness::via_session(&ir, initial.clone());
    let mut off = RecordedSession::new(Session::from_state(ir.clone(), initial), "s-1");
    let mut on = off.clone();
    for (i, spec) in specs.iter().enumerate() {
        let (k, stop_now) = splits.get(i).copied().unwrap_or((u32::MAX, false));
        let applied = harness.step(spec);
        match &applied.op {
            Some(op) => {
                off.apply(op.clone());
                on.apply(op.clone());
            }
            None => {
                off.reload();
                on.reload();
            }
        }
        let k = if stop_now { k } else { u32::MAX };
        let mut last = None;
        let mut sent = 0u32;
        while sent < k {
            let Some(addr) = next_tick(harness.state()) else {
                break;
            };
            let a = harness.step(&OpSpec::Tick(Which::Current));
            if let Some(op) = a.op {
                off.apply(op);
            }
            sent += 1;
            let stuck = is_stuck(harness.state(), &addr);
            last = Some(addr);
            if stuck || sent > 10_000 {
                break;
            }
        }
        let ops_before = on.ops().len();
        let ff = on.fast_forward(k);
        let only_ticks = on.ops()[ops_before..]
            .iter()
            .all(|o| matches!(o, HistoryOp::Engine(Op::Tick(_))));
        let invented = ff
            .events
            .iter()
            .any(|e| matches!(e, Event::InputReceived { .. } | Event::Judged { .. }));
        prop_assert!(only_ticks, "fast_forward added a non-tick op");
        prop_assert!(!invented, "fast_forward invented an input");
        prop_assert_eq!(on.session().state(), off.session().state());
        if stop_now && harness.state().run.busy.is_some() {
            let a = harness.step(&OpSpec::Stop);
            if let Some(op) = a.op {
                off.apply(op.clone());
                on.apply(op);
            }
            if let Some(addr) = last {
                off.apply(Op::Tick(addr.clone()));
                on.apply(Op::Tick(addr));
            }
        }
        prop_assert_eq!(on.session().state(), off.session().state());
        prop_assert_eq!(on.ops(), off.ops());
    }
    Ok(())
}

fn splits() -> impl Strategy<Value = Vec<(u32, bool)>> {
    prop::collection::vec((0u32..8, any::<bool>()), MAX_OPS)
}

#[test]
fn fast_forward_stop_midway_property() {
    check(
        "fast-forward-stop-midway",
        DEFAULT_CASES,
        (op_specs(), splits()),
        |(specs, splits)| stop_midway_property(&specs, &splits),
    );
}

#[test]
fn fast_forward_stop_midway_property_guided() {
    check(
        "fast-forward-stop-midway-guided",
        DEFAULT_CASES,
        (guided_op_specs(), splits()),
        |(specs, splits)| stop_midway_property(&specs, &splits),
    );
}
