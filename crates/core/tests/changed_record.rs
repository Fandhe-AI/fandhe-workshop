//! 操作 1 件の `Changed` の記録の試験（TASK-215 1/2、EVT-13・P-1b）。

mod common;

use std::path::PathBuf;

use common::property::check;
use fandhe_workshop_contract::effect::{Effect, PutEffect, SetAttrEffect};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::changed::{cause_of, record_changes, replay_changes, world_diff};
use fandhe_workshop_core::effect::apply_effect;
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState};
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_core::{apply, load_ir};
use proptest::prelude::*;

fn stamp(step: &str) -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: StepUid(step.to_string()),
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

fn put_body(path: &str, fixture: Option<&str>, attrs: &[(&str, &str)]) -> PutEffect {
    PutEffect {
        path: path.to_string(),
        fixture: fixture.map(str::to_string),
        attrs: attrs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect(),
    }
}

fn put(path: &str, fixture: Option<&str>, attrs: &[(&str, &str)]) -> Effect {
    Effect::Put(put_body(path, fixture, attrs))
}

fn word(run: &mut RunState, effect: &Effect, at: &Stamp) -> Vec<Event> {
    apply_effect(run, &Root("main".to_string()), effect, at, Cause::Input)
}

fn changed_events(events: &[Event]) -> Vec<(&str, &Option<Element>, &Option<Element>)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Changed {
                path,
                before,
                after,
                ..
            } => Some((path.as_str(), before, after)),
            _ => None,
        })
        .collect()
}

#[test]
fn evt13_world_diff_covers_add_remove_change_and_equal() {
    let empty = World::new();
    assert!(world_diff(&empty, &empty).is_empty());

    let mut a = World::new();
    a.insert("x".into(), el(Some("f"), &[("k", "1")]));
    a.insert("same".into(), el(None, &[]));
    let mut b = a.clone();
    assert!(world_diff(&a, &b).is_empty(), "同じ値は差分に出ない");

    b.insert("y".into(), el(None, &[]));
    b.remove("same");
    if let Some(e) = b.get_mut("x") {
        e.attrs.insert("k".into(), "2".into());
    }
    let d = world_diff(&a, &b);
    let paths: Vec<&str> = d.iter().map(|(p, _, _)| p.as_str()).collect();
    assert_eq!(paths, ["same", "x", "y"], "辞書順");
    assert_eq!(d[0].1, Some(el(None, &[])));
    assert_eq!(d[0].2, None);
    assert_eq!(d[1].1, Some(el(Some("f"), &[("k", "1")])));
    assert_eq!(d[1].2, Some(el(Some("f"), &[("k", "2")])));
    assert_eq!(d[2].1, None);
    assert_eq!(d[2].2, Some(el(None, &[])));
}

#[test]
fn evt13_cause_of_table() {
    let t = TickAddr {
        generation: Generation::INITIAL,
        step: StepUid("main/a".into()),
        index: 0,
    };
    assert_eq!(cause_of(&Op::Input(Input::Cancel)), Cause::Input);
    assert_eq!(cause_of(&Op::Tick(t)), Cause::Input);
    assert_eq!(cause_of(&Op::Control(Control::Stop)), Cause::Stop);
    assert_eq!(
        cause_of(&Op::Control(Control::GoTo(StepUid("main/a".into())))),
        Cause::Move
    );
    assert_eq!(
        cause_of(&Op::Control(Control::Restore(CheckpointId(1)))),
        Cause::Restore
    );
    assert_eq!(
        cause_of(&Op::Control(Control::Begin("b".into()))),
        Cause::Start
    );
}

#[test]
fn evt13_same_path_changed_twice_is_one_change_with_original_before() {
    let mut run = RunState::default();
    run.world.insert("a".into(), el(Some("f0"), &[]));
    let before = run.world.clone();
    let at1 = stamp("main/st-1");
    let at2 = stamp("main/st-2");
    let mut events = vec![Event::Entered { at: at1.clone() }];
    events.extend(word(&mut run, &put("a", Some("f1"), &[]), &at1));
    events.push(Event::Accepted { at: at1.clone() });
    events.extend(word(&mut run, &put("a", Some("f2"), &[]), &at2));
    let out = record_changes(events, &before, &run.world, &at1, Cause::Input);
    let ch = changed_events(&out);
    assert_eq!(ch.len(), 1);
    assert_eq!(ch[0].1, &Some(el(Some("f0"), &[])));
    assert_eq!(ch[0].2, &Some(el(Some("f2"), &[])));
    // Changed 以外の順は保たれ、Changed は末尾に来る。
    assert!(matches!(out[0], Event::Entered { .. }));
    assert!(matches!(out[1], Event::Accepted { .. }));
    // 宛先は語レベルの最後のもの（後勝ち）。
    match &out[2] {
        Event::Changed { at, .. } => assert_eq!(at, &at2),
        other => panic!("Changed が末尾に来ていない: {other:?}"),
    }
}

#[test]
fn evt13_put_then_remove_and_same_value_put_record_nothing() {
    let mut run = RunState::default();
    run.world.insert("k".into(), el(Some("f"), &[]));
    let before = run.world.clone();
    let at = stamp("main/st-1");
    let mut events = word(&mut run, &put("tmp", None, &[]), &at);
    events.extend(word(&mut run, &Effect::Remove("tmp".into()), &at));
    events.extend(word(&mut run, &put("k", Some("f"), &[]), &at));
    let out = record_changes(events, &before, &run.world, &at, Cause::Input);
    assert!(changed_events(&out).is_empty());
}

