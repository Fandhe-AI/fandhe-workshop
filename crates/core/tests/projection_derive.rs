//! ログ・変化の印・復習の導出の試験（TASK-215 2/2、EVT-1・EVT-13・EVT-14・P-1c・P-1d）。
//!
//! `apply()` は経由しない（`step()` の各分岐は別タスクが埋める）。手で組み立てたイベント列と、
//! 1 件目の `record_changes` で `Changed` を作る生成列で、`Projection` を直接動かす。

mod common;

use std::collections::{BTreeMap, BTreeSet};

use common::property::check;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::changed::record_changes;
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{CheckpointId, Input};
use fandhe_workshop_core::projection::{Mark, MarkKind, Projection, ReviewChange, ReviewItem};
use fandhe_workshop_core::world::{Element, World};
use proptest::prelude::*;

fn stamp(step: &str) -> Stamp {
    Stamp {
        generation: Generation::INITIAL,
        step: StepUid(step.to_string()),
    }
}

fn el(n: u8) -> Element {
    Element {
        fixture: Some(format!("f{n}")),
        attrs: BTreeMap::new(),
    }
}

fn changed(step: &str, path: &str, b: Option<u8>, a: Option<u8>, cause: Cause) -> Event {
    Event::Changed {
        at: stamp(step),
        path: path.to_string(),
        before: b.map(el),
        after: a.map(el),
        cause,
    }
}

fn entered(step: &str) -> Event {
    Event::Entered { at: stamp(step) }
}

fn saved(step: &str, id: u32, log_len: u32) -> Event {
    Event::CheckpointSaved {
        at: stamp(step),
        id: CheckpointId(id),
        log_len,
    }
}

fn restored(step: &str, id: u32, log_len: u32, owner: &str) -> Event {
    Event::Restored {
        at: stamp(step),
        id: CheckpointId(id),
        new_generation: Generation::INITIAL,
        log_len,
        owner: StepUid(owner.to_string()),
    }
}

fn mark(path: &str, kind: MarkKind) -> Mark {
    Mark {
        path: path.to_string(),
        kind,
    }
}

fn review_of<'a>(p: &'a Projection, step: &str) -> Option<&'a ReviewItem> {
    p.review().iter().find(|r| r.step.0 == step)
}

#[test]
fn evt1_log_rows_follow_the_table() {
    let a = "main/a";
    let events = vec![
        entered(a),
        Event::InputReceived {
            at: stamp(a),
            input: Input::Cancel,
        },
        Event::Accepted { at: stamp(a) },
        Event::Skipped { at: stamp(a) },
        Event::Flag {
            at: stamp(a),
            scope: "s".into(),
            name: "n".into(),
            on: true,
        },
        changed(a, "x", None, Some(1), Cause::Input),
        Event::HintOpened {
            at: stamp(a),
            opened: 1,
            total: 2,
        },
        Event::Begun {
            at: stamp(a),
            start: "s".into(),
            new_generation: Generation::INITIAL,
        },
        Event::Warning {
            at: stamp("main/b"),
            message: "w".into(),
        },
    ];
    let p = Projection::from_events(&events);
    let rows: Vec<(u32, &str)> = p
        .log()
        .iter()
        .map(|r| (r.event, r.at.step.0.as_str()))
        .collect();
    assert_eq!(rows, [(0, a), (1, a), (2, a), (8, "main/b")]);
    assert_eq!(p.consumed(), events.len());
}

#[test]
fn evt1_marks_merge_first_before_last_after() {
    let a = "main/a";
    let p = Projection::from_events(&[
        entered(a),
        changed(a, "x", None, Some(1), Cause::Input),
        changed(a, "x", Some(1), Some(2), Cause::Input),
        changed(a, "y", Some(1), Some(2), Cause::Input),
        changed(a, "z", Some(1), Some(2), Cause::Input),
        changed(a, "z", Some(2), None, Cause::Input),
        changed(a, "w", None, Some(1), Cause::Input),
        changed(a, "w", Some(1), None, Cause::Input),
    ]);
    assert_eq!(
        p.marks(),
        [
            mark("w", MarkKind::Removed),
            mark("x", MarkKind::Put),
            mark("y", MarkKind::AttrChanged),
            mark("z", MarkKind::Removed),
        ]
    );
    let item = review_of(&p, a).expect("item");
    let x = item.changes.iter().find(|c| c.path == "x").expect("x");
    assert_eq!((x.before.clone(), x.after.clone()), (None, Some(el(2))));
}

