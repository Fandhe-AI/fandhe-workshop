//! 受付と確認の操作による達成の境界の通し試験（TASK-306 1/2、R-22・EVT-3・JUDGE-7）。
//!
//! 「コマンドを受け付けた」と「確認の操作で達成になる」は別の事象である。本試験は、受付だけ・確認だけの段階で
//! 達成の記録（`Event::Achieved`・`StepStatus::Achieved`・章の完了）が 0 件であることを、入力の受付
//! （`session::apply`）・受け付けて止まる場面（`command_accept`、JUDGE-7）・学習目標の確認の記録
//! （contract の `derive_records` と `lo_first_answer`、TASK-291・TASK-305）をつないで確かめる。
//! 確認の操作による達成の遷移は TASK-207 2/3、`confirm: none` の受付と同時の達成は TASK-208 の担当で、
//! それらが入った後も本試験が崩れないよう、`none` と「確認の操作が達成にする」側は断定しない。
//! 呼び出し元は CI の `make test` のみである。

use std::collections::BTreeMap;
use std::path::PathBuf;

use fandhe_workshop_contract::confirm_spec::{achievement, Achievement, ConfirmKind};
use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckMethod, CheckObservation, CheckTarget,
};
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::command_accept::{accept_command, accepted_command};
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::command_rule::{
    judge_command_step, CommandRuleVerdict, CommandStepVerdict,
};
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::lo_first_answer::tally;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// 全 Step の `confirm` を `kind` にした教材。st-2 の `none` を使わないために置き換える。
fn ir_with_confirm(kind: ConfirmKind) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        n.step.confirm = Some(json!({ "kind": kind.as_str() }));
    }
    ir
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn entered(step: &str) -> SessionState {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
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

fn status(s: &Session, step: &str) -> StepStatus {
    let u = uid(step);
    s.state()
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

fn achieved_events(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. } | Event::ChapterCompleted { .. }))
        .count()
}

/// 達成の記録（イベント・Step の状態・章の完了）の合計。0 が期待値である。
fn achievement_records(s: &Session) -> usize {
    let statuses = s
        .state()
        .run
        .slices
        .values()
        .flat_map(|sl| sl.status.values())
        .filter(|st| **st == StepStatus::Achieved)
        .count();
    achieved_events(s.log()) + statuses + usize::from(s.state().run.completed)
}

fn cmd(s: &str) -> Input {
    Input::Command(s.to_string())
}

fn approve() -> Input {
    Input::Approval {
        decision: Decision::Approve,
        target: None,
    }
}

#[test]
fn accept_alone_never_achieves_for_every_confirming_kind() {
    for kind in ConfirmKind::ALL {
        // 受付と同時の達成は `none` だけ（TASK-208）。確認を要する 3 種で、受付だけの段階の 0 件を確かめる。
        if achievement(Some(kind)) == Achievement::OnAccept {
            continue;
        }
        assert_eq!(achievement(Some(kind)), Achievement::AfterConfirm(kind));
        let mut s = Session::from_state(ir_with_confirm(kind), entered("main/st-1"));
        s.apply(Op::Input(cmd("/make-note first-note")));
        assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted, "{kind:?}");
        assert_eq!(achievement_records(&s), 0, "{kind:?}");
    }
}

#[test]
fn missing_confirm_declaration_is_treated_as_needing_confirm() {
    // 確認の宣言が無い Step は、受付と同時に達成にしない（R-22 の安全側）。
    assert_eq!(
        achievement(None),
        Achievement::AfterConfirm(ConfirmKind::Approve)
    );
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        n.step.confirm = None;
    }
    let mut s = Session::from_state(ir, entered("main/st-1"));
    s.apply(Op::Input(cmd("/make-note first-note")));
    assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
    assert_eq!(achievement_records(&s), 0);
}

#[test]
fn confirmation_style_inputs_without_acceptance_do_not_achieve() {
    // 受付の前に確認の操作（承認・選択・UI 操作）だけが届いても、Step は入力待ちのままで達成にならない。
    let confirms = [
        approve(),
        Input::Approval {
            decision: Decision::Revise,
            target: Some("notes/draft".into()),
        },
        Input::Choice("opt-memo".into()),
        Input::Ui {
            view: "v-files".into(),
            path: "notes/draft".into(),
        },
    ];
    let mut s = Session::from_state(ir_with_confirm(ConfirmKind::Approve), entered("main/st-1"));
    for input in confirms {
        s.apply(Op::Input(input));
        assert_eq!(status(&s, "main/st-1"), StepStatus::Entered);
        assert_eq!(achievement_records(&s), 0);
    }
}

