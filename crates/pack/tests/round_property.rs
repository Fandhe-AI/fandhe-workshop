//! 周回をまたぐ操作列の性質テスト: 混ざり 0 件（M-1）・流し直しの一致（TASK-243 1/2）と、1 周目の不変（M-2）・
//! 移動の出発側の不変・古い進行の無視（S-1）・同じモジュールの完了状態の非混在（TASK-243 2/2。MOD-11）。
//!
//! 2 周の教材 `module-reuse-rounds` を `compile_dir` した IR で、1 周目と 2 周目の両方に触れる操作列
//! （入力・進行・停止・移動・保存・復帰・ヒント・下書き・新しいルートの開始。誘導つきを含む）を流し、各操作の後に
//! 「操作の宛先でない根の切り身が操作の前と一致する」ことを確かめる。
//! 置き場が pack なのは、2 周の IR を作る `compile_dir` が pack にあり、core から pack へ依存できない
//! （`make check-deps`）ためである。定義の出所は `testdata/module-reuse-rounds/test-data/property-test.md`。
//! 移動の出発側・S-1・完了状態の非混在は `Harness::step` が全試験で流し、M-2 は T1 から 2 周目に閉じた操作だけを流す
//! 専用の 2 件（固定・誘導）が確かめる（`docs/testing.md` 5.334）。
//! 未実装の部品で入力だけでは進めない箇所は、誘導が `GoTo` で飛ばす（`docs/testing.md` 5.333）。

mod common;
#[allow(
    dead_code,
    reason = "core の共通ヘルパのうち本ファイルが使わない部品があるため"
)]
#[path = "../../core/tests/common/property.rs"]
mod property;
#[path = "common/round_gen.rs"]
mod round_gen;
#[path = "common/rounds.rs"]
mod rounds;

use std::sync::Mutex;

use common::testdata;
use proptest::test_runner::TestCaseError;
use round_gen::{
    check_m2, fixed_cases, fixture, guided_cases, m2_fixed_cases, m2_guided_cases, run_case,
    run_m2_case, Case, Harness, OpSpec, ROp, Setup, Stats,
};

fn run_property(name: &str, cases: u32, guided: bool) -> Stats {
    let total = Mutex::new(Stats::default());
    let body = |c: Case| -> Result<(), TestCaseError> {
        let s = run_case(&c).map_err(TestCaseError::fail)?;
        total.lock().expect("stats lock").merge(&s);
        Ok(())
    };
    if guided {
        property::check(name, cases, guided_cases(), body);
    } else {
        property::check(name, cases, fixed_cases(), body);
    }
    let s = total.into_inner().expect("stats");
    println!("PROPERTY-TEST stats name={name} {:?}", s.0);
    s
}

fn run_m2_property(name: &str, cases: u32, guided: bool) -> Stats {
    let total = Mutex::new(Stats::default());
    let body = |c: Case| -> Result<(), TestCaseError> {
        let s = run_m2_case(&c).map_err(TestCaseError::fail)?;
        total.lock().expect("stats lock").merge(&s);
        Ok(())
    };
    if guided {
        property::check(name, cases, m2_guided_cases(), body);
    } else {
        property::check(name, cases, m2_fixed_cases(), body);
    }
    let s = total.into_inner().expect("stats");
    println!("PROPERTY-TEST stats name={name} {:?}", s.0);
    s
}

/// 毎操作の後に、移動の出発側・S-1・完了状態の非混在も `Harness::step` が確かめる（5.334）。
#[test]
fn m1_holds_across_rounds_fixed_generation() {
    run_property("m1-rounds-fixed", 1_000, false);
}

#[test]
fn m1_holds_across_rounds_guided_generation() {
    run_property("m1-rounds-guided", 1_000, true);
}

