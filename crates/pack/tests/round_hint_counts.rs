//! 周回をまたぐ操作列でヒントの使用回数が減らない性質テスト S-4（TASK-297 2/2。GUIDE-7）。
//!
//! 2 周の教材 `module-reuse-rounds` を `round_property` と同じ生成（固定・誘導）で流し、毎操作の前後で
//! 「前の各鍵が後に残り、値が前以上」と「再読み込み（`session::reload`）で `hints_opened` が変わらない」を確かめる。
//! 置き場が pack なのは、2 周の IR を作る `compile_dir` が pack にあるため。1 周内の分岐ごとの試験は core の
//! `hint_count_monotone`。書き出し → 読み込みの往復は `round_restore_property`（5.341）が状態の全体で確かめるので重複させない。

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

use std::collections::BTreeMap;
use std::sync::Mutex;

use common::testdata;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::reload;
use proptest::test_runner::TestCaseError;
use round_gen::{fixed_cases, guided_cases, Case, Harness, ROp};

/// 1 ケースを流し、毎操作で S-4 を検査する。返すのは「ヒントが開いた状態で流れた操作」の種類ごとの件数。
fn run_s4(case: &Case) -> Result<BTreeMap<&'static str, u64>, String> {
    let mut h = Harness::new(case.setup);
    let mut seen: BTreeMap<&'static str, u64> = BTreeMap::new();
    for (i, spec) in case.specs.iter().enumerate() {
        let before = h.state().hints_opened.clone();
        h.step(i, spec)?;
        let after = h.state().hints_opened.clone();
        for (k, v) in &before {
            if !after.get(k).is_some_and(|n| n >= v) {
                return Err(format!(
                    "S-4 violated at op {i} ({spec:?}): a count decreased"
                ));
            }
        }
        let (reloaded, _) = reload(h.state().clone());
        if reloaded.hints_opened != after {
            return Err(format!(
                "S-4 violated at op {i} ({spec:?}): reload changed counts"
            ));
        }
        if !before.is_empty() {
            let kind = match h.ops.last() {
                Some(ROp::Engine(Op::Control(Control::Stop))) => Some("stop"),
                Some(ROp::Engine(Op::Control(Control::Restore(_)))) => Some("restore"),
                Some(ROp::Engine(Op::Control(Control::GoTo(_)))) => Some("goto"),
                Some(ROp::Engine(Op::Control(Control::Begin(_)))) => Some("begin"),
                _ => None,
            };
            if let Some(k) = kind {
                *seen.entry(k).or_insert(0) += 1;
            }
        }
    }
    Ok(seen)
}

fn run_property(name: &str, guided: bool) -> BTreeMap<&'static str, u64> {
    let total = Mutex::new(BTreeMap::<&'static str, u64>::new());
    let body = |c: Case| -> Result<(), TestCaseError> {
        let s = run_s4(&c).map_err(TestCaseError::fail)?;
        let mut t = total.lock().expect("lock");
        for (k, v) in s {
            *t.entry(k).or_insert(0) += v;
        }
        Ok(())
    };
    if guided {
        property::check(name, 1_000, guided_cases(), body);
    } else {
        property::check(name, 1_000, fixed_cases(), body);
    }
    let t = total.into_inner().expect("stats");
    println!("PROPERTY-TEST stats name={name} {t:?}");
    t
}

#[test]
fn s4_hint_counts_never_decrease_fixed_generation() {
    run_property("s4-hints-fixed", false);
}

#[test]
fn s4_hint_counts_never_decrease_guided_generation() {
    run_property("s4-hints-guided", true);
}

/// 前提が空振りでないこと: ヒントが開いた状態で停止・復帰・移動・周回の開始が流れている。
#[test]
fn s4_generation_reaches_each_operation_with_hints_open() {
    let mut all: BTreeMap<&'static str, u64> = BTreeMap::new();
    for (name, guided) in [("s4-reach-fixed", false), ("s4-reach-guided", true)] {
        for (k, v) in run_property(name, guided) {
            *all.entry(k).or_insert(0) += v;
        }
    }
    for k in ["stop", "restore", "goto", "begin"] {
        assert!(
            all.get(k).copied().unwrap_or(0) > 0,
            "{k} never ran with hints open: {all:?}"
        );
    }
}
