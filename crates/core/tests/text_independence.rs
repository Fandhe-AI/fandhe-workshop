//! 教材の表示文字列を差し替えても、状態・イベント列・導出が変わらないことの性質テスト（TASK-218 2/3。EVT-2・P-7）。
//!
//! 教材の表示文字列 31 個（題・選択肢の表示名・ヒント・拒否の文面と次の行動）を差し替え用の表 A・B に置き換えた
//! 3 つの IR（元・A・B）に同じ入力列を流し、各操作後の記録が一致することを確かめる。あわせて、差し替えが
//! 空振りでないこと（右のログの文字列・ヒント・選択肢・復習の題だけが変わること）を確かめる。
//!
//! `Event::Judged` は差し込み後の次の行動（表示文字列）を保持する。spec の「イベント列の差 0」は `Judged` の導入
//! （TASK-267）前の記述なので、次の行動を文面の ID に写してから比べる（`common/text_table.rs` の `mask_display`）。
//! それ以外の欄は写さずにそのまま比べ、表示文字列が他の経路で状態・イベント・導出に入り込めば不一致になる。
//! 同じ入力列の 2 回適用は `determinism.rs`（1/3）、ID の変更の負の対照は 3/3 の持ち分で、本ファイルは扱わない。
//! 経路の一致は TASK-219 の持ち分のため、1,000 通りの性質テストは純粋関数の経路だけを流す。
//! 成功条件は不変条件だけで、統計は出力するだけで assert しない（`docs/testing.md` 第 2 節）。

mod common;

use common::op_gen::{guided_op_specs, op_specs, script_specs, Harness, OpSpec, Which};
use common::property::check;
use common::text_table::{
    blank_display, judged_next_actions, leaked_string, load_table, load_variant, mask_display,
    original_value, substitute, variant_strings, variant_value, Strings, Variant,
};
use common::trace::{compare_traces, trace, StepTrace};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::select_judge::choice_options;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::text_fill::filled_texts_of_log;
use proptest::prelude::*;
use std::cell::RefCell;

const VARIANTS: [Variant; 3] = [Variant::Original, Variant::A, Variant::B];

fn load_all() -> Vec<(Ir, SessionState)> {
    VARIANTS.iter().map(|v| load_variant(*v)).collect()
}

fn run_pure(irs: &[(Ir, SessionState)], specs: &[OpSpec]) -> Vec<Vec<StepTrace>> {
    irs.iter()
        .map(|(ir, init)| trace(Harness::pure(ir, init.clone()), specs))
        .collect()
}

fn run_session(irs: &[(Ir, SessionState)], specs: &[OpSpec]) -> Vec<Vec<StepTrace>> {
    irs.iter()
        .map(|(ir, init)| trace(Harness::via_session(ir, init.clone()), specs))
        .collect()
}

/// 元と A・B の写した記録が一致し、A・B の表示文字列が記録に入り込んでいないことを確かめる。
fn masked_match(runs: &[Vec<StepTrace>]) -> Result<(), String> {
    let base = mask_display(&runs[0]);
    for (i, v) in [(1, Variant::A), (2, Variant::B)] {
        let masked = mask_display(&runs[i]);
        compare_traces(&base, &masked).map_err(|e| format!("original vs {v:?}: {e}"))?;
        let strings = variant_strings(v);
        if let Some(s) = leaked_string(&masked, strings.values()) {
            return Err(format!("{v:?}: display string leaked into trace: {s}"));
        }
    }
    Ok(())
}

#[test]
fn tables_cover_all_display_strings_and_are_disjoint() {
    let original = original_value();
    let strings: Vec<Strings> = VARIANTS.iter().map(|v| variant_strings(*v)).collect();
    for s in &strings {
        assert_eq!(s.len(), 31);
        assert_eq!(
            s.keys().collect::<Vec<_>>(),
            strings[0].keys().collect::<Vec<_>>()
        );
    }
    for (i, a) in strings.iter().enumerate() {
        for b in strings.iter().skip(i + 1) {
            for (k, va) in a {
                assert_ne!(va, &b[k], "{k} is not replaced");
                assert!(!b.values().any(|x| x == va), "value of {k} is shared");
            }
        }
    }
    // 構造（ID・分類・規則）は表示文字列を空にすれば 3 つで同じになる。
    let blank = blank_display(&original);
    for v in [Variant::A, Variant::B] {
        assert_eq!(blank_display(&variant_value(v)), blank);
    }
    // 差し替えた IR に元の文字列が 1 つも残らない。
    for v in [Variant::A, Variant::B] {
        let text = serde_json::to_string(&variant_value(v)).expect("serialize");
        for (k, s) in &strings[0] {
            assert!(!text.contains(s.as_str()), "{v:?} still has original {k}");
        }
    }
}

#[test]
fn substitute_fails_closed_on_key_mismatch() {
    let original = original_value();
    let mut table = load_table("A");
    table.remove("pack_title");
    let r = std::panic::catch_unwind(|| substitute(&original, &table));
    assert!(r.is_err(), "a table with a missing key must be refused");
    let mut table = load_table("A");
    table.insert("hints/h-9-9".to_string(), "x".to_string());
    let r = std::panic::catch_unwind(|| substitute(&original, &table));
    assert!(r.is_err(), "a table with an extra key must be refused");
}

