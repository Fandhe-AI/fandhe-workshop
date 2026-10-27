//! 周回をまたぐ操作列の性質テスト: 混ざり 0 件（M-1）と流し直しの一致（TASK-243 1/2。MOD-11）。
//!
//! 2 周の教材 `module-reuse-rounds` を `compile_dir` した IR で、1 周目と 2 周目の両方に触れる操作列
//! （入力・進行・停止・移動・保存・復帰・ヒント・下書き・新しいルートの開始。誘導つきを含む）を流し、各操作の後に
//! 「操作の宛先でない根の切り身が操作の前と一致する」ことを確かめる。
//! 置き場が pack なのは、2 周の IR を作る `compile_dir` が pack にあり、core から pack へ依存できない
//! （`make check-deps`）ためである。定義の出所は `testdata/module-reuse-rounds/test-data/property-test.md`。
//! M-2・移動の出発側の切り身・S-1 は 2/2（#770）が `common/round_gen.rs` へ足す。
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
use round_gen::{fixed_cases, guided_cases, run_case, Case, Stats};

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
    ] {
        assert!(all.get(k) > 0, "{k} is 0: not covered");
    }
}