#[test]
fn evt13_whole_replacement_uses_default_at_and_cause() {
    let mut before = World::new();
    before.insert("a".into(), el(None, &[]));
    before.insert("b".into(), el(None, &[]));
    let mut after = World::new();
    after.insert("b".into(), el(Some("f"), &[]));
    let at = stamp("main/st-1");
    let out = record_changes(Vec::new(), &before, &after, &at, Cause::Restore);
    assert_eq!(out.len(), 2);
    for e in &out {
        match e {
            Event::Changed { at: a, cause, .. } => {
                assert_eq!(a, &at);
                assert_eq!(*cause, Cause::Restore);
            }
            other => panic!("Changed 以外が出た: {other:?}"),
        }
    }
}

#[test]
fn evt13_apply_skeleton_records_no_changes_and_log_matches() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let bytes = std::fs::read(base.join("engine-event-consistency/test-data/min-3step.json"))
        .expect("read testdata");
    let ir = load_ir(&bytes).expect("load ir");
    let mut state = SessionState::default();
    let ops = [
        Op::Input(Input::Cancel),
        Op::Control(Control::Stop),
        Op::Control(Control::Save),
    ];
    for op in ops {
        let n = state.log.len();
        let (next, events) = apply(&ir, state, op);
        assert!(changed_events(&events).is_empty());
        assert_eq!(&next.log[n..], events.as_slice());
        state = next;
    }
}

#[derive(Debug, Clone)]
enum W {
    Put(u8, u8, u8),
    Remove(u8),
    Replace(u8, u8),
    SetAttr(u8, u8, u8),
    /// 以前の時点の世界状態へ丸ごと戻す（停止・復帰の模擬）。
    Whole(u8),
}

fn path_of(i: u8) -> String {
    format!("p{}", i % 4)
}

fn word_of(w: &W) -> Option<Effect> {
    Some(match w {
        W::Put(p, f, v) => put(
            &path_of(*p),
            Some(&format!("f{}", f % 2)),
            &[("k", &format!("{}", v % 2))],
        ),
        W::Remove(p) => Effect::Remove(path_of(*p)),
        W::Replace(p, f) => {
            Effect::Replace(put_body(&path_of(*p), Some(&format!("f{}", f % 2)), &[]))
        }
        W::SetAttr(p, a, v) => Effect::SetAttr(SetAttrEffect {
            path: path_of(*p),
            attr: format!("a{}", a % 2),
            value: format!("{}", v % 2),
        }),
        W::Whole(_) => return None,
    })
}

fn w_strategy(with_whole: bool) -> BoxedStrategy<W> {
    let base = prop_oneof![
        (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(a, b, c)| W::Put(a, b, c)),
        any::<u8>().prop_map(W::Remove),
        (any::<u8>(), any::<u8>()).prop_map(|(a, b)| W::Replace(a, b)),
        (any::<u8>(), any::<u8>(), any::<u8>()).prop_map(|(a, b, c)| W::SetAttr(a, b, c)),
    ];
    if with_whole {
        prop_oneof![4 => base, 1 => any::<u8>().prop_map(W::Whole)].boxed()
    } else {
        base.boxed()
    }
}

fn operations(with_whole: bool) -> impl Strategy<Value = Vec<Vec<W>>> {
    proptest::collection::vec(
        proptest::collection::vec(w_strategy(with_whole), 0..=4),
        1..=50,
    )
}

/// 操作列を流し、EVT-13 と P-1b の性質を確かめる。
fn run_property(ops: Vec<Vec<W>>) -> Result<(), TestCaseError> {
    let mut run = RunState::default();
    let mut snapshots: Vec<World> = vec![World::new()];
    let mut all: Vec<Event> = Vec::new();
    let mut replayed = World::new();
    for (i, op) in ops.iter().enumerate() {
        let before = run.world.clone();
        let at = stamp(&format!("main/st-{i}"));
        let mut events = Vec::new();
        for w in op {
            match (word_of(w), w) {
                (Some(e), _) => events.extend(word(&mut run, &e, &at)),
                (None, W::Whole(n)) => {
                    run.world = snapshots[usize::from(*n) % snapshots.len()].clone();
                }
                (None, _) => {}
            }
        }
        let out = record_changes(events, &before, &run.world, &at, Cause::Input);
        let mut seen = std::collections::BTreeSet::new();
        for e in &out {
            if let Event::Changed {
                path,
                before: b,
                after: a,
                ..
            } = e
            {
                prop_assert!(seen.insert(path.clone()), "同じ経路が 2 件: {path}");
                prop_assert_eq!(b, &replayed.get(path).cloned(), "変更前が操作前の値でない");
                prop_assert_ne!(b, a);
            }
        }
        for (path, b) in &before {
            if run.world.get(path) == Some(b) {
                prop_assert!(!seen.contains(path), "変わっていない経路に Changed: {path}");
            }
        }
        replay_changes(&mut replayed, &out);
        prop_assert_eq!(&replayed, &run.world);
        snapshots.push(run.world.clone());
        all.extend(out);
    }
    // 空の世界状態から全 Changed を当てると、実際の世界状態に一致する（P-1b）。
    let mut rebuilt = World::new();
    replay_changes(&mut rebuilt, &all);
    prop_assert_eq!(rebuilt, run.world);
    Ok(())
}

#[test]
fn p1b_effects_only() {
    check("p1b-effects-only", 1000, operations(false), run_property);
}

#[test]
fn p1b_with_whole_world_replacement() {
    check(
        "p1b-with-whole-world-replacement",
        1000,
        operations(true),
        run_property,
    );
}