#[test]
fn every_input_kind_in_any_step_yields_zero_achievements() {
    let inputs = || {
        vec![
            cmd("/make-note first-note"),
            cmd("/make-note Bad_Name"),
            cmd("/nothing"),
            Input::Choice("opt-memo".into()),
            Input::Choice("opt-ghost".into()),
            approve(),
            Input::Text("文章".into()),
            Input::Cancel,
        ]
    };
    for kind in [
        ConfirmKind::PointAt,
        ConfirmKind::Answer,
        ConfirmKind::Approve,
    ] {
        for step in ["main/st-1", "main/st-2", "main/st-3"] {
            let mut s = Session::from_state(ir_with_confirm(kind), entered(step));
            // 同じ列を 2 回流し、受付済みへの再入力も含める。
            for input in inputs().into_iter().chain(inputs()) {
                s.apply(Op::Input(input));
                assert_eq!(achievement_records(&s), 0, "{kind:?} {step}");
            }
        }
    }
}

/// 受け付けて止まる場面（JUDGE-7）の Step。`module-reuse-rounds` の `st-7-3` の写し。
fn stop_scene_step() -> Step {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/command_accept_1/steps.json"
    );
    let mut all: BTreeMap<String, Step> =
        serde_json::from_str(&std::fs::read_to_string(path).expect("read")).expect("parse");
    all.remove("st-7-3").expect("st-7-3")
}

#[test]
fn stop_scene_stays_accepted_and_never_achieves() {
    let step = stop_scene_step();
    let lexed = lex_command("/", "/implement-issue-tree 42").expect("lex");
    let never = |_: &Value| None;
    let CommandStepVerdict::Rule(verdict) = judge_command_step(&step, &lexed, &never).expect("ok")
    else {
        panic!("rule verdict expected");
    };
    assert!(matches!(verdict, CommandRuleVerdict::Accepted { .. }));
    let acc = accepted_command(&step, &verdict).expect("effects");

    // 止まった状態を世界状態に持つ実行環境へ受付を適用する。
    let mut state = entered("main/st-1");
    state.run.world.insert(
        "issues/root-42".into(),
        Element {
            fixture: None,
            attrs: BTreeMap::from([("state".to_string(), "running".to_string())]),
        },
    );
    let at = Stamp {
        generation: Generation(1),
        step: uid("main/st-1"),
    };
    let events = accept_command(&mut state.run, &uid("main/st-1"), &acc, &at, &NoConditions);
    assert_eq!(achieved_events(&events), 0);
    assert_eq!(state.run.world["issues/root-42"].attrs["state"], "blocked");
    assert!(!state.run.completed);

    // 止まったまま、その Step へ入力が続いても受付のままで、達成にならない。
    let mut s = Session::from_state(ir_with_confirm(ConfirmKind::Approve), state);
    assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
    for input in [
        cmd("/implement-issue-tree 42"),
        cmd("/make-note first-note"),
    ] {
        s.apply(Op::Input(input));
        assert_eq!(status(&s, "main/st-1"), StepStatus::Accepted);
        assert_eq!(achievement_records(&s), 0);
    }
    assert_eq!(
        s.state().run.world["issues/root-42"].attrs["state"],
        "blocked"
    );
}

/// 確認の回答（選択）を `Session` へ流し、`Answered` の観測にする。
/// 受付（`Event::Accepted`）を正答の代わりに使う（選べる語が正答、未宣言の語は誤答）。
fn answer(s: &mut Session, choice: &str) -> CheckObservation {
    let events = s
        .apply(Op::Input(Input::Choice(choice.to_string())))
        .to_vec();
    CheckObservation::Answered {
        step: "main/st-2".into(),
        correct: events.iter().any(|e| matches!(e, Event::Accepted { .. })),
    }
}

fn lo_targets() -> Vec<CheckTarget> {
    vec![CheckTarget {
        step: "main/st-2".into(),
        lo: vec!["lo-2".into()],
        method: CheckMethod::Answer,
    }]
}

#[test]
fn first_answer_correct_counts_as_self_reliant_but_is_not_achievement() {
    let mut s = Session::from_state(ir_with_confirm(ConfirmKind::Answer), entered("main/st-2"));
    let obs = vec![answer(&mut s, "opt-memo")];
    let t = tally(&derive_records(&lo_targets(), &obs), "lo-2").expect("valid");
    assert_eq!((t.self_reliant, t.reselected_without_comparison), (1, 0));
    // 自力の正答でも、確認の操作を経るまで達成の記録は増えない。
    assert_eq!(status(&s, "main/st-2"), StepStatus::Accepted);
    assert_eq!(achievement_records(&s), 0);
}

#[test]
fn reselection_is_not_self_reliant_and_not_achievement() {
    let mut s = Session::from_state(ir_with_confirm(ConfirmKind::Answer), entered("main/st-2"));
    let wrong = answer(&mut s, "opt-ghost");
    assert_eq!(status(&s, "main/st-2"), StepStatus::Entered);
    assert_eq!(achievement_records(&s), 0);
    let right = answer(&mut s, "opt-memo");
    let obs = vec![wrong, right];
    let t = tally(&derive_records(&lo_targets(), &obs), "lo-2").expect("valid");
    assert_eq!((t.self_reliant, t.reselected_without_comparison), (0, 1));
    assert_eq!(status(&s, "main/st-2"), StepStatus::Accepted);
    assert_eq!(achievement_records(&s), 0);
}
