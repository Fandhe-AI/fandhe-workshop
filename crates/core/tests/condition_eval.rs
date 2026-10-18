//! 条件の語彙（TASK-42 1/2）の評価の試験。9 語それぞれの真・偽と、3 値の扱い、`apply_effects` 経由の `when`。

use fandhe_workshop_contract::condition::Condition;
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::condition::{eval_condition, VocabConditions};
use fandhe_workshop_core::effect::{apply_effects, ConditionEval};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::session::{RunState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use serde_json::{json, Value};

fn root() -> Root {
    Root("main".into())
}

fn el(attrs: &[(&str, &str)]) -> Element {
    Element {
        fixture: None,
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

/// 試験用の状態。`main` の切り身に Step・選択・フラグ、世界状態に 3 要素を置く。
fn state() -> RunState {
    let mut run = RunState::default();
    let mut sl = Slice::default();
    sl.status
        .insert(StepUid("main/s1".into()), StepStatus::Achieved);
    sl.status
        .insert(StepUid("main/s2".into()), StepStatus::Entered);
    sl.choices.insert(StepUid("main/s3".into()), "opt-a".into());
    sl.flags.insert("ready".into());
    run.slices.insert(root(), sl);
    let mut other = Slice::default();
    other.flags.insert("other-only".into());
    other
        .choices
        .insert(StepUid("m@i/s3".into()), "opt-a".into());
    run.slices.insert(Root("m@i".into()), other);
    run.global_flags.insert("g".into());
    run.world.insert("docs/a".into(), el(&[("kind", "x")]));
    run.world.insert("docs/b".into(), el(&[("kind", "x")]));
    run.world.insert("docs/c".into(), el(&[("kind", "y")]));
    run.world.insert("other/d".into(), el(&[("kind", "x")]));
    run
}

fn ev(v: Value) -> Option<bool> {
    let c = Condition::from_value(&v).expect("parse");
    let got = eval_condition(&c, &state(), &root());
    // 評価器の経路でも同じ結果になる。
    assert_eq!(VocabConditions.eval(&v, &state(), &root()), got);
    got
}

#[test]
fn step_compares_status_and_unregistered_is_not_started() {
    assert_eq!(
        ev(json!({"step": {"step": "s1", "state": "achieved"}})),
        Some(true)
    );
    assert_eq!(
        ev(json!({"step": {"step": "s1", "state": "accepted"}})),
        Some(false)
    );
    assert_eq!(
        ev(json!({"step": {"step": "s2", "state": "entered"}})),
        Some(true)
    );
    assert_eq!(
        ev(json!({"step": {"step": "nope", "state": "not_started"}})),
        Some(true)
    );
    assert_eq!(
        ev(json!({"step": {"step": "nope", "state": "skipped"}})),
        Some(false)
    );
    // 別の根の Step は見ない。
    let c = Condition::from_value(&json!({"step": {"step": "s1", "state": "achieved"}})).unwrap();
    assert_eq!(
        eval_condition(&c, &state(), &Root("m@i".into())),
        Some(false)
    );
}

#[test]
fn flag_reads_only_the_slice_of_the_root() {
    assert_eq!(ev(json!({"flag": "ready"})), Some(true));
    assert_eq!(ev(json!({"flag": "absent"})), Some(false));
    assert_eq!(ev(json!({"flag": "other-only"})), Some(false));
    assert_eq!(ev(json!({"flag": "g"})), Some(false));
}

#[test]
fn world_has_and_world_attr() {
    assert_eq!(ev(json!({"world_has": "docs/a"})), Some(true));
    assert_eq!(ev(json!({"world_has": "docs"})), Some(false));
    let wa = |p: &str, a: &str, e: &str| json!({"world_attr": {"path": p, "attr": a, "eq": e}});
    assert_eq!(ev(wa("docs/a", "kind", "x")), Some(true));
    assert_eq!(ev(wa("docs/c", "kind", "x")), Some(false));
    assert_eq!(ev(wa("missing", "kind", "x")), Some(false));
    assert_eq!(ev(wa("docs/a", "nope", "x")), Some(false));
}

#[test]
fn count_ops_boundaries_where_and_prefix() {
    let ct =
        |prefix: &str, op: &str, n: u64| json!({"count": {"prefix": prefix, "op": op, "n": n}});
    // docs/ 配下は 3 件。
    assert_eq!(ev(ct("docs/", "==", 3)), Some(true));
    assert_eq!(ev(ct("docs/", "<", 3)), Some(false));
    assert_eq!(ev(ct("docs/", "<", 4)), Some(true));
    assert_eq!(ev(ct("docs/", "<=", 3)), Some(true));
    assert_eq!(ev(ct("docs/", "<=", 2)), Some(false));
    assert_eq!(ev(ct("docs/", ">=", 3)), Some(true));
    assert_eq!(ev(ct("docs/", ">=", 4)), Some(false));
    assert_eq!(ev(ct("docs/", ">", 2)), Some(true));
    assert_eq!(ev(ct("docs/", ">", 3)), Some(false));
    // 空の前置は全件（4 件）。
    assert_eq!(ev(ct("", "==", 4)), Some(true));
    // 該当なしは 0 件。
    assert_eq!(ev(ct("zzz/", "==", 0)), Some(true));
    // where で絞る。
    let w = json!({"count": {"prefix": "docs/", "where": {"kind": "x"}, "op": "==", "n": 2}});
    assert_eq!(ev(w), Some(true));
    let w =
        json!({"count": {"prefix": "", "where": {"kind": "x", "nope": "1"}, "op": "==", "n": 0}});
    assert_eq!(ev(w), Some(true));
}

#[test]
fn chose_reads_the_choice_of_the_root_and_step() {
    assert_eq!(
        ev(json!({"chose": {"step": "s3", "option": "opt-a"}})),
        Some(true)
    );
    assert_eq!(
        ev(json!({"chose": {"step": "s3", "option": "opt-b"}})),
        Some(false)
    );
    assert_eq!(
        ev(json!({"chose": {"step": "s1", "option": "opt-a"}})),
        Some(false)
    );
    let c = Condition::from_value(&json!({"chose": {"step": "s3", "option": "opt-a"}})).unwrap();
    assert_eq!(
        eval_condition(&c, &state(), &Root("m@i".into())),
        Some(true)
    );
}

#[test]
fn logic_words_and_empty_lists() {
    let t = json!({"flag": "ready"});
    let f = json!({"flag": "absent"});
    assert_eq!(ev(json!({"all": []})), Some(true));
    assert_eq!(ev(json!({"any": []})), Some(false));
    assert_eq!(ev(json!({"all": [t, t]})), Some(true));
    assert_eq!(ev(json!({"all": [t, f]})), Some(false));
    assert_eq!(ev(json!({"any": [f, t]})), Some(true));
    assert_eq!(ev(json!({"any": [f, f]})), Some(false));
    assert_eq!(ev(json!({"not": f})), Some(true));
    assert_eq!(ev(json!({"not": t})), Some(false));
    assert_eq!(
        ev(json!({"all": [{"any": [f, t]}, {"not": f}]})),
        Some(true)
    );
}

#[test]
fn unevaluable_words_are_none_and_propagate_through_logic() {
    let p = json!({"param": {"name": "n", "eq": "v"}});
    let c = json!({"clarifying": "cl-1"});
    let t = json!({"flag": "ready"});
    let f = json!({"flag": "absent"});
    assert_eq!(ev(p.clone()), None);
    assert_eq!(ev(c.clone()), None);
    assert_eq!(ev(json!({"all": [t, p]})), None);
    // 短絡しない: 偽の子があっても評価できない子があれば None。
    assert_eq!(ev(json!({"all": [f, c]})), None);
    assert_eq!(ev(json!({"any": [t, p]})), None);
    assert_eq!(ev(json!({"not": c})), None);
}

#[test]
fn unparsable_values_are_none() {
    for v in [
        json!({"script": "x"}),
        json!({}),
        json!("flag"),
        json!(null),
    ] {
        assert_eq!(VocabConditions.eval(&v, &state(), &root()), None);
    }
}

fn item(v: Value) -> EffectItem {
    EffectItem::from_value(&v).expect("item")
}

#[test]
fn apply_effects_applies_true_skips_false_and_warns_on_none() {
    let at = Stamp {
        generation: Generation::INITIAL,
        step: StepUid("main/s1".into()),
    };
    let mut run = state();
    let items = vec![
        item(json!({"set_flag": "later", "when": {"flag": "ready"}})),
        item(json!({"put": {"path": "skip", "attrs": {}}, "when": {"flag": "absent"}})),
        item(json!({"put": {"path": "seen", "attrs": {}}, "when": {"flag": "later"}})),
        item(
            json!({"put": {"path": "unk", "attrs": {}}, "when": {"param": {"name": "n", "eq": "v"}}}),
        ),
    ];
    let events = apply_effects(
        &mut run,
        &root(),
        &items,
        &at,
        Cause::Start,
        &VocabConditions,
    );
    assert!(run.slices[&root()].flags.contains("later"));
    assert!(!run.world.contains_key("skip"));
    // 同じ列の前の `set_flag` を、後の項目の `when` が見られる。
    assert!(run.world.contains_key("seen"));
    assert!(!run.world.contains_key("unk"));
    let warnings = events
        .iter()
        .filter(|e| matches!(e, Event::Warning { .. }))
        .count();
    assert_eq!(warnings, 1);
}

#[test]
fn double_negation_and_single_all_are_identities() {
    let cases = [
        json!({"flag": "ready"}),
        json!({"flag": "absent"}),
        json!({"world_has": "docs/a"}),
        json!({"count": {"prefix": "docs/", "op": ">", "n": 1}}),
        json!({"param": {"name": "n", "eq": "v"}}),
    ];
    for c in cases {
        assert_eq!(ev(json!({"not": {"not": c.clone()}})), ev(c.clone()));
        assert_eq!(ev(json!({"all": [c.clone()]})), ev(c.clone()));
        assert_eq!(ev(json!({"any": [c.clone()]})), ev(c));
    }
}
