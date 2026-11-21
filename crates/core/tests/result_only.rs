//! 進行イベントの列から結果だけを導く `result_only::result_only` の試験（EVT-19・A11Y-29。TASK-230 1/2。#983）。
//!
//! 確かめること:
//! - 処理中の段を持つ Step の受付から最後の進行までのイベント列で、`Progress` が 0 件になり、同じ経路の `Changed` が 1 件にまとまる
//! - まとめた `Changed` を始めの世界状態に当てると、全部の `Changed` を当てた結果・セッションの最終の世界状態と一致する
//!   （最終の状態は動きを抑えない場合と同じ）
//! - 停止で世代が進むと、停止の前の変化と停止の巻き戻しの変化をまとめない
//! - 前後が同じになった変化は除く。`Progress`・`Changed` 以外のイベントは順のまま残る
//!
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;

use std::path::PathBuf;

use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::result_only::result_only;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_core::{load_ir, Session};

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// 世界状態に `Changed` を順に当てる。
fn replay_changes(mut world: World, events: &[Event]) -> World {
    for e in events {
        if let Event::Changed { path, after, .. } = e {
            match after {
                Some(el) => {
                    world.insert(path.clone(), el.clone());
                }
                None => {
                    world.remove(path);
                }
            }
        }
    }
    world
}

/// 先頭の Step に入り、処理中の段を持つ入力を送って最後の進行まで進めたときのイベント列（送る前の世界状態も返す）。
fn processed() -> (World, Vec<Event>, Session) {
    let mut s = Session::new(ir());
    s.apply(Op::Control(Control::Begin("s-1".into())));
    let start = s.state().run.world.clone();
    let mut events = s
        .apply(Op::Input(Input::Command("/make-note first-note".into())))
        .to_vec();
    while let Some(addr) = next_tick(s.state()) {
        events.extend(s.apply(Op::Tick(addr)).to_vec());
    }
    (start, events, s)
}

fn count(events: &[Event], f: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}

#[test]
fn evt19_progress_is_dropped_and_changes_of_one_path_are_merged() {
    let (_, events, _) = processed();
    let progress = count(&events, |e| matches!(e, Event::Progress { .. }));
    assert!(progress >= 2, "試験の前提: 段が 2 つ以上ある（{progress}）");
    let changed = count(
        &events,
        |e| matches!(e, Event::Changed { path, .. } if path == "notes/draft"),
    );
    assert!(
        changed >= 2,
        "試験の前提: 同じ経路の変化が 2 件以上ある（{changed}）"
    );
    let r = result_only(&events);
    assert_eq!(count(&r, |e| matches!(e, Event::Progress { .. })), 0);
    assert_eq!(
        count(
            &r,
            |e| matches!(e, Event::Changed { path, .. } if path == "notes/draft")
        ),
        1
    );
}

#[test]
fn evt19_final_world_equals_the_full_sequence_and_the_session() {
    let (start, events, s) = processed();
    let r = result_only(&events);
    let full = replay_changes(start.clone(), &events);
    let only = replay_changes(start, &r);
    assert_eq!(only, full);
    assert_eq!(only, s.state().run.world);
}

#[test]
fn evt19_other_events_keep_their_order() {
    let (_, events, _) = processed();
    let others = |es: &[Event]| -> Vec<Event> {
        es.iter()
            .filter(|e| !matches!(e, Event::Progress { .. } | Event::Changed { .. }))
            .cloned()
            .collect()
    };
    assert_eq!(others(&result_only(&events)), others(&events));
}

fn stamp(generation: u64, step: &str) -> Stamp {
    Stamp {
        generation: Generation(generation),
        step: StepUid(step.into()),
    }
}

fn el(state: &str) -> Element {
    let mut e = Element::default();
    e.attrs.insert("state".into(), state.into());
    e
}

fn changed(at: Stamp, before: Option<Element>, after: Option<Element>, cause: Cause) -> Event {
    Event::Changed {
        at,
        path: "p".into(),
        before,
        after,
        cause,
    }
}

#[test]
fn evt19_changes_across_a_stop_are_not_merged() {
    // 世代 1 で作られた要素が、停止で世代 2 に進んだ後の巻き戻しで消える。時点が違うので 2 件のまま残る。
    let events = vec![
        changed(stamp(1, "main/st-1"), None, Some(el("a")), Cause::Input),
        changed(stamp(2, "main/st-1"), Some(el("a")), None, Cause::Stop),
    ];
    assert_eq!(result_only(&events), events);
}

#[test]
fn evt19_a_change_that_returns_to_its_start_is_dropped() {
    let at = stamp(1, "main/st-1");
    let events = vec![
        changed(at.clone(), Some(el("a")), Some(el("b")), Cause::Input),
        changed(at.clone(), Some(el("b")), Some(el("a")), Cause::Input),
        Event::Accepted { at },
    ];
    let r = result_only(&events);
    assert_eq!(count(&r, |e| matches!(e, Event::Changed { .. })), 0);
    assert_eq!(r.len(), 1);
}
