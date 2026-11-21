//! 周回ごとの案内レベルの割り当てを、教材の宣言どおりに切り替えることの受け入れ試験（TASK-299。GUIDE-9・D-55。記録は `docs/testing.md` の 5.533）。
//!
//! エンジンが「どの周回にどの水準を使うか」「周回の数」を固定していないことを、割り当ての違う 4 本の教材パックで確かめる。
//! 教材は `module-reuse-rounds` の写しで、`pack.guidance`（1 周目。開始点 `s-r1` は宣言を持たない）と、2 周目・3 周目の開始点の
//! `guidance` だけを書き換える。3 周目は `round_three.rs` と同じく `step5/ch-round3.chapter.yaml` を足して作る。
//! 各周回の開始（`Control::Begin`）の直後と、その周回を終えた時点で、実行全体の水準（`run.guidance_level`）が宣言どおりであること、
//! 次の周回の開始までは水準が前の周回のまま変わらないことを見る。水準の意味（A・B・C で何を示すか）は教材側の約束で、ここでは見ない。
//! 形式 B の教材のコンパイルは pack にしかないため、core ではなく pack に置く。呼び出し元は CI の `make test` だけである。
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use std::fs;

use common::{copy_dir, testdata, Tmp};
use fandhe_workshop_contract::guidance::GuidanceLevel::{self, A, B, C};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{
    begin_round2_plan, begin_round3_plan, round1_plan, round2_plan, round3_plan, run_plan,
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

/// 周回を順に通し、各周回の開始の直後と終わりの水準を返す（`[(開始の直後, 終わり)]`）。
fn levels_through_rounds(ir: &Ir, rounds: usize) -> Vec<(GuidanceLevel, GuidanceLevel)> {
    let mut s = Session::new(ir.clone());
    let mut out = Vec::new();
    let level = |s: &Session| s.state().run.guidance_level;
    let r1 = round1_plan(ir);
    run_plan(&mut s, &r1[..1]);
    let begun = level(&s);
    run_plan(&mut s, &r1[1..]);
    out.push((begun, level(&s)));
    run_plan(&mut s, &begin_round2_plan());
    let begun = level(&s);
    run_plan(&mut s, &round2_plan(ir));
    out.push((begun, level(&s)));
    if rounds == 3 {
        run_plan(&mut s, &begin_round3_plan());
        let begun = level(&s);
        run_plan(&mut s, &round3_plan(ir));
        out.push((begun, level(&s)));
    }
    out
}

/// 宣言どおりに切り替わり、周回の途中では変わらないこと。
fn assert_follows(levels: &[GuidanceLevel]) {
    let (_t, ir) = pack_with(levels);
    let got = levels_through_rounds(&ir, levels.len());
    let want: Vec<_> = levels.iter().map(|l| (*l, *l)).collect();
    assert_eq!(got, want, "割り当て {levels:?}");
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