#[test]
fn scripted_ops_match_across_tables() {
    let irs = load_all();
    let specs = script_specs();
    assert_eq!(specs.len(), 37);
    masked_match(&run_pure(&irs, &specs)).expect("pure route");
    masked_match(&run_session(&irs, &specs)).expect("session route");
}

#[test]
fn scripted_ops_change_only_display_strings() {
    let irs = load_all();
    let runs = run_pure(&irs, &script_specs());
    // 写す前は次の行動が変わるので食い違う（差し替えが空振りでない）。
    for i in [1, 2] {
        assert!(compare_traces(&runs[0], &runs[i]).is_err());
    }
    let base_ids: Vec<(usize, String)> = judged_next_actions(&runs[0])
        .into_iter()
        .map(|(i, t, _)| (i, t))
        .collect();
    assert!(!base_ids.is_empty(), "the script produces rejections");
    let mut seen_next = Vec::new();
    for (idx, v) in VARIANTS.iter().enumerate() {
        let strings = variant_strings(*v);
        let (ir, _) = &irs[idx];
        let judged = judged_next_actions(&runs[idx]);
        let ids: Vec<(usize, String)> = judged.iter().map(|(i, t, _)| (*i, t.clone())).collect();
        assert_eq!(ids, base_ids, "{v:?}: judged ids differ");
        for (_, id, next) in &judged {
            assert_eq!(next, &strings[&format!("rejections/{id}/next_action")]);
        }
        seen_next.push(judged.into_iter().map(|j| j.2).collect::<Vec<_>>());

        let last = &runs[idx].last().expect("trace").after;
        let filled = filled_texts_of_log(ir, &last.log);
        assert!(!filled.is_empty(), "{v:?}: filled texts exist");
        for (_, f) in &filled {
            assert_eq!(
                f.message,
                strings[&format!("rejections/{}/message", f.text)]
            );
            assert_eq!(
                f.next_action.as_str(),
                strings[&format!("rejections/{}/next_action", f.text)]
            );
        }
        // 復習の項目の Step の題、ヒントの本文、選択肢の表示名。
        let mut reviews = 0;
        let mut hints = 0;
        for t in &runs[idx] {
            for item in &t.review {
                let node = ir
                    .nodes
                    .iter()
                    .find(|n| n.uid == item.step.0)
                    .expect("node");
                assert_eq!(
                    node.step.title,
                    strings[&format!("step_titles/{}", item.step.0)]
                );
                reviews += 1;
            }
            for id in current_hints(ir, &t.after.run) {
                let text = ir.hints[id]["text"].as_str().expect("hint text");
                assert_eq!(text, strings[&format!("hints/{id}")]);
                hints += 1;
            }
        }
        assert!(
            reviews > 0 && hints > 0,
            "{v:?}: reviews={reviews} hints={hints}"
        );
        for node in &ir.nodes {
            for o in choice_options(&node.step).expect("options") {
                assert_eq!(o.label, strings[&format!("option_labels/{}", o.id)]);
            }
        }
        assert_eq!(ir.pack.title, strings["pack_title"]);
    }
    assert_ne!(seen_next[0], seen_next[1]);
    assert_ne!(seen_next[0], seen_next[2]);
    assert_ne!(seen_next[1], seen_next[2]);
}

#[derive(Default, Debug)]
struct Stats {
    ops: u64,
    judged: u64,
}

fn run_property<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let irs = load_all();
    let stats = RefCell::new(Stats::default());
    check(name, 1_000, strategy, |specs| {
        let runs = run_pure(&irs, &specs);
        if let Err(message) = masked_match(&runs) {
            prop_assert!(false, "{message}");
        }
        let mut s = stats.borrow_mut();
        s.ops += specs.len() as u64;
        s.judged += judged_next_actions(&runs[0]).len() as u64;
        Ok(())
    });
    println!("PROPERTY-TEST stats={:?}", stats.borrow());
}

#[test]
fn display_swap_matches_with_fixed_sequences() {
    run_property("text-independence-fixed", op_specs());
}

#[test]
fn display_swap_matches_with_guided_sequences() {
    run_property("text-independence-guided", guided_op_specs());
}

#[test]
fn comparator_still_detects_differences_after_masking() {
    let (ir, init) = load_variant(Variant::Original);
    let full = [OpSpec::Cmd(5), OpSpec::Tick(Which::Current), OpSpec::Save];
    let x = mask_display(&trace(Harness::pure(&ir, init.clone()), &full));
    // 操作を 1 件落とした列。
    let dropped = mask_display(&trace(Harness::pure(&ir, init.clone()), &full[..2]));
    assert!(compare_traces(&x, &dropped).is_err());
    // 判定の文面の ID だけ、あるいは分類だけを書き換えた記録。
    let mut id_changed = x.clone();
    let mut category_changed = x.clone();
    let mut touched = false;
    for t in 0..x.len() {
        for e in 0..x[t].events.len() {
            if let Event::Judged {
                judgment: Judgment::Rejected(r),
                ..
            } = &mut id_changed[t].events[e]
            {
                r.text.push('x');
                touched = true;
            }
            if let Event::Judged {
                judgment: Judgment::Rejected(r),
                ..
            } = &mut category_changed[t].events[e]
            {
                r.category = if r.category == RejectCategory::Busy {
                    RejectCategory::WrongOrder
                } else {
                    RejectCategory::Busy
                };
            }
        }
    }
    assert!(touched, "the sequence produces a rejection");
    assert!(compare_traces(&x, &id_changed).is_err());
    assert!(compare_traces(&x, &category_changed).is_err());
}
