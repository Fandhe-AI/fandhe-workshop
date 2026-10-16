//! 任意の操作列を流し、各操作の後に「最初から流し直した状態と一致する」ことを確かめる試験（TASK-219 1/3、性質 P-5）。
//!
//! 学習者の入力・外れた入力・進行・古い宛先の進行・停止・Step の移動・保存・復帰・再送の 9 種を含む 1〜50 操作の列を
//! 1,000 通り流す。同じ操作列を、純粋関数 `apply` の経路（1 本目）と、web が使う `Session::apply` の経路（2 本目）で
//! 最初の状態から独立に流し、各操作について「解決した `Op`・操作後の状態全体・イベント列」の一致を比べる。
//! `apply` は純粋なので、時点 k の 2 本目の状態は「接頭辞 k を最初から流し直した状態」に当たる。
//!
//! 性質テストの成功条件は不変条件（一致）だけにする。操作の網羅性は固定列の試験
//! （`scripted_sequence_replays_and_covers_all_kinds`）と生成器の自己確認で確かめ、統計は出力するだけで assert しない
//! （`docs/testing.md` 第 2 節）。P-1〜P-6 の検査は #207、誘導つきの生成は #208、変異注入は TASK-220 が扱う。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{op_specs, script_specs, Applied, Harness, OpSpec, Which};
use common::property::{check, try_check_with, DEFAULT_SEED, MAX_OPS};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use proptest::prelude::*;
use std::cell::RefCell;
use std::collections::BTreeMap;

/// 教材（IR）を読む。ケースごとではなく、試験の先頭で 1 回だけ呼ぶ。
fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 列を最後まで流し、操作ごとの結果を返す。
fn run_all(harness: &mut Harness<'_>, specs: &[OpSpec]) -> Vec<Applied> {
    specs.iter().map(|s| harness.step(s)).collect()
}

/// 2 本の実行を、各操作の後で比べる。食い違いは何番目の操作の何の欄かを文字列で返す。
fn compare_runs(a: &[Applied], b: &[Applied]) -> Result<(), String> {
    for (i, (x, y)) in a.iter().zip(b.iter()).enumerate() {
        if x.op != y.op {
            return Err(format!("op #{i}: op differs: {:?} vs {:?}", x.op, y.op));
        }
        if x.events != y.events {
            return Err(format!(
                "op #{i} {:?}: events differ:\n  {:?}\n  {:?}",
                x.op, x.events, y.events
            ));
        }
        if x.after != y.after {
            let field = if x.after.run != y.after.run {
                "run"
            } else if x.after.log != y.after.log {
                "log"
            } else if x.after.entry != y.after.entry {
                "entry"
            } else if x.after.checkpoints != y.after.checkpoints {
                "checkpoints"
            } else if x.after.drafts != y.after.drafts {
                "drafts"
            } else if x.after.hints_opened != y.after.hints_opened {
                "hints_opened"
            } else {
                "other"
            };
            return Err(format!("op #{i} {:?}: state differs in {field}", x.op));
        }
    }
    if a.len() != b.len() {
        return Err(format!("run length differs: {} vs {}", a.len(), b.len()));
    }
    Ok(())
}

/// 同じ列を 2 つの経路で最初の状態から流し、各操作の後の一致を比べる。
fn replay_matches(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
) -> Result<Vec<Applied>, String> {
    let first = run_all(&mut Harness::pure(ir, initial.clone()), specs);
    let second = run_all(&mut Harness::via_session(ir, initial.clone()), specs);
    compare_runs(&first, &second)?;
    Ok(first)
}

/// 統計。出力するだけで、assert には使わない。
#[derive(Default, Debug)]
struct Stats {
    counts: BTreeMap<&'static str, u64>,
}

impl Stats {
    fn add(&mut self, applied: &[Applied]) {
        for a in applied {
            for e in &a.events {
                let key = match e {
                    Event::Ignored { .. } => "ignored",
                    Event::Stopped { .. } => "stopped",
                    Event::Moved { .. } => "moved",
                    Event::CheckpointSaved { .. } => "saved",
                    Event::Restored { .. } => "restored",
                    Event::Refused { .. } => "refused",
                    Event::Achieved { .. } => "achieved",
                    Event::Progress { .. } => "progress",
                    Event::Judged {
                        judgment: Judgment::Rejected(r),
                        ..
                    } if r.category == RejectCategory::Busy => "busy",
                    _ => continue,
                };
                *self.counts.entry(key).or_default() += 1;
            }
        }
    }
}

