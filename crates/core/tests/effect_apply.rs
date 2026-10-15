//! 状態変化の語彙（要素・属性・フラグ）の適用の試験（TASK-43 1/2、PACK-9）。

use std::collections::BTreeMap;

use fandhe_workshop_contract::effect::{Effect, PutEffect, SetAttrEffect};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::effect::{apply_effect, GLOBAL_FLAG_SCOPE};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::session::RunState;
use fandhe_workshop_core::world::Element;

fn stamp() -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: StepUid("main/a".to_string()),
    }
}

fn run_with(effect: &Effect, run: &mut RunState, root: &str) -> Vec<Event> {
    apply_effect(run, &Root(root.to_string()), effect, &stamp(), Cause::Input)
}

fn put_effect(path: &str, fixture: Option<&str>, attrs: &[(&str, &str)]) -> PutEffect {
    PutEffect {
        path: path.to_string(),
        fixture: fixture.map(str::to_string),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect::<BTreeMap<_, _>>(),
    }
}

fn el(fixture: Option<&str>, attrs: &[(&str, &str)]) -> Element {
    Element {
        fixture: fixture.map(str::to_string),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

#[test]
fn pack9_put_adds_element_and_overwrites() {
    let mut run = RunState::default();
    let e1 = Effect::Put(put_effect("a", Some("f"), &[("k", "v")]));
    let ev = run_with(&e1, &mut run, "main");
    assert_eq!(run.world["a"], el(Some("f"), &[("k", "v")]));
    assert_eq!(
        ev,
        vec![Event::Changed {
            at: stamp(),
            path: "a".into(),
            before: None,
            after: Some(el(Some("f"), &[("k", "v")])),
            cause: Cause::Input,
        }]
    );
    let e2 = Effect::Put(put_effect("a", None, &[]));
    let ev = run_with(&e2, &mut run, "main");
    assert_eq!(run.world["a"], el(None, &[]));
    assert_eq!(ev.len(), 1);
    match &ev[0] {
        Event::Changed { before, after, .. } => {
            assert_eq!(before, &Some(el(Some("f"), &[("k", "v")])));
            assert_eq!(after, &Some(el(None, &[])));
        }
        e => panic!("{e:?}"),
    }
}

#[test]
fn pack9_remove_deletes_or_warns() {
    let mut run = RunState::default();
    run.world.insert("a".into(), el(None, &[]));
    let ev = run_with(&Effect::Remove("a".into()), &mut run, "main");
    assert!(run.world.is_empty());
    assert!(matches!(
        &ev[..],
        [Event::Changed {
            before: Some(_),
            after: None,
            ..
        }]
    ));
    let before = run.clone();
    let ev = run_with(&Effect::Remove("a".into()), &mut run, "main");
    assert_eq!(run, before);
    assert!(matches!(&ev[..], [Event::Warning { .. }]));
}

#[test]
fn pack9_replace_replaces_or_puts() {
    let mut run = RunState::default();
    run.world.insert("a".into(), el(Some("f"), &[("k", "v")]));
    let ev = run_with(
        &Effect::Replace(put_effect("a", Some("g"), &[])),
        &mut run,
        "main",
    );
    assert_eq!(run.world["a"], el(Some("g"), &[]));
    assert!(matches!(
        &ev[..],
        [Event::Changed {
            before: Some(_),
            after: Some(_),
            ..
        }]
    ));
    let ev = run_with(
        &Effect::Replace(put_effect("b", None, &[])),
        &mut run,
        "main",
    );
    assert!(run.world.contains_key("b"));
    assert!(matches!(&ev[..], [Event::Changed { before: None, .. }]));
}

#[test]
fn pack9_set_attr_changes_only_the_attr() {
    let mut run = RunState::default();
    run.world
        .insert("a".into(), el(Some("f"), &[("k", "v"), ("x", "y")]));
    let eff = Effect::SetAttr(SetAttrEffect {
        path: "a".into(),
        attr: "k".into(),
        value: "new".into(),
    });
    let ev = run_with(&eff, &mut run, "main");
    assert_eq!(run.world["a"], el(Some("f"), &[("k", "new"), ("x", "y")]));
    assert!(matches!(
        &ev[..],
        [Event::Changed {
            before: Some(_),
            after: Some(_),
            ..
        }]
    ));
    let eff = Effect::SetAttr(SetAttrEffect {
        path: "none".into(),
        attr: "k".into(),
        value: "SECRET-VALUE".into(),
    });
    let before = run.clone();
    let ev = run_with(&eff, &mut run, "main");
    assert_eq!(run, before);
    match &ev[..] {
        [Event::Warning { message, .. }] => assert!(!message.contains("SECRET-VALUE")),
        e => panic!("{e:?}"),
    }
}

#[test]
fn pack9_flags_are_per_root() {
    let mut run = RunState::default();
    let ev = run_with(&Effect::SetFlag("f".into()), &mut run, "main");
    assert!(run.slices[&Root("main".into())].flags.contains("f"));
    assert!(!run.slices.contains_key(&Root("m@x".into())));
    assert_eq!(
        ev,
        vec![Event::Flag {
            at: stamp(),
            scope: "main".into(),
            name: "f".into(),
            on: true
        }]
    );
    run_with(&Effect::SetFlag("f".into()), &mut run, "m@x");
    let ev = run_with(&Effect::ClearFlag("f".into()), &mut run, "main");
    assert!(!run.slices[&Root("main".into())].flags.contains("f"));
    assert!(run.slices[&Root("m@x".into())].flags.contains("f"));
    assert!(matches!(&ev[..], [Event::Flag { on: false, .. }]));
    // 切り身が無い根を下ろしても作らない。
    run_with(&Effect::ClearFlag("f".into()), &mut run, "none");
    assert!(!run.slices.contains_key(&Root("none".into())));
}

#[test]
fn pack9_global_flag_touches_no_slice() {
    let mut run = RunState::default();
    let ev = run_with(&Effect::SetFlagGlobal("g".into()), &mut run, "main");
    assert!(run.global_flags.contains("g"));
    assert!(run.slices.is_empty());
    assert_eq!(
        ev,
        vec![Event::Flag {
            at: stamp(),
            scope: GLOBAL_FLAG_SCOPE.into(),
            name: "g".into(),
            on: true
        }]
    );
}

#[test]
fn pack9_apply_is_deterministic() {
    let eff = Effect::Put(put_effect("a", Some("f"), &[("k", "v")]));
    let (mut r1, mut r2) = (RunState::default(), RunState::default());
    let e1 = run_with(&eff, &mut r1, "main");
    let e2 = run_with(&eff, &mut r2, "main");
    assert_eq!(r1, r2);
    assert_eq!(e1, e2);
}

// ---- when つきの列と進行（TASK-43 2/2） ----

use fandhe_workshop_contract::effect::{EffectItem, ProcessingTick};
use fandhe_workshop_core::effect::{apply_effects, apply_tick, ConditionEval, NoConditions};
use serde_json::{json, Value};

/// 試験専用の評価器。`{"flag": 名前}` を `main` の切り身で見るだけで、製品の条件の語彙ではない。
struct FlagStub;
impl ConditionEval for FlagStub {
    fn eval(&self, when: &Value, run: &RunState, root: &Root) -> Option<bool> {
        let name = when.get("flag")?.as_str()?;
        Some(run.slices.get(root).is_some_and(|s| s.flags.contains(name)))
    }
}

struct Fixed(Option<bool>);
impl ConditionEval for Fixed {
    fn eval(&self, _: &Value, _: &RunState, _: &Root) -> Option<bool> {
        self.0
    }
}

fn items(v: Value) -> Vec<EffectItem> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| EffectItem::from_value(x).unwrap())
        .collect()
}

