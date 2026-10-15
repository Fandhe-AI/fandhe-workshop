//! 確認の操作による達成と外れた指し先の試験（TASK-207 2/3、EVT-3 の (b)・EVT-4）。
//!
//! 3 Step の教材（`min-3step.json`）と、写しの `confirm`・`on_achieved`・`next` を差し替えた教材で、
//! 受付済みの Step への入力が確認の宣言に合ったときだけ達成になり、外れたときは受付済みのまま何も変わらない
//! ことを確かめる。受付済みの状態は、進行の照合（TASK-210）が無いため `SessionState` を直接組み立てる。
//! 呼び出し元は CI の `make test` のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use serde_json::json;

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// 指定の Step の宣言を差し替えた教材。
fn with_step(f: impl Fn(&mut fandhe_workshop_contract::ir::Step)) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == "main/st-1" {
            f(&mut n.step);
        }
    }
    ir
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// 指定の Step が指定の状態（処理中でない）にある状態。
fn at(step: &str, status: StepStatus) -> SessionState {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), status);
    let mut run = RunState {
        generation: Generation(2),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn status(s: &SessionState, step: &str) -> StepStatus {
    let u = uid(step);
    s.run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

fn ui(view: &str, path: &str) -> Input {
    Input::Ui {
        view: view.into(),
        path: path.into(),
    }
}

fn approval(decision: Decision) -> Input {
    Input::Approval {
        decision,
        target: None,
    }
}

fn targeted_approval(decision: Decision) -> Input {
    Input::Approval {
        decision,
        target: Some("t-1".into()),
    }
}

fn names(events: &[Event]) -> Vec<&'static str> {
    events
        .iter()
        .map(|e| match e {
            Event::InputReceived { .. } => "input_received",
            Event::Achieved { .. } => "achieved",
            Event::Transitioned { .. } => "transitioned",
            Event::Entered { .. } => "entered",
            Event::Changed { .. } => "changed",
            Event::Flag { .. } => "flag",
            Event::Warning { .. } => "warning",
            Event::ChapterCompleted { .. } => "chapter_completed",
            _ => "other",
        })
        .collect()
}

fn send(ir: &Ir, s: SessionState, input: Input) -> (SessionState, Vec<Event>) {
    apply(ir, s, Op::Input(input))
}

/// 外れ: イベントは `InputReceived` だけで、`run` は前と完全に一致する。
fn assert_missed(ir: &Ir, s: &SessionState, input: Input) {
    let (next, events) = send(ir, s.clone(), input);
    assert_eq!(names(&events), ["input_received"], "{events:?}");
    assert_eq!(next.run, s.run);
}

#[test]
fn point_at_confirms_and_moves_to_next_step() {
    let ir = base_ir();
    let before = at("main/st-1", StepStatus::Accepted);
    let (next, events) = send(&ir, before.clone(), ui("v-files", "notes/draft"));
    assert_eq!(
        names(&events),
        ["input_received", "achieved", "transitioned", "entered"]
    );
    assert!(matches!(
        &events[1],
        Event::Achieved {
            explicit_skip: false,
            ..
        }
    ));
    assert_eq!(status(&next, "main/st-1"), StepStatus::Achieved);
    assert_eq!(status(&next, "main/st-2"), StepStatus::Entered);
    assert_eq!(next.run.current, Some(uid("main/st-2")));
    assert_eq!(next.run.generation, before.run.generation);
    assert_eq!(next.run.world, before.run.world);
    assert_eq!(&next.log[..], events.as_slice());
}

#[test]
fn point_at_miss_keeps_step_accepted() {
    let ir = base_ir();
    let s = at("main/st-1", StepStatus::Accepted);
    assert_missed(&ir, &s, ui("v-files", "notes/other"));
    assert_missed(&ir, &s, ui("v-other", "notes/draft"));
    assert_missed(&ir, &s, Input::Choice("a".into()));
    assert_missed(&ir, &s, Input::Command("/make-note x".into()));
    assert_missed(&ir, &s, approval(Decision::Approve));
}

#[test]
fn no_achievement_before_confirmation_phase() {
    let ir = base_ir();
    // 入力を待つ Step への確認の宛先は、受付の判定に進み、達成にならない。
    let (next, events) = send(
        &ir,
        at("main/st-1", StepStatus::Entered),
        ui("v-files", "notes/draft"),
    );
    assert!(!names(&events).contains(&"achieved"));
    assert_ne!(status(&next, "main/st-1"), StepStatus::Achieved);
    // 処理中の受付済みの Step も達成にならない。
    let mut busy = at("main/st-1", StepStatus::Accepted);
    busy.run.busy = Some(Busy { done: 1, total: 5 });
    assert_missed(&ir, &busy, ui("v-files", "notes/draft"));
    // 達成済みは再び達成しない。
    let done = at("main/st-1", StepStatus::Achieved);
    assert_missed(&ir, &done, ui("v-files", "notes/draft"));
}

#[test]
fn command_accept_then_confirm_are_separate_operations() {
    let ir = base_ir();
    let (s, events) = send(
        &ir,
        at("main/st-1", StepStatus::Entered),
        Input::Command("/make-note first-note".into()),
    );
    assert!(!names(&events).contains(&"achieved"));
    assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
    // 処理が終わった（進行の照合は TASK-210）状態から確認で達成になる。
    let mut s = s;
    s.run.busy = None;
    let (s, events) = send(&ir, s, ui("v-files", "notes/draft"));
    assert!(names(&events).contains(&"achieved"));
    assert_eq!(status(&s, "main/st-1"), StepStatus::Achieved);
}

#[test]
fn approve_confirms_only_on_approve_and_stops_without_next() {
    let ir = base_ir();
    let s = at("main/st-3", StepStatus::Accepted);
    assert_missed(&ir, &s, approval(Decision::Reject));
    assert_missed(&ir, &s, approval(Decision::Revise));
    assert_missed(&ir, &s, ui("v-files", "notes/draft"));
    assert_missed(&ir, &s, targeted_approval(Decision::Approve));
    let (next, events) = send(&ir, s, approval(Decision::Approve));
    assert_eq!(names(&events), ["input_received", "achieved"]);
    assert_eq!(status(&next, "main/st-3"), StepStatus::Achieved);
    assert_eq!(next.run.current, Some(uid("main/st-3")));
    assert!(!next.run.completed);
}

#[test]
fn confirm_none_never_achieves_by_confirmation() {
    let ir = base_ir();
    let s = at("main/st-2", StepStatus::Accepted);
    assert_missed(&ir, &s, approval(Decision::Approve));
    assert_missed(&ir, &s, Input::Choice("a".into()));
    assert_missed(&ir, &s, ui("v-files", "notes/draft"));
}

fn answer_ir() -> Ir {
    with_step(|st| {
        st.confirm = Some(json!({
            "kind": "answer",
            "on_wrong": "rj-wrong",
            "options": [
                {"id": "ok", "correct": true},
                {"id": "ng", "correct": false}
            ]
        }));
    })
}

#[test]
fn answer_confirms_on_correct_option_only() {
    let ir = answer_ir();
    let s = at("main/st-1", StepStatus::Accepted);
    assert_missed(&ir, &s, Input::Choice("ng".into()));
    assert_missed(&ir, &s, Input::Choice("unknown".into()));
    assert_missed(&ir, &s, ui("v-files", "notes/draft"));
    let (next, events) = send(&ir, s, Input::Choice("ok".into()));
    assert_eq!(
        names(&events),
        ["input_received", "achieved", "transitioned", "entered"]
    );
    assert_eq!(status(&next, "main/st-1"), StepStatus::Achieved);
}

#[test]
fn invalid_declarations_warn_and_keep_state() {
    let bad = [
        json!({"kind": "skip"}),
        json!({"kind": "point_at", "targets": []}),
        json!({"kind": "point_at", "targets": [{"view": "v", "path": "p"}], "extra": 1}),
        json!({"kind": "answer", "on_wrong": "w", "options": [{"id": "a", "correct": false}]}),
    ];
    for confirm in bad {
        let ir = with_step(|st| st.confirm = Some(confirm.clone()));
        let s = at("main/st-1", StepStatus::Accepted);
        let (next, events) = send(&ir, s.clone(), ui("v", "p"));
        assert_eq!(names(&events), ["input_received", "warning"], "{confirm}");
        assert_eq!(next.run, s.run);
        if let Event::Warning { message, .. } = &events[1] {
            assert!(!message.contains("skip") && !message.contains("extra"));
        }
    }
}

#[test]
fn missing_confirm_is_treated_as_approve() {
    let ir = with_step(|st| st.confirm = None);
    let s = at("main/st-1", StepStatus::Accepted);
    assert_missed(&ir, &s, ui("v-files", "notes/draft"));
    assert_missed(&ir, &s, targeted_approval(Decision::Approve));
    let (next, _) = send(&ir, s, approval(Decision::Approve));
    assert_eq!(status(&next, "main/st-1"), StepStatus::Achieved);
}

#[test]
fn on_achieved_effects_apply_between_achieved_and_transition() {
    let ir = with_step(|st| st.on_achieved = vec![json!({"set_flag": "done-1"})]);
    let (next, events) = send(
        &ir,
        at("main/st-1", StepStatus::Accepted),
        ui("v-files", "notes/draft"),
    );
    let n = names(&events);
    assert_eq!(n[..2], ["input_received", "achieved"]);
    assert!(n.contains(&"flag"), "{n:?}");
    let pos = |k: &str| n.iter().position(|x| *x == k);
    assert!(pos("flag") < pos("transitioned"));
    assert!(next
        .run
        .slices
        .get(&Root("main".into()))
        .is_some_and(|sl| sl.flags.contains("done-1")));
}

#[test]
fn unreadable_achievement_effects_do_not_achieve() {
    let ir = with_step(|st| {
        st.on_achieved = vec![json!({"set_flag": "ok"}), json!({"bogus": 1})];
    });
    let s = at("main/st-1", StepStatus::Accepted);
    let (next, events) = send(&ir, s.clone(), ui("v-files", "notes/draft"));
    assert_eq!(names(&events), ["input_received", "warning"]);
    assert_eq!(next.run, s.run);
}

#[test]
fn conditional_or_dangling_edges_do_not_transition() {
    let ir = with_step(|st| {
        st.next[0].when = Some(json!({"flag": "x"}));
    });
    let (next, events) = send(
        &ir,
        at("main/st-1", StepStatus::Accepted),
        ui("v-files", "notes/draft"),
    );
    assert_eq!(names(&events), ["input_received", "achieved"]);
    assert_eq!(next.run.current, Some(uid("main/st-1")));

    let ir = with_step(|st| st.next[0].to = "main/nowhere".into());
    let (next, events) = send(
        &ir,
        at("main/st-1", StepStatus::Accepted),
        ui("v-files", "notes/draft"),
    );
    assert_eq!(names(&events), ["input_received", "achieved", "warning"]);
    assert_eq!(status(&next, "main/st-1"), StepStatus::Achieved);
    assert_eq!(next.run.current, Some(uid("main/st-1")));
}

#[test]
fn confirmation_is_deterministic() {
    let ir = base_ir();
    let run = |ir: &Ir| {
        let mut s = at("main/st-1", StepStatus::Accepted);
        let mut all: Vec<Vec<Event>> = Vec::new();
        for i in [
            ui("v-files", "x"),
            ui("v-files", "notes/draft"),
            approval(Decision::Approve),
        ] {
            let (n, e) = send(ir, s, i);
            s = n;
            all.push(e);
        }
        (s, all)
    };
    assert_eq!(run(&ir), run(&ir));
}
