//! 任意の操作列の各操作の後に、性質 P-1a〜P-1d・P-2・P-3・P-4・P-6 を検査する試験（TASK-219 2/3）。
//!
//! 1〜50 操作の列（学習者の入力・外れた入力・進行・古い宛先の進行・停止・Step の移動・保存・復帰・再送）を
//! 1,000 通り、純粋関数 `apply` の経路で流し、操作のたびに `common::op_props::PropChecker` で性質を確かめる。
//! 違反は 1 件でも即失敗にする。「古い進行による状態の変化 0 件」「確認の前の達成 0 件」は、この試験が通ることで
//! 確かめられる。成功条件は不変条件だけで、統計は出力するだけである（`docs/testing.md` 第 2 節）。
//!
//! 固定列（`scripted-inputs.json`）にも同じ検査器を通し、各性質の検査が実際に行われたこと（網羅）を確かめる。
//! 検査器の感度の試験は、実際に流した結果を 1 か所だけ壊し、対応する性質が違反を検出することを確かめる
//! （変異注入 TASK-220 までの代わり）。
//!
//! 誘導つきの生成（操作の 40% を現在の状態での正しい次の操作にする。TASK-219 3/3）でも同じ検査を 1,000 通り流す。
//! 固定の生成は深い状態（確認による達成・最後の Step の達成）に届く列が少ないため、誘導つきの生成がそれを補うことを、
//! 既定の種に固定した比較の試験で確かめる（PoC-1 の発見事項 F-11）。
//!
//! 範囲外: P-5（流し直しの一致）は `op_sequence_replay.rs`、変異注入は TASK-220。
//! 呼び出し元は CI の `make test` のみ。

mod common;

use std::cell::RefCell;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{
    guided_op_specs, op_specs, run_with, script_specs, Applied, Harness, OpSpec, Which, NODES,
};
use common::op_props::{PropChecker, PropStats};
use common::property::{check, try_check_with, DEFAULT_SEED};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::SessionState;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// 教材（IR）を読む。ケースごとではなく、試験の先頭で 1 回だけ呼ぶ。
fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 列を流し、各操作の後に検査する。違反は最初のものを文字列で返す。
fn run_checked(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
) -> Result<(Vec<Applied>, PropStats), String> {
    let mut checker = PropChecker::new(ir, initial);
    let mut harness = Harness::pure(ir, initial.clone());
    let mut all = Vec::new();
    for (i, spec) in specs.iter().enumerate() {
        let applied = harness.step(spec);
        checker.check(i, &applied)?;
        all.push(applied);
    }
    Ok((all, checker.stats))
}

/// 戦略を 1,000 通り流し、操作のたびに性質を検査する。統計は出力するだけである。
fn run_property_test<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let (ir, initial) = load_min_ir();
    let stats = RefCell::new(PropStats::default());
    check(name, 1_000, strategy, |specs| {
        let mut checker = PropChecker::new(&ir, &initial);
        run_with(&ir, initial.clone(), &specs, |i, a| {
            checker.check(i, a).map_err(TestCaseError::fail)
        })?;
        stats.borrow_mut().merge(&checker.stats);
        Ok(())
    });
    let stats = stats.borrow();
    println!(
        "PROPERTY-TEST stats={:?} max_log={}",
        stats.counts, stats.max_log
    );
}

#[test]
fn properties_p1_to_p6_hold_after_every_op() {
    run_property_test("op-sequence-properties", op_specs());
}

#[test]
fn properties_p1_to_p6_hold_with_guided_sequences() {
    run_property_test("op-sequence-properties-guided", guided_op_specs());
}