fn effects(run: &mut RunState, v: Value, cond: &dyn ConditionEval) -> Vec<Event> {
    let root = Root("main".into());
    apply_effects(run, &root, &items(v), &stamp(), Cause::Input, cond)
}

#[test]
fn pack9_when_false_skips_without_change_or_event() {
    let mut run = RunState::default();
    let ev = effects(
        &mut run,
        json!([{"put": {"path": "a"}, "when": {"x": 1}}]),
        &Fixed(Some(false)),
    );
    assert!(ev.is_empty());
    assert_eq!(run, RunState::default());
}

#[test]
fn pack9_when_true_and_absent_apply() {
    let mut run = RunState::default();
    let ev = effects(
        &mut run,
        json!([{"put": {"path": "a"}, "when": {"x": 1}}, {"put": {"path": "b"}}]),
        &Fixed(Some(true)),
    );
    assert_eq!(ev.len(), 2);
    assert!(run.world.contains_key("a") && run.world.contains_key("b"));
}

#[test]
fn pack9_when_unevaluable_skips_with_one_warning() {
    let mut run = RunState::default();
    let ev = effects(
        &mut run,
        json!([{"put": {"path": "a"}, "when": {"SECRET-VALUE": 1}}]),
        &Fixed(None),
    );
    assert!(run.world.is_empty());
    match &ev[..] {
        [Event::Warning { message, .. }] => assert!(!message.contains("SECRET-VALUE")),
        e => panic!("{e:?}"),
    }
    // 既定の評価器では when つきの項目は適用されない。
    let mut run = RunState::default();
    let ev = effects(
        &mut run,
        json!([{"set_flag": "f", "when": {"flag": "g"}}]),
        &NoConditions,
    );
    assert!(run.slices.is_empty());
    assert!(matches!(&ev[..], [Event::Warning { .. }]));
}

