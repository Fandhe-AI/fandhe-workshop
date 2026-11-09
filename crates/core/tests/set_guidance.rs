//! 受講者の選択 `Control::SetGuidance` の反映の試験（TASK-294 2/2、GUIDE-3）。
//!
//! 水準が指定どおりになりイベントが 0 件であること、ヒントの数に効くこと、開始前の選択と `Begin` の関係、
//! 処理中の停止は選択を保ち復帰は写しの水準へ戻すこと（MOD-9）、再読み込みと適用し直しで保たれることを確かめる。
//! 判定が水準によらないことの性質テストは `guidance_judge_invariance.rs`。変異は `GUIDE3-4`・`GUIDE3-6`。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::path::Path;

use common::contract_sample::{load, testdata_dir};
use common::min_ir;
use common::tick::next_tick;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::history_replay::{first_difference, replay};
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::{apply_recorded, reload_recorded, OpsRecorder};
use fandhe_workshop_core::session::{apply, Session, SessionState};
use serde_json::{json, Value};

fn set(l: GuidanceLevel) -> Op {
    Op::Control(Control::SetGuidance(l))
}

fn sample_ir() -> Ir {
    let s = load();
    load_ir(&std::fs::read(testdata_dir().join(&s.ir)).unwrap()).unwrap()
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた処理中の状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    let note = Op::Input(Input::Command("/make-note first-note".into()));
    let mut state = apply(ir, load().initial, note).0;
    for _ in 0..ticks {
        let addr = next_tick(&state).expect("next tick");
        state = apply(ir, state, Op::Tick(addr)).0;
    }
    assert!(state.run.busy.is_some());
    state
}

/// 水準以外の全欄が同じことを確かめる。
fn same_but_level(a: &SessionState, b: &SessionState) {
    let mut a = a.clone();
    a.run.guidance_level = b.run.guidance_level;
    assert_eq!(&a, b);
}

#[test]
fn set_guidance_sets_the_level() {
    let ir = sample_ir();
    let mut starts: Vec<(&str, SessionState)> = vec![
        ("fresh", SessionState::default()),
        ("initial", load().initial),
        ("busy", busy_state(&ir, 2)),
    ];
    let mut done = load().initial;
    done.run.completed = true;
    starts.push(("completed", done));
    for (name, state) in starts {
        for l in GuidanceLevel::ALL {
            let (next, events) = apply(&ir, state.clone(), set(l));
            assert!(events.is_empty(), "{name}: no events");
            assert_eq!(next.run.guidance_level, l, "{name}");
            same_but_level(&state, &next);
        }
    }
}

#[test]
fn set_guidance_changes_visible_hints() {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    // 水準 A だけ 3 本、B と C は 1 本の Step にする。
    v["hints"]["h-a2"] = json!({"levels": ["A"], "text": "A2"});
    v["nodes"][0]["step"]["hints"] = json!(["h-all", "h-a", "h-a2"]);
    let ir: Ir = serde_json::from_value(v).unwrap();
    let mut state = Session::entered_at(ir.clone(), "main/s1").state().clone();
    assert_eq!(state.run.guidance_level, GuidanceLevel::A);
    let mut counts = Vec::new();
    for l in [GuidanceLevel::A, GuidanceLevel::B, GuidanceLevel::C] {
        state = apply(&ir, state, set(l)).0;
        counts.push(current_hints(&ir, &state.run).len());
    }
    assert_eq!(counts, vec![3, 1, 1]);
    // 上限は選んだ水準の本数に従う（C は 1 本）。
    let open = || Op::Control(Control::OpenHint);
    state = apply(&ir, state, open()).0;
    let (_, events) = apply(&ir, state, open());
    assert!(matches!(events.as_slice(), [Event::Refused { .. }]));
}

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

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

