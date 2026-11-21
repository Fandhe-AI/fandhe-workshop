//! 周回ごとの案内レベルの割り当てを、教材の宣言どおりに切り替えることの受け入れ試験（TASK-299。GUIDE-9・D-55。記録は `docs/testing.md` の 5.533）。
//!
//! エンジンが「どの周回にどの水準を使うか」を固定せず、2 周と 3 周の教材のどちらも宣言どおりに扱うことを、割り当ての違う 4 本の教材パックで確かめる。
//! 4 周目以降は扱わない（3 周で打ち切る回帰はこの試験では検出しない）。
//! 教材は `module-reuse-rounds` の写しで、`pack.guidance`（1 周目。開始点 `s-r1` は宣言を持たない）と、2 周目・3 周目の開始点の
//! `guidance` だけを書き換える。3 周目は `round_three.rs` と同じく `step5/ch-round3.chapter.yaml` を足して作る。
//! 各周回の開始（`Control::Begin`）が成立し、その直後から周回の計画の 1 手ごとに実行全体の水準（`run.guidance_level`）が宣言どおりで、
//! 周回の終わりに呼び出しの Step が達成していることを見る。水準の意味（A・B・C で何を示すか）は教材側の約束で、ここでは見ない。
//! 形式 B の教材のコンパイルは pack にしかないため、core ではなく pack に置く。呼び出し元は CI の `make test` だけである。
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use std::fs;

use common::{copy_dir, testdata, Tmp};
use fandhe_workshop_contract::guidance::GuidanceLevel::{self, A, B, C};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{
    begin_round2_plan, begin_round3_plan, round1_plan, round2_plan, round3_plan, run_plan, PlanStep,
};

const PACK: &str = "module-reuse-rounds/test-data/pack";
const STEP5: &str = "module-reuse-rounds/test-data/step5/ch-round3.chapter.yaml";
const ROUND2: &str = "chapters/ch-round2/chapter.yaml";
const ROUND3: &str = "chapters/ch-round3/chapter.yaml";

fn letter(l: GuidanceLevel) -> &'static str {
    match l {
        A => "A",
        B => "B",
        C => "C",
    }
}

/// `text` の中の `from` をちょうど `n` 回置き換える（写しの元の形が変わったら気づけるよう、回数を確かめる）。
fn replace_n(text: &str, from: &str, to: &str, n: usize) -> String {
    assert_eq!(text.matches(from).count(), n, "{from:?} の数が想定と違う");
    text.replace(from, to)
}

/// 割り当て `levels`（1 周目から順。長さ 2 か 3）を宣言した写しの IR。`testdata` は読むだけで書き換えない。
fn pack_with(levels: &[GuidanceLevel]) -> (Tmp, Ir) {
    assert!(matches!(levels.len(), 2 | 3));
    let t = Tmp::new("round-guidance");
    copy_dir(&testdata(PACK), &t.0);
    let workshop = fs::read_to_string(t.0.join("workshop.yaml")).expect("workshop.yaml");
    let workshop = replace_n(
        &workshop,
        "\n  guidance: A\n",
        &format!("\n  guidance: {}\n", letter(levels[0])),
        1,
    );
    t.put("workshop.yaml", workshop.as_bytes());
    let round2 = fs::read_to_string(t.0.join(ROUND2)).expect("ch-round2");
    // `s-r2` と `s-r2-fresh` の 2 つ。
    let round2 = replace_n(
        &round2,
        "    guidance: B\n",
        &format!("    guidance: {}\n", letter(levels[1])),
        2,
    );
    t.put(ROUND2, round2.as_bytes());
    if let Some(l3) = levels.get(2) {
        let round3 = fs::read_to_string(testdata(STEP5)).expect("step5");
        let round3 = replace_n(
            &round3,
            "    guidance: B\n",
            &format!("    guidance: {}\n", letter(*l3)),
            1,
        );
        t.put(ROUND3, round3.as_bytes());
    }
    let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
    (t, ir)
}

/// 周回を順に通し、宣言どおりの水準で進むことを確かめる。各周回で、(1) `Begin` がちょうど 1 件の `Begun` を出し拒否が無い、
/// (2) 開始の直後から周回の計画の 1 手ごとに水準が宣言値のまま、(3) 周回の終わりにその周回の 2 つの呼び出しの Step が達成、を見る。
fn assert_follows(levels: &[GuidanceLevel]) {
    let (_t, ir) = pack_with(levels);
    let mut s = Session::new(ir.clone());
    let r1 = round1_plan(&ir);
    let rounds: Vec<(Vec<PlanStep>, Vec<PlanStep>)> = [
        Some((r1[..1].to_vec(), r1[1..].to_vec())),
        Some((begin_round2_plan(), round2_plan(&ir))),
        (levels.len() == 3).then(|| (begin_round3_plan(), round3_plan(&ir))),
    ]
    .into_iter()
    .flatten()
    .collect();
    assert_eq!(rounds.len(), levels.len());
    for (n, ((begin, body), want)) in rounds.iter().zip(levels).enumerate() {
        let round = n + 1;
        let ev = run_plan(&mut s, begin);
        let begun = ev
            .iter()
            .filter(|e| matches!(e, Event::Begun { .. }))
            .count();
        assert_eq!(begun, 1, "{round} 周目の開始: {ev:?}");
        assert!(
            !ev.iter().any(|e| matches!(e, Event::Refused { .. })),
            "{round} 周目の開始: {ev:?}"
        );
        assert_eq!(
            s.state().run.guidance_level,
            *want,
            "{round} 周目の開始の直後（割り当て {levels:?}）"
        );
        for (k, step) in body.iter().enumerate() {
            run_plan(&mut s, std::slice::from_ref(step));
            assert_eq!(
                s.state().run.guidance_level,
                *want,
                "{round} 周目の {k} 手目の後（割り当て {levels:?}）"
            );
        }
        for id in ["idea", "impl"] {
            let uid = StepUid(format!("main/r{round}-{id}"));
            assert_eq!(
                s.step_status(&uid),
                StepStatus::Achieved,
                "{round} 周目の終わりの {uid:?}"
            );
        }
    }
}

#[test]
fn guide9_two_rounds_a_then_b_as_declared() {
    // 最初の教材の割り当て（D-55）。
    assert_follows(&[A, B]);
}

#[test]
fn guide9_two_rounds_all_a_as_declared() {
    assert_follows(&[A, A]);
}

#[test]
fn guide9_three_rounds_a_b_c_as_declared() {
    assert_follows(&[A, B, C]);
}

#[test]
fn guide9_three_rounds_not_in_order_as_declared() {
    // 水準を周回の順に上げる・下げる規則をエンジンが持たないこと。
    assert_follows(&[C, A, B]);
}

#[test]
fn guide9_first_round_begin_is_the_first_plan_step() {
    // 開始の直後の水準を 1 手目の後で測る前提（`round1_plan` の先頭が `Begin("s-r1")`）。
    let (_t, ir) = pack_with(&[B, A]);
    let mut s = Session::new(ir.clone());
    assert_eq!(s.state().run.guidance_level, A, "開始前は既定の A");
    run_plan(&mut s, &round1_plan(&ir)[..1]);
    assert_eq!(
        s.state().run.guidance_level,
        B,
        "1 手目の Begin で pack.guidance に切り替わる"
    );
}
