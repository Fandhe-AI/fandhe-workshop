//! 同じ入力列を 2 回適用して、状態・イベント列・導出が一致することの性質テスト（TASK-218 1/3。EVT-22・EVT-16、性質 P-5）。
//!
//! 同じ経路（純粋関数 `apply`、`Session::apply`）を、別々に読んだ IR・別々に取った初期状態で 2 回流し、
//! 各操作後の「解決した `Op`・イベント列・状態全体・導出」を `==` と直列化したバイト列の両方で比べる。
//! 2 回目は 1 回目の完了後に別スレッドで流し、スレッド局所の状態や前の実行の残りに依存しないことも確かめる。
//! 経路をまたぐ一致は `op_sequence_replay.rs`（TASK-219 1/3）が担い、本ファイルは同じ経路の 2 回適用を担う。
//! 表示文字列の差し替え（2/3）と ID の変更の負の対照（3/3）は本ファイルでは扱わない。
//! 「1,000 通り×2」は固定の生成と誘導つきの生成の 2 本と読む。成功条件は不変条件（一致）だけで、統計は出力するだけで
//! assert しない（`docs/testing.md` 第 2 節）。時刻・乱数の源が無いことの静的検査は `determinism_sources.rs`。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{guided_op_specs, op_specs, script_specs, Harness, OpSpec, Which};
use common::property::check;
use common::trace::{compare_traces, trace, StepTrace};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use proptest::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// 教材と初期状態を読む。2 回の実行が値の共有に依存しないよう、別々に読む。
fn load_fresh() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

#[derive(Clone, Copy)]
enum Route {
    Pure,
    Session,
}

fn harness(route: Route, ir: &Ir, initial: SessionState) -> Harness<'_> {
    match route {
        Route::Pure => Harness::pure(ir, initial),
        Route::Session => Harness::via_session(ir, initial),
    }
}

/// 1 本目をこのスレッド、2 本目を 1 本目の完了後に別スレッドで流す。
fn run_twice(
    route: Route,
    a: &(Ir, SessionState),
    b: &(Ir, SessionState),
    specs: &[OpSpec],
) -> (Vec<StepTrace>, Vec<StepTrace>) {
    let first = trace(harness(route, &a.0, a.1.clone()), specs);
    let second = std::thread::scope(|s| {
        s.spawn(|| trace(harness(route, &b.0, b.1.clone()), specs))
            .join()
            .expect("second run thread")
    });
    (first, second)
}

fn twice_match(
    a: &(Ir, SessionState),
    b: &(Ir, SessionState),
    specs: &[OpSpec],
) -> Result<Vec<StepTrace>, String> {
    let mut kept = Vec::new();
    for (name, route) in [("pure", Route::Pure), ("session", Route::Session)] {
        let (x, y) = run_twice(route, a, b, specs);
        compare_traces(&x, &y).map_err(|e| format!("{name}: {e}"))?;
        if kept.is_empty() {
            kept = x;
        }
    }
    Ok(kept)
}

#[derive(Default, Debug)]
struct Stats {
    counts: BTreeMap<&'static str, u64>,
}

impl Stats {
    fn add(&mut self, traces: &[StepTrace]) {
        *self.counts.entry("ops").or_default() += traces.len() as u64;
        for t in traces {
            for e in &t.events {
                let key = match e {
                    Event::Progress { .. } => "progress",
                    Event::Ignored { .. } => "ignored",
                    Event::Stopped { .. } => "stopped",
                    Event::Restored { .. } => "restored",
                    Event::Achieved { .. } => "achieved",
                    _ => continue,
                };
                *self.counts.entry(key).or_default() += 1;
            }
        }
    }
}

fn run_property<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let a = load_fresh();
    let b = load_fresh();
    let stats = RefCell::new(Stats::default());
    check(name, 1_000, strategy, |specs| {
        match twice_match(&a, &b, &specs) {
            Ok(t) => stats.borrow_mut().add(&t),
            Err(message) => prop_assert!(false, "{message}"),
        }
        Ok(())
    });
    println!("PROPERTY-TEST stats={:?}", stats.borrow().counts);
}

