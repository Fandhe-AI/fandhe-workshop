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
