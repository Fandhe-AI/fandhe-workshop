//! 途中開始で飛ばした Step の省略の試験（TASK-217 1/2、EVT-21・MOD-6）。
//!
//! 3 Step の教材（`min-3step.json`）を 5 Step に増やした IR で、開始の節点より前の Step が `Skipped` になり、
//! 達成にならないこと、別の章・別の実体・達成済みの Step・呼び出しの Step・拒否された開始では変わらないことを確かめる。
//! 件数の保証と再読み込み・書き出しをまたぐ保持は TASK-217 2/2（#556）が確かめる。
//! 実体ごとの途中開始は `crates/pack/tests/start_skip.rs` で確かめる。

mod common;

use common::min_ir;
use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::status_of;
use fandhe_workshop_core::session::{apply, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;

/// 5 Step の章。`main/st-1`〜`main/st-5` はすべて `ch-min`・`main`。開始点 `s-<n>` は `main/st-<n>`。
fn ir5() -> Ir {
    let mut ir = min_ir();
    let proto = ir.nodes[2].clone();
    for n in 4..=5 {
        let mut node = proto.clone();
        node.uid = format!("main/st-{n}");
        node.step.id = format!("st-{n}");
        ir.nodes.push(node);
    }
    ir.starts = (1..=5)
        .map(|n| Start {
            id: format!("s-{n}"),
            node: format!("main/st-{n}"),
            effects: vec![],
            guidance: None,
            requires: None,
            unmet: None,
        })
        .collect();
    ir
}

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

fn uid(n: usize) -> StepUid {
    StepUid(format!("main/st-{n}"))
}

fn skipped_steps(events: &[Event]) -> Vec<StepUid> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn midstart_skips_earlier_steps_in_ir_order() {
    let ir = ir5();
    let (state, events) = apply(&ir, SessionState::default(), begin("s-4"));
    for n in 1..=3 {
        assert_eq!(status_of(&ir, &state.run, &uid(n)), StepStatus::Skipped);
    }
    assert_eq!(status_of(&ir, &state.run, &uid(4)), StepStatus::Entered);
    assert_eq!(status_of(&ir, &state.run, &uid(5)), StepStatus::NotStarted);
    assert_eq!(skipped_steps(&events), vec![uid(1), uid(2), uid(3)]);
    let Some(Event::Begun { new_generation, .. }) = events.first() else {
        panic!("begun first: {events:?}");
    };
    for e in &events {
        if let Event::Skipped { at } = e {
            assert_eq!(at.generation, *new_generation);
        }
    }
    let first_entered = events
        .iter()
        .position(|e| matches!(e, Event::Entered { .. }));
    let last_skipped = events
        .iter()
        .rposition(|e| matches!(e, Event::Skipped { .. }));
    assert!(last_skipped < first_entered, "{events:?}");
}

#[test]
fn midstart_at_the_first_step_skips_nothing() {
    let ir = ir5();
    let (state, events) = apply(&ir, SessionState::default(), begin("s-1"));
    assert!(skipped_steps(&events).is_empty());
    assert_eq!(status_of(&ir, &state.run, &uid(1)), StepStatus::Entered);
}

#[test]
fn midstart_keeps_achieved_steps() {
    let ir = ir5();
    let mut state = SessionState::default();
    state
        .run
        .slices
        .entry(uid(1).root())
        .or_default()
        .status
        .insert(uid(1), StepStatus::Achieved);
    let (state, events) = apply(&ir, state, begin("s-4"));
    assert_eq!(status_of(&ir, &state.run, &uid(1)), StepStatus::Achieved);
    assert_eq!(skipped_steps(&events), vec![uid(2), uid(3)]);
}

#[test]
fn midstart_leaves_other_units_and_scopes_alone() {
    let mut ir = ir5();
    ir.nodes[0].unit = "ch-other".into();
    ir.nodes[1].scope = "main@round0".into();
    let (state, events) = apply(&ir, SessionState::default(), begin("s-4"));
    assert_eq!(skipped_steps(&events), vec![uid(3)]);
    assert_eq!(status_of(&ir, &state.run, &uid(1)), StepStatus::NotStarted);
    assert_eq!(status_of(&ir, &state.run, &uid(2)), StepStatus::NotStarted);
    assert_eq!(status_of(&ir, &state.run, &uid(3)), StepStatus::Skipped);
}

#[test]
fn midstart_does_not_skip_call_steps() {
    let mut ir = ir5();
    ir.nodes[1].step.use_module = Some(serde_json::json!({"module": "m"}));
    let (_, events) = apply(&ir, SessionState::default(), begin("s-4"));
    assert_eq!(skipped_steps(&events), vec![uid(1), uid(3)]);
}

#[test]
fn refused_begin_skips_nothing() {
    let run = SessionState::default().run;
    let ir = ir5();
    let (state, events) = apply(&ir, SessionState::default(), begin("nope"));
    assert_eq!(state.run, run);
    assert!(skipped_steps(&events).is_empty());

    let mut ir = ir5();
    ir.starts[3].effects = vec![serde_json::json!({"unknown_effect": 1})];
    let (state, events) = apply(&ir, SessionState::default(), begin("s-4"));
    assert_eq!(state.run, run);
    assert!(skipped_steps(&events).is_empty());

    let mut ir = ir5();
    ir.starts[3].requires = Some(serde_json::json!({"flag": "never-set"}));
    let (state, events) = apply(&ir, SessionState::default(), begin("s-4"));
    assert_eq!(state.run, run);
    assert!(skipped_steps(&events).is_empty());
}

#[test]
fn two_entrances_agree_on_skips() {
    let ir = ir5();
    let (a, ev_a) = apply(&ir, SessionState::default(), begin("s-4"));
    let mut s = Session::new(ir.clone());
    let ev_s = s.apply(begin("s-4")).to_vec();
    assert_eq!(ev_a, ev_s);
    assert_eq!(&a, s.state());
    assert_eq!(skipped_steps(&ev_a).len(), 3);
}

#[test]
fn midstart_halted_by_transition_limit_keeps_no_skips() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-self.json",
    );
    let ir = fandhe_workshop_core::load_ir(&std::fs::read(p).expect("read")).expect("load");
    let (state, events) = apply(&ir, SessionState::default(), begin("s-enter-self"));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. })));
    assert!(skipped_steps(&events).is_empty(), "{events:?}");
    assert!(skipped_steps(&state.log).is_empty());
    for n in 1..=3 {
        assert_eq!(status_of(&ir, &state.run, &uid(n)), StepStatus::NotStarted);
    }
}