#[test]
fn same_ops_applied_twice_match_with_fixed_sequences() {
    run_property("determinism-fixed", op_specs());
}

#[test]
fn same_ops_applied_twice_match_with_guided_sequences() {
    run_property("determinism-guided", guided_op_specs());
}

fn any_event(traces: &[StepTrace], pred: impl Fn(&Event) -> bool) -> bool {
    traces.iter().any(|t| t.events.iter().any(&pred))
}

#[test]
fn scripted_ops_applied_twice_match() {
    let (a, b) = (load_fresh(), load_fresh());
    let specs = script_specs();
    assert_eq!(specs.len(), 37);
    let traces = twice_match(&a, &b, &specs).expect("twice match");
    // 進行は時刻ではなく宛先つきの `Tick` の列で進む（EVT-16）。古い宛先の進行は無視される。
    assert!(any_event(&traces, |e| matches!(e, Event::Progress { .. })));
    assert!(any_event(&traces, |e| matches!(
        e,
        Event::Ignored {
            reason: IgnoreReason::Stale,
            ..
        }
    )));
    assert!(any_event(&traces, |e| matches!(e, Event::Stopped { .. })));
    assert!(any_event(&traces, |e| matches!(e, Event::Moved { .. })));
    assert!(any_event(&traces, |e| matches!(
        e,
        Event::CheckpointSaved { .. }
    )));
    assert!(any_event(&traces, |e| matches!(e, Event::Restored { .. })));
}

#[test]
fn applied_twice_compares_projection() {
    let (a, b) = (load_fresh(), load_fresh());
    let traces = twice_match(&a, &b, &script_specs()).expect("twice match");
    assert!(
        traces.iter().any(|t| !t.review.is_empty()),
        "review is recorded"
    );
    assert!(
        traces.iter().any(|t| !t.log_rows.is_empty()),
        "log rows are recorded"
    );
    assert!(traces.len() >= 37);
}

#[test]
fn comparator_detects_a_dropped_op() {
    let (a, b) = (load_fresh(), load_fresh());
    let full = [OpSpec::Cmd(0), OpSpec::Tick(Which::Current), OpSpec::Save];
    let dropped = [OpSpec::Cmd(0), OpSpec::Save];
    let x = trace(Harness::pure(&a.0, a.1.clone()), &full);
    let y = trace(Harness::pure(&b.0, b.1.clone()), &dropped);
    assert!(compare_traces(&x, &y).is_err());
}

#[test]
fn comparator_detects_a_changed_initial_state() {
    let (a, b) = (load_fresh(), load_fresh());
    let specs = [OpSpec::Cmd(0), OpSpec::Tick(Which::Current)];
    let mut changed = b.1.clone();
    changed
        .hints_opened
        .insert(StepUid("main/st-1".to_string()), 1);
    let x = trace(Harness::pure(&a.0, a.1.clone()), &specs);
    let y = trace(Harness::pure(&b.0, changed), &specs);
    assert!(compare_traces(&x, &y).is_err());
}

#[test]
fn comparator_detects_a_projection_only_difference() {
    let a = load_fresh();
    let traces = trace(Harness::pure(&a.0, a.1.clone()), &script_specs());
    let i = traces
        .iter()
        .position(|t| !t.review.is_empty())
        .expect("review exists");
    let mut review_changed = traces.clone();
    review_changed[i].review.clear();
    assert!(compare_traces(&traces, &review_changed).is_err());
    let j = traces
        .iter()
        .position(|t| !t.log_rows.is_empty())
        .expect("log rows exist");
    let mut rows_changed = traces.clone();
    rows_changed[j].log_rows.clear();
    assert!(compare_traces(&traces, &rows_changed).is_err());
}