#[test]
fn round_generation_reaches_both_rounds() {
    let mut all = Stats::default();
    for (name, guided) in [("reach-fixed", false), ("reach-guided", true)] {
        let total = Mutex::new(Stats::default());
        let body = |c: Case| -> Result<(), TestCaseError> {
            let s = run_case(&c).map_err(TestCaseError::fail)?;
            total.lock().expect("stats lock").merge(&s);
            Ok(())
        };
        let r = if guided {
            property::try_check_with(name, &property::DEFAULT_SEED, 1_000, &guided_cases(), body)
        } else {
            property::try_check_with(name, &property::DEFAULT_SEED, 1_000, &fixed_cases(), body)
        };
        r.unwrap_or_else(|m| panic!("{m}"));
        all.merge(&total.into_inner().expect("stats"));
    }
    println!("PROPERTY-TEST reach stats {:?}", all.0);
    for k in [
        "cases_both_rounds",
        "ops_touching_both",
        "gotos_cross_round",
        "restores_cross_round",
        "hints_r1",
        "hints_r2",
        "ops_with_r1_draft",
        "ops_with_r2_draft",
        "begin_ok",
        "begin_refused_unknown",
        "stale_ticks",
        "stale_ticks_dup",
        "stale_ticks_past",
        "stale_ticks_other_root",
        "stale_ticks_cross_round",
        "moves_source_checked",
        "moves_cross_round",
        "completion_sibling_checks",
        "completion_sibling_checks_sibling_achieved",
        "s5_restores_checked",
        "s5_restores_with_foreign_world_diff",
        "s5_restores_with_foreign_slice_diff",
        "s5_call_step_restores",
        "s6_stops_checked",
    ] {
        assert!(all.get(k) > 0, "{k} is 0: not covered");
    }
}

