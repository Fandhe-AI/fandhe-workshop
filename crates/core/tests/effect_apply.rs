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