/// 深さの指標（1,000 通りの合計）。
#[derive(Default, Debug)]
struct Depth {
    /// 確認の成立による達成の件数。
    achieved_by_confirm: u64,
    /// 確認を省く Step の達成の件数。
    achieved_by_none: u64,
    /// 最後の Step（`main/st-3`）が達成まで届いた列の数。章の完了（`ChapterCompleted`・`run.completed`）は
    /// エンジンが未実装（TASK-233 以降）のため、代わりにここを章の終わりへの深さの指標にする。
    reached_last_step_runs: u64,
    /// 達成の直後に同じ操作で次の Step へ入った件数（変化の印を持ったままの Step の移り）。
    achieved_then_entered: u64,
}

/// 既定の種に固定して 1,000 通りを流し、性質を検査しながら深さを集計する。
fn depth_stats<S>(ir: &Ir, initial: &SessionState, name: &str, strategy: &S) -> Depth
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let depth = RefCell::new(Depth::default());
    try_check_with(name, &DEFAULT_SEED, 1_000, strategy, |specs| {
        let mut checker = PropChecker::new(ir, initial);
        let mut chained = 0;
        let mut last_step = false;
        run_with(ir, initial.clone(), &specs, |i, a| {
            if let Some(p) = a
                .events
                .iter()
                .position(|e| matches!(e, Event::Achieved { .. }))
            {
                if a.events[p + 1..]
                    .iter()
                    .any(|e| matches!(e, Event::Entered { .. }))
                {
                    chained += 1;
                }
            }
            last_step |= a
                .events
                .iter()
                .any(|e| matches!(e, Event::Achieved { at, .. } if at.step.0 == NODES[2]));
            checker.check(i, a).map_err(TestCaseError::fail)
        })?;
        let mut d = depth.borrow_mut();
        d.achieved_by_confirm += checker.stats.get("achieved_by_confirm");
        d.achieved_by_none += checker.stats.get("achieved_by_none");
        d.reached_last_step_runs += u64::from(last_step);
        d.achieved_then_entered += chained;
        Ok(())
    })
    .expect("properties hold");
    depth.into_inner()
}

/// 誘導つきの生成は、固定の生成で足りない深い状態（確認による達成・最後の Step の達成・達成の直後の Step の移り）を補う。
/// 網羅性の確認なので、種と件数を既定に固定して環境変数の上書きを受けない。
#[test]
fn guided_generation_reaches_deeper_states_than_fixed_generation() {
    let (ir, initial) = load_min_ir();
    let fixed = depth_stats(&ir, &initial, "depth-fixed", &op_specs());
    let guided = depth_stats(&ir, &initial, "depth-guided", &guided_op_specs());
    println!("DEPTH fixed={fixed:?}\nDEPTH guided={guided:?}");
    let both = format!("fixed={fixed:?} guided={guided:?}");
    assert!(
        guided.achieved_by_confirm > fixed.achieved_by_confirm,
        "achieved_by_confirm: {both}"
    );
    assert!(
        guided.reached_last_step_runs > fixed.reached_last_step_runs,
        "reached_last_step_runs: {both}"
    );
    assert!(
        guided.achieved_then_entered > fixed.achieved_then_entered,
        "achieved_then_entered: {both}"
    );
    assert!(
        guided.achieved_by_confirm > 500,
        "guided achieved_by_confirm floor: {both}"
    );
    assert!(
        guided.reached_last_step_runs > 200,
        "guided reached_last_step_runs floor: {both}"
    );
}

#[test]
fn scripted_sequence_satisfies_properties_and_covers_each_check() {
    let (ir, initial) = load_min_ir();
    let specs = script_specs();
    assert_eq!(specs.len(), 37);
    let (_, stats) = run_checked(&ir, &initial, &specs).expect("properties hold");
    assert!(stats.get("tick_stale") >= 1, "stale tick: {stats:?}");
    assert!(stats.get("tick_current") >= 1, "current tick: {stats:?}");
    assert!(
        stats.get("restore_review_checked") >= 1,
        "restore review: {stats:?}"
    );
    assert!(
        stats.get("achieved_by_confirm") >= 1,
        "achievement by confirmation: {stats:?}"
    );
    assert!(
        stats.get("gen_stop") >= 1 && stats.get("gen_move") >= 1 && stats.get("gen_restore") >= 1,
        "generation bumps: {stats:?}"
    );
}