#[test]
fn evt1_boundary_events_clear_marks() {
    let a = "main/a";
    let boundaries = vec![
        entered("main/b"),
        Event::Stopped {
            at: stamp(a),
            new_generation: Generation::INITIAL,
        },
        Event::Moved {
            at: stamp(a),
            to: StepUid("main/b".into()),
            new_generation: Generation::INITIAL,
        },
        restored(a, 1, 0, a),
        Event::Begun {
            at: stamp(a),
            start: "s".into(),
            new_generation: Generation::INITIAL,
        },
    ];
    for boundary in boundaries {
        let events = vec![
            entered(a),
            saved(a, 1, 1),
            changed(a, "x", None, Some(1), Cause::Input),
            boundary,
        ];
        let p = Projection::from_events(&events);
        assert!(p.marks().is_empty(), "{:?} で印が残った", events[3]);
    }
}

#[test]
fn reloaded_adds_one_row_and_keeps_marks() {
    let a = "main/a";
    let reloaded = |g: u64| Event::Reloaded {
        at: Stamp {
            generation: Generation(g),
            step: StepUid(a.to_string()),
        },
        new_generation: Generation(g + 1),
    };
    let events = vec![
        entered(a),
        changed(a, "x", None, Some(1), Cause::Input),
        reloaded(1),
        reloaded(2),
    ];
    let without = Projection::from_events(&events[..2]);
    let one = Projection::from_events(&events[..3]);
    let two = Projection::from_events(&events);
    // 再読み込み 1 回につき 1 行（SAVE-6・D-57）。行は `Reloaded` の位置と前の世代を指す。
    assert_eq!(one.log().len(), without.log().len() + 1);
    assert_eq!(two.log().len(), without.log().len() + 2);
    let last = two.log().last().expect("row");
    assert_eq!(last.event, 3);
    assert_eq!(last.at.generation, Generation(2));
    // 処理が続くので変化の印は消さない（SAVE-4）。
    assert_eq!(two.marks(), without.marks());
    assert!(!two.marks().is_empty());
    assert_eq!(two.consumed(), events.len());
    // 増分と一括は一致する。
    let mut inc = Projection::new();
    for n in 1..=events.len() {
        inc.extend(&events[..n]);
    }
    assert_eq!(inc, two);
}

#[test]
fn transition_limit_reached_adds_one_row_and_keeps_marks() {
    let a = "main/a";
    let limit = Event::TransitionLimitReached {
        at: Stamp {
            generation: Generation(3),
            step: StepUid(a.to_string()),
        },
        landings: 1000,
    };
    let events = vec![
        entered(a),
        changed(a, "x", None, Some(1), Cause::Input),
        limit,
    ];
    let without = Projection::from_events(&events[..2]);
    let with = Projection::from_events(&events);
    // 上限で止めるたびに 1 行（SAFE-34）。行は専用のイベントの位置と止めた地点の世代を指す。
    assert_eq!(with.log().len(), without.log().len() + 1);
    let last = with.log().last().expect("row");
    assert_eq!(last.event, 2);
    assert_eq!(last.at.generation, Generation(3));
    // 変化の印は基準点でしか消えない。
    assert_eq!(with.marks(), without.marks());
    assert_eq!(with.consumed(), events.len());
}

#[test]
fn evt13_non_input_changes_are_not_marked_or_reviewed() {
    let a = "main/a";
    let events: Vec<Event> = [Cause::Stop, Cause::Move, Cause::Restore, Cause::Start]
        .into_iter()
        .map(|c| changed(a, "x", None, Some(1), c))
        .collect();
    let p = Projection::from_events(&events);
    assert!(p.marks().is_empty());
    assert!(p.review().is_empty());
    assert!(p.log().is_empty());
}

#[test]
fn evt13_review_merges_per_step_and_path() {
    let p = Projection::from_events(&[
        changed("main/b", "p", None, Some(1), Cause::Input),
        changed("main/a", "q", Some(1), Some(2), Cause::Input),
        changed("main/b", "p", Some(1), Some(3), Cause::Input),
        changed("main/b", "r", None, Some(1), Cause::Input),
        Event::Achieved {
            at: stamp("main/a"),
            explicit_skip: false,
        },
        Event::Skipped {
            at: stamp("main/c"),
        },
    ]);
    let steps: Vec<&str> = p.review().iter().map(|r| r.step.0.as_str()).collect();
    assert_eq!(steps, ["main/b", "main/a", "main/c"]);
    assert_eq!(
        review_of(&p, "main/b").expect("b").changes,
        vec![
            ReviewChange {
                path: "p".into(),
                before: None,
                after: Some(el(3)),
            },
            ReviewChange {
                path: "r".into(),
                before: None,
                after: Some(el(1)),
            },
        ]
    );
    let a = review_of(&p, "main/a").expect("a");
    assert!(a.achieved && !a.skipped);
    let c = review_of(&p, "main/c").expect("c");
    assert!(c.skipped && !c.achieved && c.changes.is_empty());
}

