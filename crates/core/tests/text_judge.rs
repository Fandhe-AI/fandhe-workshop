//! 自然文の受付の判定と、受付の適用の試験（TASK-266、JUDGE-2）。
//!
//! 宣言の例は、試作 bbs-tutorial（format-b）の `st-1-3` と、ch1-pack の golden の `st-1-3` の写しである
//! （出所はそれぞれ spec repo の `prototype/packs/bbs-tutorial/format-b/chapters/ch01-practice/chapter.yaml` と
//! `crates/pack/tests/data/golden/ch1-pack.ir.json`）。`docs/spec/` は private のため、試験からは直接読まない。

mod common;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::input_spec::TextVerdict;
use fandhe_workshop_contract::ir::Step;
use fandhe_workshop_contract::judgment::{Category, JudgmentKind, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::effect::{ConditionEval, NoConditions};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::text_judge::{
    accept_text, judge_text, text_spec, TextDeclError, TextJudgment,
};
use proptest::prelude::*;
use serde_json::{json, Value};

struct AlwaysTrue;

impl ConditionEval for AlwaysTrue {
    fn eval(&self, _when: &Value, _run: &RunState, _root: &Root) -> Option<bool> {
        Some(true)
    }
}

fn step_of(v: Value) -> Step {
    serde_json::from_value(v).expect("test step must be valid")
}

fn text_step(min: u32, max: u32, rules: Value) -> Step {
    step_of(json!({
        "id": "st-1-3", "title": "memo",
        "input": { "kind": "text", "min": min, "max": max },
        "rules": rules,
    }))
}

/// 試作 `st-1-3` の写し。`put` の属性は固定値で、受講者の文は入らない（D-50）。
fn memo_rules() -> Value {
    json!([{
        "id": "r-memo",
        "on": { "text": true },
        "outcome": {
            "kind": "accept",
            "effects": [{ "put": { "path": "memo/note-1", "fixture": "fx-card", "attrs": { "state": "shown" } } }]
        }
    }])
}

fn uid() -> StepUid {
    StepUid("main/st-1-3".to_string())
}

fn stamp() -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: uid(),
    }
}

fn judge(step: &Step, text: &str) -> Result<TextJudgment, TextDeclError> {
    judge_text(
        step,
        text,
        &RunState::default(),
        &Root("main".into()),
        &NoConditions,
    )
}

fn accepted(step: &Step, text: &str) -> fandhe_workshop_core::text_judge::AcceptedText {
    match judge(step, text) {
        Ok(TextJudgment::Accepted(a)) => a,
        other => panic!("expected accepted, got {other:?}"),
    }
}

#[test]
fn boundaries() {
    let s = text_step(3, 2000, json!([]));
    for t in ["", "   ", "\n\t ", "\u{3000}\u{3000}"] {
        assert_eq!(
            judge(&s, t),
            Ok(TextJudgment::Rejected(TextVerdict::Empty)),
            "{t:?}"
        );
    }
    assert_eq!(
        judge(&s, "ab"),
        Ok(TextJudgment::Rejected(TextVerdict::TooShort))
    );
    assert!(matches!(judge(&s, "abc"), Ok(TextJudgment::Accepted(_))));
    assert!(matches!(
        judge(&s, &"あ".repeat(2000)),
        Ok(TextJudgment::Accepted(_))
    ));
    let over = judge(&s, &"あ".repeat(2001)).expect("declaration is valid");
    assert_eq!(over, TextJudgment::Rejected(TextVerdict::TooLong));
    assert_eq!(over.category(), Category::BadArgShape);
    assert_eq!(over.reject_category(), Some(RejectCategory::BadArgShape));
    assert_eq!(over.kind(), JudgmentKind::Rejected);
}

#[test]
fn accepted_is_free_text_unjudged() {
    let s = text_step(1, 10, json!([]));
    let j = judge(&s, "hello").expect("valid");
    assert_eq!(j.kind(), JudgmentKind::Accepted);
    assert_eq!(j.category(), Category::FreeTextUnjudged);
    assert!(!j.category().is_rejection());
    assert_eq!(j.reject_category(), None);
}

#[test]
fn content_is_not_judged() {
    let s = text_step(5, 5, memo_rules());
    let base = accepted(&s, "abcde");
    for t in [
        "日本語です。",
        "<scr>",
        "/help",
        "!?@#$",
        "a😀b😀c",
        "\n\n\n\n\n",
    ] {
        // 前後の空白だけの文字列は空として拒否されるため、内容のある 5 文字だけを比べる。
        if t.trim().chars().count() != 5 {
            continue;
        }
        let a = accepted(&s, t);
        assert_eq!(a.rule_id, base.rule_id, "{t:?}");
        assert_eq!(a.effects, base.effects, "{t:?}");
    }
    let a = accepted(&s, "a😀b😀c");
    assert_eq!(a.rule_id.as_deref(), Some("r-memo"));
}

