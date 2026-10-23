//! 条件の語彙（TASK-42）の評価の試験。11 語それぞれの真・偽と、3 値の扱い、`apply_effects` 経由の `when`。

use fandhe_workshop_contract::condition::Condition;
use fandhe_workshop_contract::effect::EffectItem;
use fandhe_workshop_contract::ir::Instance;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::condition::{eval_condition, VocabConditions};
use fandhe_workshop_core::effect::{apply_effects, ConditionEval};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::session::{RunState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use serde_json::{json, Value};
use std::collections::BTreeMap;

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

/// 実体の一覧。`main` は引数なし、`m@i` は引数あり。
fn instances() -> Vec<Instance> {
    let inst = |id: &str, args: &[(&str, &str)]| Instance {
        id: id.into(),
        module: None,
        args: args
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
        caller: None,
    };
    vec![inst("main", &[]), inst("m@i", &[("n", "v"), ("k", "w")])]
}

fn args_of(root: &str) -> BTreeMap<String, String> {
    instances()
        .into_iter()
        .find(|i| i.id == root)
        .expect("instance")
        .args
}

fn ev(v: Value) -> Option<bool> {
    let c = Condition::from_value(&v).expect("parse");
    let got = eval_condition(&c, &state(), &root(), Some(&args_of("main")), &instances());
    // 評価器の経路でも同じ結果になる。
    let inst = instances();
    assert_eq!(VocabConditions::new(&inst).eval(&v, &state(), &root()), got);
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
        eval_condition(&c, &state(), &Root("m@i".into()), None, &[]),
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
        eval_condition(&c, &state(), &Root("m@i".into()), None, &[]),
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
fn param_compares_the_arguments_of_the_root_instance() {
    let inst = instances();
    let ve = VocabConditions::new(&inst);
    let p = |n: &str, e: &str| json!({"param": {"name": n, "eq": e}});
    let mi = Root("m@i".into());
    assert_eq!(ve.eval(&p("n", "v"), &state(), &mi), Some(true));
    assert_eq!(ve.eval(&p("k", "w"), &state(), &mi), Some(true));
    assert_eq!(ve.eval(&p("n", "x"), &state(), &mi), Some(false));
    assert_eq!(ve.eval(&p("absent", "v"), &state(), &mi), Some(false));
    // 章の根は引数が空なので偽。
    assert_eq!(ve.eval(&p("n", "v"), &state(), &root()), Some(false));
    // 実体が見つからない根は評価できない。
    assert_eq!(ve.eval(&p("n", "v"), &state(), &Root("zz@i".into())), None);
    assert_eq!(
        VocabConditions::new(&[]).eval(&p("n", "v"), &state(), &mi),
        None
    );
    // 論理に混ぜる。
    assert_eq!(
        ve.eval(&json!({"not": p("n", "v")}), &state(), &mi),
        Some(false)
    );
    assert_eq!(
        ve.eval(
            &json!({"all": [p("n", "v"), {"flag": "other-only"}]}),
            &state(),
            &mi
        ),
        Some(true)
    );
    assert_eq!(
        ve.eval(
            &json!({"any": [p("n", "x"), {"flag": "absent"}]}),
            &state(),
            &mi
        ),
        Some(false)
    );
    // 引数が分からないとき（None）は評価できず、論理に伝わる。
    let c = Condition::from_value(&p("n", "v")).unwrap();
    assert_eq!(eval_condition(&c, &state(), &mi, None, &[]), None);
    let all = Condition::from_value(&json!({"all": [{"flag": "absent"}, p("n", "v")]})).unwrap();
    assert_eq!(eval_condition(&all, &state(), &mi, None, &[]), None);
    let not = Condition::from_value(&json!({"not": p("n", "v")})).unwrap();
    assert_eq!(eval_condition(&not, &state(), &mi, None, &[]), None);
}

#[test]
fn clarifying_is_true_only_for_the_same_root_and_id() {
    let mut run = state();
    let c = json!({"clarifying": "cl-1"});
    let none: Vec<Instance> = Vec::new();
    let ve = VocabConditions::new(&none);
    // 待ちが無ければ偽（安全側）。
    assert_eq!(ve.eval(&c, &run, &root()), Some(false));
    run.slices.get_mut(&root()).unwrap().clarifying = Some("cl-1".into());
    assert_eq!(ve.eval(&c, &run, &root()), Some(true));
    assert_eq!(
        ve.eval(&json!({"clarifying": "cl-2"}), &run, &root()),
        Some(false)
    );
    // 別の根の待ちは見ない。
    assert_eq!(ve.eval(&c, &run, &Root("m@i".into())), Some(false));
    assert_eq!(ve.eval(&c, &run, &Root("none".into())), Some(false));
    assert_eq!(ve.eval(&json!({"not": c}), &run, &root()), Some(false));
}

#[test]
fn unparsable_values_are_none() {
    for v in [
        json!({"script": "x"}),
        json!({}),
        json!("flag"),
        json!(null),
    ] {
        assert_eq!(VocabConditions::new(&[]).eval(&v, &state(), &root()), None);
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
        &VocabConditions::new(&[]),
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

#[test]
fn apply_effects_when_param_differs_per_instance() {
    let inst = instances();
    let at = Stamp {
        generation: Generation::INITIAL,
        step: StepUid("m@i/s1".into()),
    };
    let items = vec![item(
        json!({"set_flag": "hit", "when": {"param": {"name": "n", "eq": "v"}}}),
    )];
    let mut run = state();
    for r in ["main", "m@i"] {
        apply_effects(
            &mut run,
            &Root(r.into()),
            &items,
            &at,
            Cause::Start,
            &VocabConditions::new(&inst),
        );
    }
    assert!(!run.slices[&root()].flags.contains("hit"));
    assert!(run.slices[&Root("m@i".into())].flags.contains("hit"));
}

/// 呼び出しの Step の状態・選択は呼ばれた実体の切り身にある。条件はその持ち主から読む（MOD-3）。
#[test]
fn step_and_chose_read_the_callee_slice_for_a_call_step() {
    let mut inst = instances();
    inst[1].caller = Some("main/c1".into());
    let mut run = RunState::default();
    let mut callee = Slice::default();
    callee
        .status
        .insert(StepUid("main/c1".into()), StepStatus::Entered);
    callee
        .choices
        .insert(StepUid("main/c1".into()), "opt-a".into());
    run.slices.insert(Root("m@i".into()), callee);
    let step = |state: &str| json!({"step": {"step": "c1", "state": state}});
    let eval = |run: &RunState, v: Value, instances: &[Instance]| {
        let c = Condition::from_value(&v).expect("parse");
        eval_condition(&c, run, &root(), None, instances)
    };
    assert_eq!(eval(&run, step("entered"), &inst), Some(true));
    assert_eq!(eval(&run, step("not_started"), &inst), Some(false));
    assert_eq!(eval(&run, step("achieved"), &inst), Some(false));
    assert_eq!(
        eval(
            &run,
            json!({"chose": {"step": "c1", "option": "opt-a"}}),
            &inst
        ),
        Some(true)
    );
    // 対照: 持ち主を引けない（実体の一覧が空）と実体 ID の根を見て NotStarted になる。
    assert_eq!(eval(&run, step("entered"), &[]), Some(false));
    // 評価器の経路も同じ。
    assert_eq!(
        VocabConditions::new(&inst).eval(&step("entered"), &run, &root()),
        Some(true)
    );
    // 達成後も同じ持ち主を読む。
    run.slices
        .get_mut(&Root("m@i".into()))
        .expect("slice")
        .status
        .insert(StepUid("main/c1".into()), StepStatus::Achieved);
    assert_eq!(eval(&run, step("achieved"), &inst), Some(true));
}