#[test]
fn evt13_stop_resets_the_stopped_step_review() {
    let p = Projection::from_events(&[
        changed("main/a", "p", None, Some(1), Cause::Input),
        Event::Achieved {
            at: stamp("main/a"),
            explicit_skip: false,
        },
        changed("main/b", "q", None, Some(1), Cause::Input),
        Event::Stopped {
            at: stamp("main/a"),
            new_generation: Generation::INITIAL,
        },
    ]);
    let a = review_of(&p, "main/a").expect("a");
    assert!(a.changes.is_empty() && !a.achieved && !a.skipped);
    assert_eq!(review_of(&p, "main/b").expect("b").changes.len(), 1);
    // 項目の無い Step の停止は項目を作らない。
    let q = Projection::from_events(&[Event::Stopped {
        at: stamp("main/z"),
        new_generation: Generation::INITIAL,
    }]);
    assert!(q.review().is_empty());
}

#[test]
fn evt14_restore_rebuilds_owner_review_and_keeps_other_roots() {
    let events = vec![
        entered("main/a"),
        changed("main/a", "x", None, Some(1), Cause::Input),
        saved("main/a", 1, 2),
        entered("m@a/s"),
        changed("m@a/s", "y", None, Some(1), Cause::Input),
        changed("main/a", "z", None, Some(1), Cause::Input),
        restored("main/a", 1, 2, "main/a"),
    ];
    let p = Projection::from_events(&events);
    let steps: Vec<&str> = p.review().iter().map(|r| r.step.0.as_str()).collect();
    assert_eq!(steps, ["main/a", "m@a/s"]);
    let main = review_of(&p, "main/a").expect("main");
    assert_eq!(main.changes.len(), 1);
    assert_eq!(main.changes[0].path, "x");
    assert_eq!(review_of(&p, "m@a/s").expect("m").changes.len(), 1);
    // 持ち主の根の項目は、保存の時点までの一括に等しい。
    let at_save = Projection::from_events(&events[..2]);
    assert_eq!(review_of(&p, "main/a"), review_of(&at_save, "main/a"));
}

#[test]
fn evt14_restore_with_unknown_log_len_does_not_panic() {
    for bad in [0u32, 1, 2, 3, 99, u32::MAX] {
        let events = vec![
            entered("main/a"),
            changed("main/a", "x", None, Some(1), Cause::Input),
            changed("m@a/s", "y", None, Some(1), Cause::Input),
            restored("main/a", 1, bad, "main/a"),
        ];
        let p = Projection::from_events(&events);
        assert!(review_of(&p, "main/a").is_none(), "log_len={bad}");
        assert_eq!(review_of(&p, "m@a/s").expect("m").changes.len(), 1);
        assert!(p.marks().is_empty());
    }
}

#[test]
fn evt1_extend_with_shorter_slice_is_noop() {
    let events = vec![
        entered("main/a"),
        changed("main/a", "x", None, Some(1), Cause::Input),
    ];
    let mut p = Projection::from_events(&events);
    let snapshot = p.clone();
    p.extend(&events[..1]);
    p.extend(&[]);
    assert_eq!(p, snapshot);
    assert_eq!(p.consumed(), 2);
}

// ---- 性質テスト ----

const STEPS: [&str; 4] = ["main/a", "main/b", "m@a/x", "m@a/y"];
const PATHS: [&str; 4] = ["p0", "p1", "p2", "p3"];

#[derive(Debug, Clone)]
enum Op {
    Input {
        step: u8,
        world: [Option<u8>; 4],
        extra: u8,
    },
    Stop {
        step: u8,
        pick: u8,
    },
    Move {
        step: u8,
        pick: u8,
    },
    Begin {
        pick: u8,
    },
    Save {
        step: u8,
    },
    Restore {
        pick: u8,
    },
}

fn world_strategy() -> impl Strategy<Value = [Option<u8>; 4]> {
    proptest::array::uniform4(proptest::option::of(0u8..3))
}

