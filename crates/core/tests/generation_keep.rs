//! 世代を変えない操作の試験（TASK-210 1/3、仕様 EVT-6 の (1)(2)(5) と性質テスト P-2 の世代の部分）。
//!
//! 学習者の入力（受付・拒否・確認・達成・再送・取り消し）、宛先つきの進行（`Tick`）、保存（`Control::Save`）は
//! 世代を変えない。`session::apply` の関門が表（`GenerationRule::Keep`）を強制することを、状態の世代・
//! イベントの宛先の世代・世代を運ぶイベントの有無で確かめる。
//! 今の段階では骨格の `Tick`・`Save` に対して自明に通る。宛先の照合（TASK-210 2/3・3/3）と保存（TASK-214）が
//! 分岐を埋めたとき、この試験が回帰の検査として働く。進むか無視かの判定は本試験の範囲外である。
//! 呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::property::{check, op_sequence, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Decision, GenerationRule, Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// st-1 が指定の状態・処理中でない・指定の世代の状態。
fn at(status: StepStatus, generation: Generation) -> SessionState {
    let u = uid("main/st-1");
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), status);
    let mut run = RunState {
        generation,
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "created".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn ui(view: &str, path: &str) -> Input {
    Input::Ui {
        view: view.into(),
        path: path.into(),
    }
}

fn tick(generation: u64, step: &str, index: u32) -> Op {
    Op::Tick(TickAddr {
        generation: Generation(generation),
        step: uid(step),
        index,
    })
}

/// 世代を運ぶイベントか。
fn carries_generation(e: &Event) -> bool {
    matches!(
        e,
        Event::Stopped { .. }
            | Event::Moved { .. }
            | Event::Restored { .. }
            | Event::Begun { .. }
            | Event::Reloaded { .. }
    )
}

/// イベントの宛先 `at` の世代（持たないイベントは `None`）。
fn stamp_generation(e: &Event) -> Option<u64> {
    let v = serde_json::to_value(e).expect("serialize event");
    v.as_object()?
        .values()
        .next()?
        .get("at")?
        .get("generation")?
        .as_u64()
}

/// 操作を 1 件送り、世代が変わらず、宛先の世代が操作前のままで、世代を運ぶイベントが無いことを確かめる。
fn assert_keeps(session: &mut Session, op: Op) -> Vec<Event> {
    assert_eq!(op.generation_rule(), GenerationRule::Keep, "{op:?}");
    let before = session.generation();
    let events = session.apply(op.clone()).to_vec();
    assert_eq!(session.generation(), before, "{op:?}");
    for e in &events {
        assert!(!carries_generation(e), "{op:?}: {e:?}");
        if let Some(g) = stamp_generation(e) {
            assert_eq!(g, before.0, "{op:?}: {e:?}");
        }
    }
    events
}

fn session_at(status: StepStatus, generation: Generation) -> Session {
    Session::from_state(base_ir(), at(status, generation))
}

#[test]
fn keep_on_accept() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    let events = assert_keeps(
        &mut s,
        Op::Input(Input::Command("/make-note first-note".into())),
    );
    assert!(events.iter().any(|e| matches!(e, Event::Accepted { .. })));
}

#[test]
fn keep_on_refuse() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    let events = assert_keeps(&mut s, Op::Input(Input::Command("/nothing".into())));
    assert!(!events.iter().any(|e| matches!(e, Event::Accepted { .. })));
}

#[test]
fn keep_on_confirm_achieve() {
    let mut s = session_at(StepStatus::Accepted, Generation(1));
    let events = assert_keeps(&mut s, Op::Input(ui("v-files", "notes/draft")));
    assert!(events.iter().any(|e| matches!(e, Event::Achieved { .. })));
}

#[test]
fn keep_on_resend_burst() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    for _ in 0..20 {
        assert_keeps(
            &mut s,
            Op::Input(Input::Command("/make-note first-note".into())),
        );
    }
    assert_eq!(s.generation(), Generation(1));
}

#[test]
fn keep_on_cancel() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    assert_keeps(&mut s, Op::Input(Input::Cancel));
}

#[test]
fn keep_on_tick_any_addr() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    for op in [
        tick(1, "main/st-1", 0),
        tick(0, "main/st-1", 0),
        tick(2, "main/st-1", 0),
        tick(1, "main/st-2", 0),
        tick(1, "main/st-1", 3),
    ] {
        assert_keeps(&mut s, op);
    }
}

#[test]
fn keep_on_save() {
    let mut s = session_at(StepStatus::Entered, Generation(1));
    assert_keeps(&mut s, Op::Control(Control::Save));
}

#[test]
fn keep_at_generation_max() {
    let max = Generation(u64::MAX);
    let ops = [
        Op::Input(Input::Command("/make-note first-note".into())),
        Op::Input(Input::Command("/nothing".into())),
        Op::Input(Input::Cancel),
        tick(u64::MAX, "main/st-1", 0),
        Op::Control(Control::Save),
    ];
    for op in ops {
        let mut s = session_at(StepStatus::Entered, max);
        assert_keeps(&mut s, op);
        assert_eq!(s.generation(), max);
    }
    let mut s = session_at(StepStatus::Accepted, max);
    assert_keeps(&mut s, Op::Input(ui("v-files", "notes/draft")));
}

fn keep_op_strategy() -> impl Strategy<Value = Op> {
    let steps = prop_oneof![Just("main/st-1"), Just("main/st-2"), Just("main/st-3")];
    prop_oneof![
        4 => Just(Op::Input(Input::Command("/make-note first-note".into()))),
        1 => Just(Op::Input(Input::Command("/make-note Bad_Name".into()))),
        1 => Just(Op::Input(Input::Command("/nothing".into()))),
        2 => Just(Op::Input(Input::Choice("opt-memo".into()))),
        2 => Just(Op::Input(Input::Choice("opt-ghost".into()))),
        3 => Just(Op::Input(ui("v-files", "notes/draft"))),
        1 => Just(Op::Input(ui("v-other", "notes/other"))),
        3 => Just(Op::Input(Input::Approval { decision: Decision::Approve, target: None })),
        1 => Just(Op::Input(Input::Approval { decision: Decision::Reject, target: None })),
        1 => Just(Op::Input(Input::Text("文章".into()))),
        2 => Just(Op::Input(Input::Cancel)),
        3 => (0u64..3, steps, 0u32..4).prop_map(|(g, s, i)| tick(g, s, i)),
        1 => Just(Op::Control(Control::Save)),
    ]
}

#[test]
fn p2_keep_ops_never_change_generation() {
    check(
        "p2-keep-ops-never-change-generation",
        DEFAULT_CASES,
        op_sequence(keep_op_strategy()),
        |ops| {
            let mut s = session_at(StepStatus::Entered, Generation(1));
            for (n, op) in ops.into_iter().enumerate() {
                prop_assert_eq!(op.generation_rule(), GenerationRule::Keep, "op#{}", n);
                let before = s.generation();
                let events = s.apply(op).to_vec();
                prop_assert_eq!(s.generation(), before, "op#{}", n);
                prop_assert_eq!(s.generation(), Generation(1), "op#{}", n);
                for e in &events {
                    prop_assert!(!carries_generation(e), "op#{} {:?}", n, e);
                    if let Some(g) = stamp_generation(e) {
                        prop_assert_eq!(g, before.0, "op#{} {:?}", n, e);
                    }
                }
            }
            Ok(())
        },
    );
}