#[test]
fn set_guidance_before_begin() {
    let mut ir = min_ir();
    ir.starts = vec![start(1, None), start(2, Some(json!("B"))), start(3, None)];
    ir.pack.guidance = "A".to_string();
    // 宣言の無い開始点を新しい状態から開始すると pack.guidance が勝つ。
    let s = apply(&ir, SessionState::default(), set(GuidanceLevel::C)).0;
    let (s, _) = apply(&ir, s, begin("s-1"));
    assert_eq!(s.run.guidance_level, GuidanceLevel::A);
    // 宣言のある開始点では宣言どおり。
    let s = apply(&ir, SessionState::default(), set(GuidanceLevel::C)).0;
    let (s, _) = apply(&ir, s, begin("s-2"));
    assert_eq!(s.run.guidance_level, GuidanceLevel::B);
    // 開始済みで宣言の無い開始点へ開始すると選んだ水準が続く。
    let (s, _) = apply(&ir, SessionState::default(), begin("s-2"));
    let s = apply(&ir, s, set(GuidanceLevel::C)).0;
    let (s, _) = apply(&ir, s, begin("s-3"));
    assert_eq!(s.run.guidance_level, GuidanceLevel::C);
}

#[test]
fn stop_keeps_the_selected_level() {
    let ir = sample_ir();
    let stop = || Op::Control(Control::Stop);
    // 通常の処理中の選択。
    let s = apply(&ir, busy_state(&ir, 2), set(GuidanceLevel::C)).0;
    let (s, events) = apply(&ir, s, stop());
    assert!(matches!(events.first(), Some(Event::Stopped { .. })));
    assert_eq!(s.run.guidance_level, GuidanceLevel::C);
    // 復帰の直後（写しの起点がチェックポイントの水準 A）でも選択を保つ。
    let s = busy_state(&ir, 1);
    let (s, _) = apply(&ir, s, Op::Control(Control::Save));
    let (s, _) = apply(&ir, s, Op::Control(Control::Restore(CheckpointId(1))));
    let s = apply(&ir, s, set(GuidanceLevel::C)).0;
    let note = Op::Input(Input::Command("/make-note first-note".into()));
    let s = apply(&ir, s, note).0;
    assert!(s.run.busy.is_some());
    let (s, events) = apply(&ir, s, stop());
    assert!(matches!(events.first(), Some(Event::Stopped { .. })));
    assert_eq!(s.run.guidance_level, GuidanceLevel::C);
}

#[test]
fn restore_reverts_to_the_saved_level() {
    let ir = sample_ir();
    let s = load().initial;
    assert_eq!(s.run.guidance_level, GuidanceLevel::A);
    let (s, _) = apply(&ir, s, Op::Control(Control::Save));
    let s = apply(&ir, s, set(GuidanceLevel::C)).0;
    let (s, events) = apply(&ir, s, Op::Control(Control::Restore(CheckpointId(1))));
    assert!(events.iter().any(|e| matches!(e, Event::Restored { .. })));
    assert_eq!(s.run.guidance_level, GuidanceLevel::A);
}

#[test]
fn level_survives_reload_and_replay() {
    let ir = sample_ir();
    let initial = load().initial;
    let mut rec = OpsRecorder::new();
    let (s, _) = apply_recorded(&ir, initial.clone(), &mut rec, set(GuidanceLevel::B));
    let note = Op::Input(Input::Command("/make-note first-note".into()));
    let (s, _) = apply_recorded(&ir, s, &mut rec, note);
    let (s, _) = apply_recorded(&ir, s, &mut rec, set(GuidanceLevel::C));
    let (s, _) = reload_recorded(s, &mut rec);
    assert_eq!(s.run.guidance_level, GuidanceLevel::C);
    let replayed = replay(&ir, initial, rec.ops()).expect("replay");
    assert_eq!(first_difference(&s, &replayed), None);
    let json = serde_json::to_string(rec.ops()).unwrap();
    assert!(json.contains("set_guidance"), "recorded as an op");
    // Session の経路でも保たれる。
    let mut session = Session::from_state(ir, load().initial);
    session.apply(set(GuidanceLevel::B));
    session.reload();
    assert_eq!(session.state().run.guidance_level, GuidanceLevel::B);
}