fn operations(with_save_restore: bool) -> impl Strategy<Value = Vec<Op>> {
    let input = (0u8..4, world_strategy(), 0u8..6).prop_map(|(step, world, extra)| Op::Input {
        step,
        world,
        extra,
    });
    let stop = (0u8..4, any::<u8>()).prop_map(|(step, pick)| Op::Stop { step, pick });
    let mv = (0u8..4, any::<u8>()).prop_map(|(step, pick)| Op::Move { step, pick });
    let begin = any::<u8>().prop_map(|pick| Op::Begin { pick });
    let save = (0u8..4).prop_map(|step| Op::Save { step });
    let restore = any::<u8>().prop_map(|pick| Op::Restore { pick });
    let op = if with_save_restore {
        prop_oneof![
            6 => input,
            1 => stop,
            1 => mv,
            1 => begin,
            2 => save,
            3 => restore,
        ]
        .boxed()
    } else {
        prop_oneof![6 => input, 1 => stop, 1 => mv, 1 => begin].boxed()
    };
    proptest::collection::vec(op, 1..=50)
}

fn to_world(slots: &[Option<u8>; 4]) -> World {
    let mut w = World::new();
    for (path, slot) in PATHS.iter().zip(slots) {
        if let Some(n) = slot {
            w.insert((*path).to_string(), el(*n));
        }
    }
    w
}

fn is_base(e: &Event) -> bool {
    matches!(
        e,
        Event::Entered { .. }
            | Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Restored { .. }
            | Event::Begun { .. }
    )
}

fn root_of(step: &str) -> Root {
    StepUid(step.to_string()).root()
}

struct Built {
    /// 操作ごとのイベント列。
    ops: Vec<Vec<Event>>,
    /// 操作ごとの、復帰なら（`log_len`, 持ち主の Step）。
    restores: Vec<Option<(usize, String)>>,
}

/// 操作列から、世界状態の前後の差分で `Changed` を作った操作ごとのイベント列を組み立てる。
fn build(ops: &[Op]) -> Built {
    let mut world = World::new();
    let mut log_len = 0usize;
    let mut saves: Vec<(u32, usize, World, String)> = Vec::new();
    let mut snapshots: Vec<World> = vec![World::new()];
    let mut out = Vec::new();
    let mut restores = Vec::new();
    for op in ops {
        let mut events = Vec::new();
        let mut restore = None;
        let (target, step, cause): (World, String, Cause);
        match op {
            Op::Input {
                step: s,
                world: w,
                extra,
            } => {
                let st = STEPS[usize::from(*s) % STEPS.len()];
                events.push(Event::InputReceived {
                    at: stamp(st),
                    input: Input::Cancel,
                });
                events.push(Event::Accepted { at: stamp(st) });
                match extra {
                    0 => events.push(Event::Achieved {
                        at: stamp(st),
                        explicit_skip: false,
                    }),
                    1 => events.push(Event::Skipped { at: stamp(st) }),
                    2 => events.push(Event::Transitioned {
                        at: stamp(st),
                        to: StepUid(STEPS[0].to_string()),
                    }),
                    3 => events.push(entered(st)),
                    _ => {}
                }
                target = to_world(w);
                step = st.to_string();
                cause = Cause::Input;
            }
            Op::Stop { step: s, pick } => {
                let st = STEPS[usize::from(*s) % STEPS.len()];
                events.push(Event::Stopped {
                    at: stamp(st),
                    new_generation: Generation::INITIAL,
                });
                target = snapshots[usize::from(*pick) % snapshots.len()].clone();
                step = st.to_string();
                cause = Cause::Stop;
            }
            Op::Move { step: s, pick } => {
                let st = STEPS[usize::from(*s) % STEPS.len()];
                events.push(Event::Moved {
                    at: stamp(st),
                    to: StepUid(st.to_string()),
                    new_generation: Generation::INITIAL,
                });
                target = snapshots[usize::from(*pick) % snapshots.len()].clone();
                step = st.to_string();
                cause = Cause::Move;
            }
            Op::Begin { pick } => {
                events.push(Event::Begun {
                    at: stamp(STEPS[0]),
                    start: "s".into(),
                    new_generation: Generation::INITIAL,
                });
                target = snapshots[usize::from(*pick) % snapshots.len()].clone();
                step = STEPS[0].to_string();
                cause = Cause::Start;
            }
            Op::Save { step: s } => {
                let st = STEPS[usize::from(*s) % STEPS.len()];
                let id = u32::try_from(saves.len()).unwrap_or(0);
                events.push(saved(st, id, u32::try_from(log_len).unwrap_or(0)));
                saves.push((id, log_len, world.clone(), st.to_string()));
                target = world.clone();
                step = st.to_string();
                cause = Cause::Input;
            }
            Op::Restore { pick } => {
                if saves.is_empty() {
                    target = world.clone();
                    step = STEPS[0].to_string();
                    cause = Cause::Input;
                } else {
                    let (id, len, w, owner) = saves[usize::from(*pick) % saves.len()].clone();
                    events.push(restored(
                        &owner,
                        id,
                        u32::try_from(len).unwrap_or(0),
                        &owner,
                    ));
                    restore = Some((len, owner.clone()));
                    target = w;
                    step = owner;
                    cause = Cause::Restore;
                }
            }
        }
        let events = record_changes(events, &world, &target, &stamp(&step), cause);
        world = target;
        snapshots.push(world.clone());
        log_len += events.len();
        out.push(events);
        restores.push(restore);
    }
    Built { ops: out, restores }
}