/// 流した `Applied` の今回のイベント列を `f` で書き換え、`after.log` も合わせる（ログの追記の前提は保つ）。
fn with_events(a: &Applied, f: impl FnOnce(&mut Vec<Event>)) -> Applied {
    let mut b = a.clone();
    f(&mut b.events);
    b.after.log.truncate(b.before.log.len());
    b.after.log.extend(b.events.iter().cloned());
    b
}

/// `all[..k]` を正常に通した検査器へ、壊した `broken` を渡した結果。
fn check_broken(
    ir: &Ir,
    initial: &SessionState,
    all: &[Applied],
    k: usize,
    broken: &Applied,
) -> String {
    let mut checker = PropChecker::new(ir, initial);
    for (i, a) in all[..k].iter().enumerate() {
        checker.check(i, a).expect("unbroken prefix passes");
    }
    checker
        .check(k, broken)
        .expect_err("a broken op must be detected")
}

#[test]
fn checker_detects_each_broken_property() {
    let (ir, initial) = load_min_ir();
    let specs = script_specs();
    let (all, _) = run_checked(&ir, &initial, &specs).expect("properties hold");
    let find = |pred: &dyn Fn(&Applied) -> bool| -> usize {
        all.iter().position(pred).expect("op for the mutation")
    };

    // P-2: 古い進行が状態を変えた。
    let k = find(&|a| {
        matches!(&a.op, Op::Tick(t) if next_tick(&a.before).as_ref() != Some(t))
            && a.before.run.current.is_some()
    });
    let mut b = all[k].clone();
    b.after.run.completed = !b.after.run.completed;
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-2"));
    let b = with_events(&all[k], |e| e.push(e[0].clone()));
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-2"));

    // P-3: 確認を要する Step の達成から `Judged { Confirmed }` を除いた。
    let k = find(&|a| {
        a.events
            .iter()
            .any(|e| matches!(e, Event::Achieved { at, .. } if at.step.0 != "main/st-2"))
            && a.events.iter().any(|e| {
                matches!(
                    e,
                    Event::Judged {
                        judgment: Judgment::Confirmed,
                        ..
                    }
                )
            })
    });
    let b = with_events(&all[k], |e| {
        e.retain(|x| {
            !matches!(
                x,
                Event::Judged {
                    judgment: Judgment::Confirmed,
                    ..
                }
            )
        })
    });
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-3"));

    // P-3: 確認を省く Step の達成から、同じ操作の `Accepted` を除いた。
    let k = find(&|a| {
        a.events.iter().enumerate().any(|(i, e)| {
            matches!(e, Event::Achieved { at, .. }
                if at.step.0 == "main/st-2"
                    && a.events[..i]
                        .iter()
                        .any(|p| matches!(p, Event::Accepted { at: pa } if pa == at)))
        })
    });
    let b = with_events(&all[k], |e| {
        e.retain(|x| !matches!(x, Event::Accepted { .. }))
    });
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-3"));

    // P-4: 入力で世代が 1 増えた。
    let k = find(&|a| matches!(a.op, Op::Input(_)));
    let mut b = all[k].clone();
    b.after.run.generation.0 += 1;
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-4"));

    // P-6: 同じ（世代, Step）で `Entered` が 2 件ある。
    let k = find(&|a| a.events.iter().any(|e| matches!(e, Event::Entered { .. })));
    let b = with_events(&all[k], |e| {
        let entered = e
            .iter()
            .find(|x| matches!(x, Event::Entered { .. }))
            .cloned()
            .expect("Entered");
        e.push(entered);
    });
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-6"));

    // P-1b: 世界状態から 1 経路が消えた。
    let k = find(&|a| {
        !a.after.run.world.is_empty() && a.events.iter().any(|e| matches!(e, Event::Changed { .. }))
    });
    let mut b = all[k].clone();
    let key = b.after.run.world.keys().next().cloned().expect("a path");
    b.after.run.world.remove(&key);
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-1b"));

    // P-1c: 保存の操作で、印の無い経路が世界状態に現れた。
    let k = find(&|a| matches!(a.op, Op::Control(Control::Save)));
    let mut b = all[k].clone();
    b.after
        .run
        .world
        .insert("unmarked/path".to_string(), Default::default());
    assert!(check_broken(&ir, &initial, &all, k, &b).contains("P-1c"));

    // P-1d: 達成の後で、ログから `Achieved` が消えた（復習の達成の印がログから導けない）。
    let j = find(&|a| a.events.iter().any(|e| matches!(e, Event::Achieved { .. })));
    let detected = (j + 1..all.len()).any(|k| {
        let mut b = all[k].clone();
        b.before
            .log
            .retain(|e| !matches!(e, Event::Achieved { .. }));
        b.after.log.retain(|e| !matches!(e, Event::Achieved { .. }));
        let mut checker = PropChecker::new(&ir, &initial);
        for (i, a) in all[..k].iter().enumerate() {
            checker.check(i, a).expect("unbroken prefix passes");
        }
        checker
            .check(k, &b)
            .err()
            .is_some_and(|m| m.contains("P-1d"))
    });
    assert!(detected, "P-1d must detect a review item without Achieved");
}