#[test]
fn pack41_rule_choice_ignores_words() {
    // 判定語は無い（PACK-41、D-43）。語を含む文と含まない文で、選ばれる規則も分類も変わらない。
    let rules = json!([
        { "id": "r-first", "on": { "text": true }, "outcome": { "kind": "accept", "effects": [] } },
        { "id": "r-second", "on": { "text": true }, "outcome": { "kind": "accept", "effects": [] } },
    ]);
    let s = text_step(1, 40, rules);
    for t in ["contains foo", "no such word", "foo"] {
        let a = accepted(&s, t);
        assert_eq!(a.rule_id.as_deref(), Some("r-first"), "{t:?}");
        let j = judge(&s, t).expect("valid");
        assert_eq!(j.category(), Category::FreeTextUnjudged, "{t:?}");
    }
}

#[test]
fn text_is_kept_verbatim() {
    let s = text_step(1, 100, memo_rules());
    for t in [
        "一行目\n二行目",
        "  前後に空白  ",
        "\n改行で始まる\n",
        "\t tab\r\n",
    ] {
        assert_eq!(accepted(&s, t).text, t);
    }
}

#[test]
fn rule_selection() {
    let rules = json!([
        { "id": "r-false", "on": { "text": false }, "outcome": { "kind": "accept", "effects": [] } },
        { "id": "r-cmd", "on": { "command": "x" }, "outcome": { "kind": "accept", "effects": [] } },
        { "id": "r-reject", "on": { "text": true }, "outcome": { "kind": "reject" } },
        { "id": "r-when", "on": { "text": true }, "when": { "flag": "f" }, "outcome": { "kind": "accept", "effects": [{ "set_flag": "w" }] } },
        { "id": "r-hit", "on": { "text": true }, "outcome": { "kind": "accept", "effects": [{ "set_flag": "h" }] } },
        { "id": "r-late", "on": { "text": true }, "outcome": { "kind": "accept", "effects": [] } },
    ]);
    let s = text_step(1, 10, rules);
    let a = accepted(&s, "x");
    assert_eq!(a.rule_id.as_deref(), Some("r-hit"));
    assert_eq!(a.effects.len(), 1);

    // `when` つきの規則は、評価器が真を返すときだけ選ばれる。
    let run = RunState::default();
    let root = Root("main".into());
    let j = judge_text(&s, "x", &run, &root, &AlwaysTrue).expect("valid");
    match j {
        TextJudgment::Accepted(a) => assert_eq!(a.rule_id.as_deref(), Some("r-when")),
        other => panic!("{other:?}"),
    }

    // 該当する規則が無くても受付で、効果は空。
    let none = text_step(
        1,
        10,
        json!([{ "id": "r", "on": { "command": "x" }, "outcome": { "kind": "accept" } }]),
    );
    let a = accepted(&none, "x");
    assert_eq!((a.rule_id, a.effects.len()), (None, 0));
}

#[test]
fn null_when_is_unconditional() {
    // `when: null` は無条件として選ぶ。評価器が条件を解釈できなくても効果は落ちない（judge の規則選択と同じ）。
    let rules = json!([
        { "id": "r-null", "on": { "text": true }, "when": null,
        "outcome": { "kind": "accept", "effects": [{ "set_flag": "n" }] } },
        { "id": "r-late", "on": { "text": true }, "outcome": { "kind": "accept", "effects": [] } },
    ]);
    let s = text_step(1, 10, rules);
    let a = accepted(&s, "x");
    assert_eq!(a.rule_id.as_deref(), Some("r-null"));
    assert_eq!(a.effects.len(), 1);
}

#[test]
fn declaration_errors() {
    let no_input = step_of(json!({ "id": "s", "title": "t" }));
    assert_eq!(judge(&no_input, "x").err(), Some(TextDeclError::NotText));
    let cmd =
        step_of(json!({ "id": "s", "title": "t", "input": { "kind": "command", "name": "help" } }));
    assert_eq!(text_spec(&cmd).err(), Some(TextDeclError::NotText));
    let bad = [
        json!({ "kind": "text", "min": 5, "max": 1 }),
        json!({ "kind": "text", "min": 1, "max": 5, "extra": 1 }),
        json!({ "kind": "text", "min": "1", "max": 5 }),
        json!({ "kind": "text", "min": 1 }),
    ];
    for input in bad {
        let s = step_of(json!({ "id": "s", "title": "t", "input": input }));
        assert_eq!(judge(&s, "x").err(), Some(TextDeclError::InvalidSpec));
    }
    let rules = json!([{ "on": { "text": true }, "outcome": { "kind": "accept",
        "effects": [{ "set_flag": "ok" }, { "unknown_word": 1 }] } }]);
    let s = text_step(1, 5, rules);
    assert_eq!(
        judge(&s, "x").err(),
        Some(TextDeclError::InvalidEffect { item: 1 })
    );
    // 配列でない effects は宣言の誤りで、空の受付にしない。省略は正常。
    for bad_effects in [json!({ "set_flag": "done" }), json!("x"), json!(null)] {
        let rules = json!([{ "on": { "text": true },
            "outcome": { "kind": "accept", "effects": bad_effects } }]);
        let s = text_step(1, 5, rules);
        assert_eq!(judge(&s, "x").err(), Some(TextDeclError::InvalidEffects));
    }
    let omitted = text_step(
        1,
        5,
        json!([{ "on": { "text": true }, "outcome": { "kind": "accept" } }]),
    );
    assert!(matches!(
        judge(&omitted, "x"),
        Ok(TextJudgment::Accepted(_))
    ));
    // 範囲外の入力では規則を読まないため、効果が不正でも拒否を返す。
    assert!(matches!(judge(&s, ""), Ok(TextJudgment::Rejected(_))));
}