fn run_property(ops: Vec<Op>) -> Result<(), TestCaseError> {
    let built = build(&ops);
    let mut log: Vec<Event> = Vec::new();
    let mut proj = Projection::new();
    let mut fine = Projection::new();
    for (events, restore) in built.ops.iter().zip(&built.restores) {
        let review_before = proj.review().to_vec();
        let start = log.len();
        log.extend(events.iter().cloned());
        proj.extend(&log);
        for k in start..log.len() {
            fine.extend(&log[..=k]);
        }
        // (a) 増分（操作ごと・1 件ずつ）と一括の一致。
        prop_assert_eq!(&proj, &fine);
        prop_assert_eq!(&proj, &Projection::from_events(&log));

        // (b) 印の経路の集合は、最後の基準点より後の Input の Changed の経路の集合に等しい。
        let base = log.iter().rposition(is_base).map_or(0, |p| p + 1);
        let expected: BTreeSet<&str> = log[base..]
            .iter()
            .filter_map(|e| match e {
                Event::Changed {
                    path,
                    cause: Cause::Input,
                    ..
                } => Some(path.as_str()),
                _ => None,
            })
            .collect();
        let marked: BTreeSet<&str> = proj.marks().iter().map(|m| m.path.as_str()).collect();
        prop_assert_eq!(&marked, &expected);

        // (c) 基準点の時点から値が変わった経路は、すべて印に含まれる。
        let mut current = World::new();
        let mut at_base = World::new();
        for e in &log {
            if is_base(e) {
                at_base = current.clone();
            }
            if let Event::Changed {
                path, after, cause, ..
            } = e
            {
                let apply = |w: &mut World| match after {
                    Some(el) => {
                        w.insert(path.clone(), el.clone());
                    }
                    None => {
                        w.remove(path);
                    }
                };
                apply(&mut current);
                if *cause != Cause::Input {
                    apply(&mut at_base);
                }
            }
        }
        for path in PATHS {
            if current.get(path) != at_base.get(path) {
                prop_assert!(marked.contains(path), "値が変わった経路に印が無い: {path}");
            }
        }

        // (d) 復習のすべての変化に、同じ Step・同じ経路の Changed がログ上にある。
        for item in proj.review() {
            for c in &item.changes {
                let found = log.iter().any(|e| match e {
                    Event::Changed { at, path, .. } => at.step == item.step && path == &c.path,
                    _ => false,
                });
                prop_assert!(found, "対応する Changed が無い: {}", c.path);
            }
        }

        // (e) 復帰の直後、持ち主の根の項目は保存の時点までの一括に等しく、他の根の項目は変わらない。
        if let Some((len, owner)) = restore {
            let own = root_of(owner);
            let from_prefix = Projection::from_events(&log[..*len]);
            let mut expected_review: Vec<ReviewItem> = from_prefix
                .review()
                .iter()
                .filter(|r| r.step.root() == own)
                .cloned()
                .collect();
            expected_review.extend(review_before.into_iter().filter(|r| r.step.root() != own));
            prop_assert_eq!(proj.review(), &expected_review[..]);
        }
    }
    Ok(())
}

#[test]
fn p1c_p1d_without_restore() {
    check(
        "p1c-p1d-without-restore",
        1000,
        operations(false),
        run_property,
    );
}

#[test]
fn p1c_p1d_with_save_restore() {
    check(
        "p1c-p1d-with-save-restore",
        1000,
        operations(true),
        run_property,
    );
}