#[test]
fn stale_ticks_after_stop_move_and_restore_are_ignored() {
    let (ir, initial) = load_min_ir();
    let specs = [
        OpSpec::Cmd(0),
        OpSpec::Tick(Which::Current),
        OpSpec::Stop,
        OpSpec::Tick(Which::StaleGen),
        OpSpec::Tick(Which::Dup),
        OpSpec::GoTo(1),
        OpSpec::Tick(Which::StaleAny(0)),
        OpSpec::Save,
        OpSpec::Restore(1),
        OpSpec::Tick(Which::StaleNode),
    ];
    let (_, stats) = run_checked(&ir, &initial, &specs).expect("properties hold");
    assert!(stats.get("tick_stale") >= 3, "{stats:?}");
}

/// 復帰の後に確認を省く Step が `Tick` で達成すると、`Accepted` は旧世代のままである。P-3 は世代を除いて
/// 受け付け済みを探すため、この有効な列を不合格にしない。
#[test]
fn p3_accepts_achievement_whose_acceptance_has_an_older_generation() {
    let (ir, initial) = load_min_ir();
    let (all, _) = run_checked(&ir, &initial, &script_specs()).expect("properties hold");
    let mut checker = PropChecker::new(&ir, &initial);
    let mut tested = 0;
    for (i, a) in all.iter().enumerate() {
        let achieved = a.events.iter().any(|e| matches!(e, Event::Achieved { .. }));
        let confirmed = a.events.iter().any(|e| {
            matches!(
                e,
                Event::Judged {
                    judgment: Judgment::Confirmed,
                    ..
                }
            )
        });
        if achieved && !confirmed {
            // 達成の世代だけを進め、旧世代の `Accepted` と食い違う復帰後の形にする。
            let shifted = with_events(a, |events| {
                for e in events.iter_mut() {
                    if let Event::Achieved { at, .. } = e {
                        at.generation.0 += 1;
                    }
                }
            });
            checker
                .check_p3(&shifted)
                .expect("an older-generation Accepted still counts");
            tested += 1;
        }
        checker.check(i, a).expect("unbroken op passes");
    }
    assert!(tested >= 1, "no confirm-none achievement in the script");
}

