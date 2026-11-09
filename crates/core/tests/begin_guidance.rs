//! 開始点が決める案内レベルの試験（TASK-294 1/2、GUIDE-3・GUIDE-13）。
//!
//! `Control::Begin` が `starts[].guidance` で実行全体の案内レベルを決めること、宣言が無いときは新しい状態で
//! `pack.guidance`・開始済みでは今の水準のままであること、読めない値と拒否・上限の停止では水準を変えないことを確かめる。
//! 変異は `GUIDE3-1`〜`GUIDE3-3`。`SetGuidance` の反映と判定の不変の性質は TASK-294 2/2（`set_guidance.rs`・`guidance_judge_invariance.rs`）。

mod common;

use common::min_ir;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::{apply, SessionState};
use serde_json::{json, Value};

fn start(n: usize, guidance: Option<Value>) -> Start {
    Start {
        id: format!("s-{n}"),
        node: format!("main/st-{n}"),
        effects: vec![],
        guidance,
        requires: None,
        unmet: None,
    }
}

/// 3 Step の教材。開始点 `s-1`〜`s-3` に宣言を付ける。
fn ir3(g: [Option<Value>; 3]) -> Ir {
    let mut ir = min_ir();
    let [g1, g2, g3] = g;
    ir.starts = vec![start(1, g1), start(2, g2), start(3, g3)];
    ir
}

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

fn level(s: &SessionState) -> GuidanceLevel {
    s.run.guidance_level
}

#[test]
fn begin_takes_the_declared_level() {
    for (text, want) in [
        ("A", GuidanceLevel::A),
        ("B", GuidanceLevel::B),
        ("C", GuidanceLevel::C),
    ] {
        let ir = ir3([Some(json!(text)), None, None]);
        let (state, _) = apply(&ir, SessionState::default(), begin("s-1"));
        assert_eq!(level(&state), want);
    }
}

#[test]
fn begin_without_declaration_on_fresh_state_uses_pack_guidance() {
    let mut ir = ir3([None, None, None]);
    ir.pack.guidance = "C".to_string();
    let (state, _) = apply(&ir, SessionState::default(), begin("s-1"));
    assert_eq!(level(&state), GuidanceLevel::C);
}

#[test]
fn begin_without_declaration_keeps_current_level() {
    let ir = ir3([Some(json!("B")), None, None]);
    let (state, _) = apply(&ir, SessionState::default(), begin("s-1"));
    let (state, _) = apply(&ir, state, begin("s-2"));
    assert_eq!(level(&state), GuidanceLevel::B);
}

#[test]
fn switching_rounds_changes_level_a_to_b() {
    let ir = ir3([Some(json!("A")), Some(json!("B")), None]);
    let (state, _) = apply(&ir, SessionState::default(), begin("s-1"));
    assert_eq!(level(&state), GuidanceLevel::A);
    let (state, _) = apply(&ir, state, begin("s-2"));
    assert_eq!(level(&state), GuidanceLevel::B);
}

#[test]
fn unreadable_start_guidance_is_refused_without_change() {
    for bad in [
        json!("Z"),
        json!("b"),
        json!(1),
        Value::Null,
        json!([]),
        json!({}),
    ] {
        let ir = ir3([Some(bad.clone()), None, None]);
        let before = SessionState::default();
        let (state, events) = apply(&ir, before.clone(), begin("s-1"));
        assert_eq!(state.run, before.run, "{bad}");
        let [Event::Warning { message, .. }] = events.as_slice() else {
            panic!("one warning expected: {events:?}");
        };
        assert!(!message.contains('Z'), "{message}");
    }
}

#[test]
fn unreadable_pack_guidance_on_fresh_begin_is_refused() {
    let mut ir = ir3([None, Some(json!("B")), None]);
    ir.pack.guidance = "Z".to_string();
    let before = SessionState::default();
    let (state, events) = apply(&ir, before.clone(), begin("s-1"));
    assert_eq!(state.run, before.run);
    assert!(matches!(events.as_slice(), [Event::Warning { .. }]));
    // 宣言があれば `pack.guidance` は読まない
    let (state, _) = apply(&ir, SessionState::default(), begin("s-2"));
    assert_eq!(level(&state), GuidanceLevel::B);
}

#[test]
fn refused_begin_keeps_level() {
    let mut ir = ir3([Some(json!("B")), Some(json!("C")), None]);
    let (state, _) = apply(&ir, SessionState::default(), begin("s-1"));
    let (after, _) = apply(&ir, state.clone(), begin("nope"));
    assert_eq!(level(&after), GuidanceLevel::B);
    ir.starts[1].requires = Some(json!({"flag": "never-set"}));
    let (after, _) = apply(&ir, state, begin("s-2"));
    assert_eq!(level(&after), GuidanceLevel::B);
}

#[test]
fn transition_limit_reverts_level() {
    let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(
        "../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/cycle-enter-self.json",
    );
    let mut ir = fandhe_workshop_core::load_ir(&std::fs::read(p).expect("read")).expect("load");
    for s in &mut ir.starts {
        s.guidance = Some(json!("C"));
    }
    let (state, events) = apply(&ir, SessionState::default(), begin("s-enter-self"));
    assert!(events
        .iter()
        .any(|e| matches!(e, Event::TransitionLimitReached { .. })));
    assert_eq!(level(&state), GuidanceLevel::A);
}

#[test]
fn level_is_reproduced_by_replaying_ops() {
    let ir = ir3([Some(json!("A")), Some(json!("B")), None]);
    let mut s = fandhe_workshop_core::Session::new(ir.clone());
    s.apply(begin("s-1"));
    s.apply(begin("s-2"));
    assert_eq!(level(s.state()), GuidanceLevel::B);
    // 水準はイベントでなく `Begin` の再適用で再現する（D5）
    let mut again = SessionState::default();
    for op in [begin("s-1"), begin("s-2")] {
        again = apply(&ir, again, op).0;
    }
    assert_eq!(level(&again), GuidanceLevel::B);
}
