//! 確認を省く `wait` の Step の輪を `validate` が断る試験（#2209。PR #2207 のレビューから）。
//!
//! `input: { kind: wait }`・`processing`・`confirm: none` の Step は、入ると処理中になり、最後の進行で受付・達成になって
//! 最初の無条件の `next` へ移る。そのような Step だけで戻る輪は、受講者の入力なしに進行が続いて止まらないため、
//! 輪に入る Step ごとに `E-STRUCTURE` を 1 件出す。輪の一部が入力を待つ Step なら出さない（対照）。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::{Tmp, WORKSHOP};
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;

const CYCLE: &str = "wait steps with confirm none form a cycle without learner input";

fn wait_step(id: &str, next: &str, confirm: &str) -> String {
    format!(
        "  - {{id: {id}, title: {id}, next: [{{to: {next}}}], input: {{kind: wait}}, processing: [{{name: p-{id}, effects: []}}], confirm: {confirm}}}\n"
    )
}

/// 開始点 `s1` が `w1` を指す章。`steps` は Step の行。
fn pack(steps: &str) -> Tmp {
    let t = Tmp::new("wait-cycle");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: w1}}]\nsteps:\n{steps}").as_bytes(),
    );
    t
}

/// 輪の診断の項目（`step:<id>`）の一覧。
fn cycle_items(t: &Tmp) -> Vec<String> {
    validate_dir(&t.0)
        .iter()
        .filter(|d| d.code() == DiagCode::Structure && d.to_string().contains(CYCLE))
        .map(|d| {
            let s = d.to_string();
            ["w1", "w2"]
                .into_iter()
                .find(|id| s.contains(&format!("step:{id}")))
                .unwrap_or("?")
                .to_owned()
        })
        .collect()
}

const NONE: &str = "{kind: none, reason: r}";

#[test]
fn two_wait_steps_with_confirm_none_in_a_cycle_are_refused() {
    let t = pack(&format!(
        "{}{}",
        wait_step("w1", "w2", NONE),
        wait_step("w2", "w1", NONE)
    ));
    assert_eq!(cycle_items(&t), vec!["w1".to_owned(), "w2".to_owned()]);
}

#[test]
fn a_wait_step_with_confirm_none_that_returns_to_itself_is_refused() {
    let t = pack(&wait_step("w1", "w1", NONE));
    assert_eq!(cycle_items(&t), vec!["w1".to_owned()]);
}

#[test]
fn a_cycle_through_a_step_that_waits_for_input_is_not_refused() {
    // w2 は承認を待つ（受講者の入力で止まる）ので、輪でも止まる。
    let t = pack(&format!(
        "{}{}",
        wait_step("w1", "w2", NONE),
        wait_step("w2", "w1", "{kind: approve}")
    ));
    assert!(cycle_items(&t).is_empty());
}

#[test]
fn a_cycle_that_appears_after_module_argument_substitution_is_refused() {
    // モジュールの確認の種類が引数で決まる。`none` を渡した実体だけが、輪になる。
    let module = |c: &str| {
        let t = Tmp::new("wait-cycle-module");
        t.put("workshop.yaml", WORKSHOP.as_bytes());
        t.put(
            "chapters/c1/chapter.yaml",
            format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U, use: {{module: m, instance: i1, args: {{c: {c}}}}}, next: [{{to: z}}]}}\n  - {{id: z, title: Z, final: true}}\n").as_bytes(),
        );
        t.put(
            "modules/m/module.yaml",
            b"id: m\ntitle: M\nparams: {c: approve}\nsteps:\n  - {id: w1, title: W1, next: [{to: w2}], input: {kind: wait}, processing: [{name: p1, effects: []}], confirm: {kind: '${c}', reason: r}}\n  - {id: w2, title: W2, next: [{to: w1}], input: {kind: wait}, processing: [{name: p2, effects: []}], confirm: {kind: '${c}', reason: r}}\n",
        );
        t
    };
    assert_eq!(
        cycle_items(&module("none")),
        vec!["w1".to_owned(), "w2".to_owned()]
    );
    assert!(
        cycle_items(&module("approve")).is_empty(),
        "承認を待つ実体は輪でも止まる"
    );
}

#[test]
fn a_long_chain_without_a_cycle_is_not_refused() {
    // 確認を省く wait の Step が 2,000 段続いて終わる（輪ではない）。たどり済みの印で 1 回ずつ見る。
    let mut steps = String::new();
    for i in 1..=2000 {
        steps.push_str(&wait_step(&format!("w{i}"), &format!("w{}", i + 1), NONE));
    }
    steps.push_str("  - {id: w2001, title: end, final: true}\n");
    let t = pack(&steps);
    let started = std::time::Instant::now();
    assert!(cycle_items(&t).is_empty());
    assert!(
        started.elapsed() < std::time::Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn the_cycle_in_a_module_is_reported_in_the_module_file_even_if_a_chapter_has_the_same_id() {
    // 章とモジュールが同じ ID（c1）でも、モジュールの輪はモジュールのファイルに出す。
    let t = Tmp::new("wait-cycle-same-id");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U, use: {module: c1, instance: i1}, next: [{to: z}]}\n  - {id: z, title: Z, final: true}\n",
    );
    t.put(
        "modules/c1/module.yaml",
        b"id: c1\ntitle: M\nsteps:\n  - {id: w1, title: W1, next: [{to: w1}], input: {kind: wait}, processing: [{name: p1, effects: []}], confirm: {kind: none, reason: r}}\n",
    );
    let hits: Vec<String> = validate_dir(&t.0)
        .iter()
        .filter(|d| d.to_string().contains(CYCLE))
        .map(ToString::to_string)
        .collect();
    assert_eq!(hits.len(), 1, "{hits:?}");
    assert!(hits[0].contains("modules/c1/module.yaml"), "{hits:?}");
}