#[test]
fn replay_from_start_matches_after_every_op() {
    let (ir, initial) = load_min_ir();
    let stats = RefCell::new(Stats::default());
    check(
        "op-sequence-replay",
        1_000,
        op_specs(),
        |specs| match replay_matches(&ir, &initial, &specs) {
            Ok(applied) => {
                stats.borrow_mut().add(&applied);
                Ok(())
            }
            Err(message) => {
                prop_assert!(false, "{message}");
                Ok(())
            }
        },
    );
    println!("PROPERTY-TEST stats={:?}", stats.borrow().counts);
}

fn any_event(applied: &[Applied], pred: impl Fn(&Event) -> bool) -> bool {
    applied.iter().any(|a| a.events.iter().any(&pred))
}

#[test]
fn scripted_sequence_replays_and_covers_all_kinds() {
    let (ir, initial) = load_min_ir();
    let specs = script_specs();
    assert_eq!(specs.len(), 37);
    let applied = replay_matches(&ir, &initial, &specs).expect("replay matches");
    assert!(
        any_event(&applied, |e| matches!(
            e,
            Event::Ignored {
                reason: IgnoreReason::Stale,
                ..
            }
        )),
        "stale tick"
    );
    assert!(
        any_event(&applied, |e| matches!(e, Event::Stopped { .. })),
        "stop"
    );
    assert!(
        any_event(&applied, |e| matches!(e, Event::Moved { .. })),
        "move"
    );
    assert!(
        any_event(&applied, |e| matches!(e, Event::CheckpointSaved { .. })),
        "save"
    );
    assert!(
        any_event(&applied, |e| matches!(e, Event::Restored { .. })),
        "restore"
    );
    assert!(
        any_event(&applied, |e| matches!(e, Event::Progress { .. })),
        "progress"
    );
    assert!(
        any_event(&applied, |e| matches!(
            e,
            Event::Judged { judgment: Judgment::Rejected(r), .. } if r.category != RejectCategory::Busy
        )),
        "rejected input"
    );
    assert!(
        any_event(&applied, |e| matches!(
            e,
            Event::Judged { judgment: Judgment::Rejected(r), .. } if r.category == RejectCategory::Busy
        )),
        "busy resend"
    );
}

#[test]
fn generator_produces_every_kind() {
    #[derive(Default)]
    struct Seen {
        input: bool,
        tick: bool,
        stale_tick: bool,
        stop: bool,
        go_to: bool,
        save: bool,
        restore: bool,
        bad_input: bool,
        resend: bool,
    }
    let seen = RefCell::new(Seen::default());
    try_check_with(
        "generator-kinds",
        &DEFAULT_SEED,
        256,
        &op_specs(),
        |specs| {
            prop_assert!(
                !specs.is_empty() && specs.len() <= MAX_OPS,
                "length {}",
                specs.len()
            );
            let mut s = seen.borrow_mut();
            for (i, spec) in specs.iter().enumerate() {
                match spec {
                    OpSpec::Cmd(0) | OpSpec::Choice(0 | 1) | OpSpec::Text => s.input = true,
                    OpSpec::Cmd(_) | OpSpec::Choice(_) => s.bad_input = true,
                    OpSpec::Tick(Which::Current) => s.tick = true,
                    OpSpec::Tick(_) => s.stale_tick = true,
                    OpSpec::Stop => s.stop = true,
                    OpSpec::GoTo(_) => s.go_to = true,
                    OpSpec::Save => s.save = true,
                    OpSpec::Restore(_) => s.restore = true,
                    _ => {}
                }
                if i > 0 && spec.is_learner_input() && specs[i - 1] == *spec {
                    s.resend = true;
                }
            }
            Ok(())
        },
    )
    .expect("generator property");
    let s = seen.borrow();
    assert!(
        s.input && s.bad_input && s.tick && s.stale_tick,
        "inputs and ticks"
    );
    assert!(
        s.stop && s.go_to && s.save && s.restore && s.resend,
        "controls and resend"
    );
}

#[test]
fn replay_comparison_detects_a_dropped_op() {
    let (ir, initial) = load_min_ir();
    let full = [OpSpec::Cmd(0), OpSpec::Tick(Which::Current), OpSpec::Save];
    let first = run_all(&mut Harness::pure(&ir, initial.clone()), &full);
    let second = run_all(&mut Harness::via_session(&ir, initial.clone()), &full);
    assert_eq!(compare_runs(&first, &second), Ok(()));
    let dropped = [OpSpec::Cmd(0), OpSpec::Save];
    let short = run_all(&mut Harness::via_session(&ir, initial), &dropped);
    let result = compare_runs(&first, &short);
    assert!(result.is_err(), "a dropped op must be detected");
    // `Op` を 1 件落とした列は、2 件目（進行と保存）で食い違う。
    assert!(matches!(first[1].op, Op::Tick(_)));
}