/// 誘導だけで通したときの `GoTo` の列が、`rounds.rs` の計画の `GoTo` の列と一致する
/// （1 周目は `T0`、2 周目は 2 周目の開始の後の `T2`）。
#[test]
fn guided_goto_sequence_matches_rounds_plans() {
    use fandhe_workshop_core::input::{Control, Op};
    use rounds::PlanStep;

    let ir = &fixture().ir;
    let gotos = |plan: Vec<PlanStep>| -> Vec<Op> {
        plan.into_iter()
            .filter_map(|p| match p {
                PlanStep::Op(o @ Op::Control(Control::GoTo(_))) => Some(o),
                _ => None,
            })
            .collect()
    };
    for (setup, expected) in [
        (Setup::T0, gotos(rounds::round1_plan(ir))),
        (Setup::T2, gotos(rounds::round2_plan(ir))),
    ] {
        let mut h = Harness::new(setup);
        for i in 0..120 {
            h.step(i, &OpSpec::Guided).unwrap_or_else(|m| panic!("{m}"));
            // T0 は 1 周目の完了で誘導が 2 周目の `Begin` を出す。以降の `GoTo` は 2 周目の分なので、
            // 1 周目の計画と比べる列にはそこまでを使う。
            if setup == Setup::T0
                && matches!(
                    h.ops.last(),
                    Some(ROp::Engine(Op::Control(Control::Begin(_))))
                )
            {
                break;
            }
        }
        let actual: Vec<Op> = h
            .ops
            .iter()
            .filter_map(|o| match o {
                ROp::Engine(o @ Op::Control(Control::GoTo(_))) => Some(o.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(actual.len(), expected.len(), "{setup:?}: {actual:?}");
        if setup == Setup::T0 {
            assert_eq!(actual, expected, "{setup:?}");
        } else {
            // T2 は `s-r2-fresh` なので根の名前が計画（`s-r2`）と違いうる。飛び先の Step の ID だけ比べる。
            let ids = |v: &[Op]| -> Vec<String> {
                v.iter()
                    .map(|o| match o {
                        Op::Control(Control::GoTo(u)) => {
                            u.0.rsplit('/').next().unwrap().to_string()
                        }
                        _ => unreachable!(),
                    })
                    .collect()
            };
            assert_eq!(ids(&actual), ids(&expected), "{setup:?}");
        }
    }
}

/// M-2: 1 周目を終えた直後（T1）から 2 周目に閉じた操作だけを最大 50 回混ぜても、1 周目の 2 実体の切り身と
/// 世界状態の 13 経路が毎操作の後で基点と一致する（固定の生成）。
#[test]
fn m2_round1_invariant_fixed_generation() {
    run_m2_property("m2-round1-fixed", 1_000, false);
}

/// 同上（誘導つきの生成。2 周目の完了まで届く列を増やす）。
#[test]
fn m2_round1_invariant_guided_generation() {
    run_m2_property("m2-round1-guided", 1_000, true);
}

/// M-2 の生成が 2 周目の奥まで届き、古い進行・復帰・ヒントが 0 件でない。
#[test]
fn m2_generation_covers_round2() {
    let mut all = Stats::default();
    for guided in [false, true] {
        let total = Mutex::new(Stats::default());
        let body = |c: Case| -> Result<(), TestCaseError> {
            let s = run_m2_case(&c).map_err(TestCaseError::fail)?;
            total.lock().expect("stats lock").merge(&s);
            Ok(())
        };
        let r = if guided {
            property::try_check_with(
                "m2-reach",
                &property::DEFAULT_SEED,
                1_000,
                &m2_guided_cases(),
                body,
            )
        } else {
            property::try_check_with(
                "m2-reach",
                &property::DEFAULT_SEED,
                1_000,
                &m2_fixed_cases(),
                body,
            )
        };
        r.unwrap_or_else(|m| panic!("{m}"));
        all.merge(&total.into_inner().expect("stats"));
    }
    println!("PROPERTY-TEST m2 reach stats {:?}", all.0);
    for k in [
        "m2_reached_issue_to_merge_r2",
        "m2_round2_completed",
        "restores_ok",
        "hints_r2",
        "stale_ticks",
        "stale_ticks_cross_round",
    ] {
        assert!(all.get(k) > 0, "{k} is 0: not covered");
    }
}

/// 負の対照: `s-r2-fresh` は 1 周目の世界状態を設計どおりに書く入口なので、M-2 の検査が 2 経路の違いを検出する
/// （検査が空振りでないことと、M-2 の開始の候補から外した判断が正しいことの確認）。
#[test]
fn m2_checker_detects_fresh_start_writes() {
    let mut h = Harness::new_m2();
    assert!(check_m2(fixture(), h.state()).is_empty());
    h.rs.apply(fandhe_workshop_core::input::Op::Control(
        fandhe_workshop_core::input::Control::Begin("s-r2-fresh".into()),
    ));
    let v = check_m2(fixture(), h.state());
    let paths: Vec<&str> = v
        .iter()
        .filter_map(|m| m.strip_prefix("world path "))
        .collect();
    assert_eq!(
        paths,
        [
            "ideas/bbs-with-login/03-poc/login-session/result.md",
            "prs/pr-1-a"
        ],
        "{v:?}"
    );
}

/// 誘導だけで 2 周目を完了させても 1 周目の基点が保たれ、2 周目の `st-4-1`・`st-5-1`（`setup: "false"`）は
/// 未着手のまま、1 周目の同じ Step は達成のまま残る（完了状態の非混在の決定的な確認）。
#[test]
fn m2_guided_completes_round2_keeping_round1() {
    use fandhe_workshop_core::step_status::StepStatus;

    let f = fixture();
    let mut h = Harness::new_m2();
    for i in 0..120 {
        h.step(i, &OpSpec::Guided).unwrap_or_else(|m| panic!("{m}"));
    }
    assert!(check_m2(f, h.state()).is_empty());
    let status = |round: &str, id: &str| {
        let uid = fandhe_workshop_core::addr::StepUid(format!("issue-to-merge@{round}/{id}"));
        h.rs.session().step_status(&uid)
    };
    for id in ["st-4-1", "st-5-1"] {
        assert_eq!(status("round2", id), StepStatus::NotStarted, "{id}");
        assert_eq!(status("round1", id), StepStatus::Achieved, "{id}");
    }
    let s = h.finish_stats();
    assert!(s.get("m2_round2_completed") > 0, "{:?}", s.0);
}