#[test]
fn pack9_when_sees_earlier_items_of_the_same_list() {
    let mut run = RunState::default();
    let ev = effects(
        &mut run,
        json!([
            {"put": {"path": "no"}, "when": {"flag": "f"}},
            {"set_flag": "f"},
            {"put": {"path": "yes"}, "when": {"flag": "f"}}
        ]),
        &FlagStub,
    );
    assert!(!run.world.contains_key("no"));
    assert!(run.world.contains_key("yes"));
    assert_eq!(ev.len(), 2);
}

#[test]
fn pack9_tick_emits_progress_then_effects_and_keeps_busy() {
    let tick = ProcessingTick::from_value(&json!({"name": "scan", "effects": [
        {"put": {"path": "a"}}, {"set_flag": "f"}
    ]}))
    .unwrap();
    let mut run = RunState::default();
    let root = Root("main".into());
    let cond = NoConditions;
    let ev = apply_tick(&mut run, &root, &tick, 0, 2, &stamp(), Cause::Input, &cond);
    assert_eq!(
        ev[0],
        Event::Progress {
            at: stamp(),
            name: "scan".into(),
            index: 0,
            total: 2
        }
    );
    assert!(matches!(&ev[1], Event::Changed { .. }));
    assert!(matches!(&ev[2], Event::Flag { on: true, .. }));
    assert_eq!(ev.len(), 3);
    assert_eq!(run.busy, None);
    // 空の effects でも Progress は 1 件。
    let empty = ProcessingTick::from_value(&json!({"name": "n"})).unwrap();
    let ev = apply_tick(&mut run, &root, &empty, 1, 2, &stamp(), Cause::Input, &cond);
    assert!(matches!(&ev[..], [Event::Progress { index: 1, .. }]));
}

#[test]
fn pack9_ticks_accumulate_and_are_deterministic() {
    let ticks: Vec<ProcessingTick> = [
        json!({"name": "a", "effects": [{"set_flag": "f"}]}),
        json!({"name": "b", "effects": [{"put": {"path": "x"}, "when": {"flag": "f"}}]}),
    ]
    .iter()
    .map(|v| ProcessingTick::from_value(v).unwrap())
    .collect();
    let root = Root("main".into());
    let play = || {
        let mut run = RunState::default();
        let mut all = Vec::new();
        for (i, t) in ticks.iter().enumerate() {
            let at = stamp();
            all.extend(apply_tick(
                &mut run,
                &root,
                t,
                i as u32,
                2,
                &at,
                Cause::Input,
                &FlagStub,
            ));
        }
        (run, all)
    };
    let (r1, e1) = play();
    let (r2, e2) = play();
    assert!(r1.world.contains_key("x"));
    assert_eq!((r1, e1), (r2, e2));
}