/// 一度 `Accepted` した Step へ入り直す（`Entered` が `Accepted` より後ろに来る）と、旧世代の `Accepted` は
/// 失効する。受け付け前の誤った達成を P-3 が見逃さないことを確かめる。
#[test]
fn p3_rejects_achievement_after_reentry_invalidates_acceptance() {
    let (ir, initial) = load_min_ir();
    let (all, _) = run_checked(&ir, &initial, &script_specs()).expect("properties hold");
    let mut checker = PropChecker::new(&ir, &initial);
    let mut tested = 0;
    for (i, a) in all.iter().enumerate() {
        let none_achieved = a.events.iter().find_map(|e| match e {
            Event::Achieved { at, .. } => Some(at.clone()),
            _ => None,
        });
        let confirmed = a.events.iter().any(|e| {
            matches!(
                e,
                Event::Judged {
                    judgment: Judgment::Confirmed,
                    ..
                }
            )
        });
        if let (Some(at), false) = (none_achieved, confirmed) {
            // 達成の直前に同じ Step へ入り直した形にし、同じ操作の `Accepted` も取り除く。
            let broken = with_events(a, |events| {
                events.retain(|x| !matches!(x, Event::Accepted { .. }));
                let pos = events
                    .iter()
                    .position(|x| matches!(x, Event::Achieved { .. }))
                    .expect("Achieved");
                events.insert(pos, Event::Entered { at: at.clone() });
            });
            assert!(
                checker.check_p3(&broken).is_err(),
                "re-entry must invalidate the older Accepted"
            );
            tested += 1;
        }
        checker.check(i, a).expect("unbroken op passes");
    }
    assert!(tested >= 1, "no confirm-none achievement in the script");
}

/// 受け付け前に保存し、受け付けと達成の後に受け付け前へ復帰すると、ログには失効した `Accepted` が残るが、
/// 復元後の状態は受け付け済みでない。この状態からの受け付けなしの達成を P-3 が拒否し、復元後の状態が
/// 受け付け済みなら受け入れることを確かめる。
#[test]
fn p3_rejects_achievement_when_restore_revoked_the_logged_acceptance() {
    use fandhe_workshop_core::step_status::StepStatus;
    let (ir, initial) = load_min_ir();
    let (all, _) = run_checked(&ir, &initial, &script_specs()).expect("properties hold");
    let mut tested = 0;
    for a in &all {
        let achieved = a.events.iter().find_map(|e| match e {
            Event::Achieved { at, .. } => Some(at.clone()),
            _ => None,
        });
        let confirmed = a.events.iter().any(|e| {
            matches!(
                e,
                Event::Judged {
                    judgment: Judgment::Confirmed,
                    ..
                }
            )
        });
        let (Some(at), false) = (achieved, confirmed) else {
            continue;
        };
        // 操作前のログには `Accepted` が残り、復元後の状態は受け付け前（`Entered`）という形にする。
        let mut b = with_events(a, |events| {
            events.retain(|x| !matches!(x, Event::Accepted { .. } | Event::Entered { .. }));
        });
        let old = Event::Accepted { at: at.clone() };
        b.before.log.push(old.clone());
        b.after.log.insert(b.before.log.len() - 1, old);
        for slice in b.before.run.slices.values_mut() {
            if slice.status.contains_key(&at.step) {
                slice.status.insert(at.step.clone(), StepStatus::Entered);
            }
        }
        let mut checker = PropChecker::new(&ir, &initial);
        assert!(
            checker.check_p3(&b).is_err(),
            "an Accepted revoked by a restore must not justify the achievement"
        );
        for slice in b.before.run.slices.values_mut() {
            if slice.status.contains_key(&at.step) {
                slice.status.insert(at.step.clone(), StepStatus::Accepted);
            }
        }
        let mut checker = PropChecker::new(&ir, &initial);
        checker
            .check_p3(&b)
            .expect("a restored Accepted state still justifies the achievement");
        tested += 1;
    }
    assert!(tested >= 1, "no confirm-none achievement in the script");
}
