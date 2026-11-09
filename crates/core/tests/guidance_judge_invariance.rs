//! 案内レベルを変えても判定の結果が変わらないことの性質テスト（TASK-294 2/2、GUIDE-3）。
//!
//! 同じ操作列を、基準（`SetGuidance` を挟まない）・先頭で A/B/C に切り替えた列・途中の乱数の位置で乱数の水準に
//! 切り替えた列で流し、操作ごとにイベント列の一致と、水準の欄だけを A に揃えた状態の一致を確かめる。
//! 挟んだ `SetGuidance` 自身は、イベント 0 件で水準以外の状態を変えない。
//!
//! 主張に含めないもの: `OpenHint`（ヒントの数は水準で変わる設計。生成器も出さない）・`hints_opened`・表示の充填
//! （`text_fill` の `step_do`。水準で変えるのが仕様）・`Begin`（生成器に無い。`begin_guidance.rs` の範囲）。
//! 水準で読む定義（`do_levels`・`levels` つきのヒント）を足した教材でも同じ検査を流す。
//! 中身の無い試験にならないよう、`Judged` を含むケースが 0 件、または受付・拒否のどちらかが 0 件なら失敗にする。
//! 失敗の文には添字・欄の名前だけを出し、値は出さない。変異は `GUIDE3-5`。

mod common;

use std::cell::RefCell;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{guided_op_specs_with_reload, op_specs_with_reload, Harness, OpSpec};
use common::property::check;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, SessionState};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

const CASES: u32 = 1_000;

fn level_of(n: u8) -> GuidanceLevel {
    GuidanceLevel::ALL[usize::from(n) % GuidanceLevel::ALL.len()]
}

fn plain() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 水準で読む定義（`do_levels` と `levels` つきのヒント）を足した教材。
fn level_dependent() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    for node in v["nodes"].as_array_mut().expect("nodes") {
        node["step"]["do_levels"] = serde_json::json!({"B": "B の do", "C": "C の do"});
    }
    v["hints"]["h-1-1"]["levels"] = serde_json::json!(["A"]);
    v["hints"]["h-1-2"]["levels"] = serde_json::json!(["B", "C"]);
    v["hints"]["h-1-3"]["levels"] = serde_json::json!(["C"]);
    let ir = load_ir(&serde_json::to_vec(&v).expect("ser")).expect("load ir");
    (ir, sample.initial)
}

fn mask_run(run: &mut RunState) {
    run.guidance_level = GuidanceLevel::A;
}

/// 水準の欄だけを A に揃えた写し。
fn mask_level(state: &SessionState) -> SessionState {
    let mut s = state.clone();
    mask_run(&mut s.run);
    if let Some(e) = s.entry.as_mut() {
        mask_run(&mut e.run);
    }
    for cp in s.checkpoints.values_mut() {
        mask_run(&mut cp.run);
        if let Some(e) = cp.entry.as_mut() {
            mask_run(&mut e.run);
        }
    }
    s
}

fn set(level: GuidanceLevel) -> Op {
    Op::Control(Control::SetGuidance(level))
}

#[derive(Default)]
struct Stats {
    cases_with_judged: u64,
    judged: u64,
    accepted: u64,
    rejected: u64,
}

/// 水準を切り替え、イベント 0 件・水準の反映・水準以外の不変を確かめる。
fn switch(var: &mut Harness, level: GuidanceLevel, at: usize) -> Result<(), TestCaseError> {
    let before = var.state().clone();
    let applied = var.step_op(set(level));
    if !applied.events.is_empty() {
        return Err(TestCaseError::fail(format!(
            "set_guidance emitted events at {at}"
        )));
    }
    if var.state().run.guidance_level != level {
        return Err(TestCaseError::fail(format!("level not set at {at}")));
    }
    if mask_level(var.state()) != mask_level(&before) {
        return Err(TestCaseError::fail(format!(
            "set_guidance changed a field other than the level at {at}"
        )));
    }
    Ok(())
}