fn changes(ev: &[Event]) -> Vec<&Event> {
    ev.iter()
        .filter(|e| matches!(e, Event::Changed { .. }))
        .collect()
}

fn apply_flow(step: &Step, text: &str) -> (RunState, RunState, Vec<Event>) {
    let mut run = RunState {
        current: Some(uid()),
        ..RunState::default()
    };
    run.slices
        .entry(Root("main".into()))
        .or_default()
        .status
        .insert(uid(), StepStatus::Entered);
    let before = run.clone();
    let a = accepted(step, text);
    let ev = accept_text(&mut run, &uid(), &a, &stamp(), &NoConditions);
    (before, run, ev)
}

#[test]
fn accept_reflects_into_world_without_achieving() {
    let s = text_step(3, 2000, memo_rules());
    let (before, run, ev) = apply_flow(&s, "一行目\n二行目");
    assert!(before.world.is_empty());
    assert_eq!(run.world["memo/note-1"].fixture.as_deref(), Some("fx-card"));
    assert_eq!(
        ev.iter()
            .filter(|e| matches!(e, Event::Accepted { .. }))
            .count(),
        1
    );
    let ch = changes(&ev);
    assert_eq!(ch.len(), 1);
    assert!(
        matches!(ch[0], Event::Changed { path, cause: Cause::Input, .. } if path == "memo/note-1")
    );
    assert_eq!(
        run.slices[&Root("main".into())].status[&uid()],
        StepStatus::Accepted
    );
    assert!(!ev.iter().any(|e| matches!(
        e,
        Event::Achieved { .. } | Event::Transitioned { .. } | Event::ChapterCompleted { .. }
    )));
    assert_eq!(run.current, before.current);
    assert_eq!(run.generation, before.generation);
    assert_eq!(run.busy, before.busy);
    assert_eq!(run.completed, before.completed);
}

#[test]
fn ch1_pack_st_1_3() {
    let s = step_of(json!({
        "id": "st-1-3", "title": "2 行のメモを送る",
        "input": { "kind": "text", "max": 20000, "min": 1 },
        "rules": [{ "id": "r-memo", "on": { "text": true },
            "outcome": { "kind": "accept",
                "effects": [{ "put": { "attrs": { "state": "shown" }, "path": "cards/memo" } }] } }],
    }));
    let (_, run, _) = apply_flow(&s, "一行目\n二行目");
    assert_eq!(run.world["cards/memo"].attrs["state"], "shown");
}

#[test]
fn judging_properties() {
    let strat = (
        0u32..6,
        0u32..6,
        "\\PC{0,12}|[ \\n\\t]{0,4}[a-z]{0,6}[ \\n]{0,3}",
    );
    check("text-judging", DEFAULT_CASES, strat, |(a, b, text)| {
        let (min, max) = (a.min(b), a.max(b));
        let s = text_step(min, max, memo_rules());
        let j1 = judge(&s, &text).expect("valid declaration");
        let j2 = judge(&s, &text).expect("valid declaration");
        prop_assert_eq!(&j1, &j2);
        let n = text.trim().chars().count();
        let in_range = !text.trim().is_empty() && n >= min as usize && n <= max as usize;
        match j1 {
            TextJudgment::Accepted(acc) => {
                prop_assert!(in_range);
                prop_assert_eq!(&acc.text, &text);
                let (_, run, ev) = apply_flow(&s, &text);
                prop_assert!(
                    !ev.iter().any(|e| matches!(e, Event::Achieved { .. })),
                    "achieved"
                );
                prop_assert_eq!(
                    run.slices[&Root("main".into())].status[&uid()],
                    StepStatus::Accepted
                );
            }
            TextJudgment::Rejected(_) => prop_assert!(!in_range),
        }
        Ok(())
    });
}

#[test]
fn large_input_is_bounded() {
    let s = text_step(1, 10, json!([]));
    let big = "あ".repeat(200_000);
    assert_eq!(
        judge(&s, &big),
        Ok(TextJudgment::Rejected(TextVerdict::TooLong))
    );
    let huge = text_step(0, u32::MAX, json!([]));
    assert!(matches!(judge(&huge, "x"), Ok(TextJudgment::Accepted(_))));
}