/// 1 列を基準と変種で流して比べる。`inserts` は (位置, 水準) の列で、位置の操作の前に切り替える。
fn compare(
    ir: &Ir,
    initial: &SessionState,
    specs: &[OpSpec],
    first: Option<GuidanceLevel>,
    inserts: &[(usize, u8)],
    stats: &RefCell<Stats>,
) -> Result<(), TestCaseError> {
    let mut base = Harness::pure(ir, initial.clone());
    let mut var = Harness::pure(ir, initial.clone());
    let (mut judged, mut accepted, mut rejected) = (0u64, 0u64, 0u64);
    if let Some(level) = first {
        switch(&mut var, level, 0)?;
    }
    for (i, spec) in specs.iter().enumerate() {
        for (_, n) in inserts.iter().filter(|(p, _)| *p == i) {
            switch(&mut var, level_of(*n), i)?;
        }
        let b = base.step(spec);
        let v = var.step(spec);
        if b.events != v.events {
            return Err(TestCaseError::fail(format!("events differ at {i}")));
        }
        if mask_level(base.state()) != mask_level(var.state()) {
            return Err(TestCaseError::fail(format!(
                "state differs (other than the level) at {i}"
            )));
        }
        for e in &b.events {
            if let Event::Judged { judgment, .. } = e {
                judged += 1;
                match judgment {
                    Judgment::Accepted(_) => accepted += 1,
                    Judgment::Rejected(_) => rejected += 1,
                    _ => {}
                }
            }
        }
    }
    let mut s = stats.borrow_mut();
    s.judged += judged;
    s.accepted += accepted;
    s.rejected += rejected;
    s.cases_with_judged += u64::from(judged > 0);
    Ok(())
}

fn report_and_assert(name: &str, stats: &RefCell<Stats>) {
    let s = stats.borrow();
    println!(
        "GUIDANCE-INVARIANCE {name} cases_with_judged={} judged={} accepted={} rejected={}",
        s.cases_with_judged, s.judged, s.accepted, s.rejected
    );
    assert!(s.cases_with_judged > 0, "no case reached a judgment");
    assert!(s.accepted > 0 && s.rejected > 0, "coverage floor not met");
}

fn run_first<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let (ir, initial) = plain();
    let stats = RefCell::new(Stats::default());
    check(name, CASES, strategy, |specs| {
        for l in GuidanceLevel::ALL {
            compare(&ir, &initial, &specs, Some(l), &[], &stats)?;
        }
        Ok(())
    });
    report_and_assert(name, &stats);
}

#[test]
fn judgments_do_not_depend_on_guidance_level() {
    run_first("guidance-first-plain", op_specs_with_reload());
    run_first("guidance-first-guided", guided_op_specs_with_reload());
}

#[test]
fn switching_guidance_mid_sequence_keeps_judgments() {
    let (ir, initial) = plain();
    let stats = RefCell::new(Stats::default());
    let strategy = (
        guided_op_specs_with_reload(),
        proptest::collection::vec((0usize..50, 0u8..3), 0..6),
    );
    check("guidance-mid", CASES, strategy, |(specs, inserts)| {
        compare(&ir, &initial, &specs, None, &inserts, &stats)
    });
    report_and_assert("guidance-mid", &stats);
}

#[test]
fn invariance_with_level_dependent_ir() {
    let (ir, initial) = level_dependent();
    let stats = RefCell::new(Stats::default());
    let strategy = (
        guided_op_specs_with_reload(),
        0u8..3,
        proptest::collection::vec((0usize..50, 0u8..3), 0..6),
    );
    check(
        "guidance-dependent-ir",
        CASES,
        strategy,
        |(specs, f, inserts)| compare(&ir, &initial, &specs, Some(level_of(f)), &inserts, &stats),
    );
    report_and_assert("guidance-dependent-ir", &stats);
}
