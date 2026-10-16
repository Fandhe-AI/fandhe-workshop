//! 確認前の達成が 0 件であることの性質テスト（TASK-207 3/3、仕様 EVT-3 の P-3）。
//!
//! 受付（`Accepted`）と確認の操作による達成（`Achieved`）は別の事象である。乱数の操作列を
//! `Session::apply` で流し、操作のたびに次の不変条件を検査する。違反の件数が 0 でなければ失敗する。
//!
//! - 確認を省くと明示した Step は `confirm.kind` が `none` の Step だけである。それ以外
//!   （`confirm` が無い Step を含む。R-22 の安全側）を「確認を要する Step」と呼ぶ
//! - I1: 確認を要する Step の `Achieved{explicit_skip: false}` は、操作の前に `Accepted` で処理中でない
//!   Step にだけ出る
//! - I2: 同じ操作の中で、その Step の `Accepted` と `Achieved` は同時に出ない（受付と達成は別の操作）
//! - I3: `Achieved` の前のログに、同じ Step・同じ世代の `Accepted` がある
//! - I4: 操作の後で `Achieved` になっている確認を要する Step には、ログに `Achieved` がある
//!
//! `none` の Step は検査の対象外にする（受付と同時の達成は `confirm_none_achieve`、TASK-208）。
//! 確認を要する Step の `explicit_skip: true` は現状の経路では出ない。出た場合も同じ規則で検査し、
//! 学習者による明示の省略を導入するタスクが本試験を見直す。
//!
//! 擬似操作 `FinishProcessing` は、処理中（`run.busy`）を畳む代わりである。本物の進行（`Op::Tick`、TASK-210 2/3）は
//! 一致の宛先で処理を進めるが、本試験の `Tick`（index 0 固定）は残し、擬似操作の置き換えは見送った
//! （進行の試験は `tick_progress.rs`）。
//! `Op::Control` は列に入れない。復帰（TASK-214）・移動（TASK-213）などはログ・状態を巻き戻す経路を作り、
//! I3・I4 の前提が変わるため、P-3 との関係は各タスクで扱う。
//! 網羅性（3 種の確認すべてを通ること）は、性質テストでなく固定列の試験で確かめる。
//! 呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::property::{check, op_sequence, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;
use serde_json::{json, Value};

const STEPS: [&str; 3] = ["main/st-1", "main/st-2", "main/st-3"];

type Confirms = [ConfirmChoice; 3];

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// 各 Step の `confirm` の選択肢。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConfirmChoice {
    PointAt,
    Answer,
    Approve,
    /// 宣言なし（R-22 により `approve` として扱う）。
    Unset,
    Skip,
}

impl ConfirmChoice {
    fn declaration(self) -> Option<Value> {
        match self {
            ConfirmChoice::PointAt => Some(json!({
                "kind": "point_at",
                "targets": [{"view": "v-files", "path": "notes/draft"}]
            })),
            ConfirmChoice::Answer => Some(json!({
                "kind": "answer",
                "on_wrong": "rj-wrong",
                "options": [
                    {"id": "opt-memo", "correct": true},
                    {"id": "opt-todo", "correct": false}
                ]
            })),
            ConfirmChoice::Approve => Some(json!({"kind": "approve"})),
            ConfirmChoice::Unset => None,
            ConfirmChoice::Skip => Some(json!({"kind": "none", "reason": "skip"})),
        }
    }
}

fn ir_with(confirms: &Confirms) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if let Some(i) = STEPS.iter().position(|s| *s == n.uid) {
            n.step.confirm = confirms[i].declaration();
        }
    }
    ir
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// 開始 Step が `Entered`・処理中でない・世代 1 の状態。
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

fn status_of(state: &SessionState, step: &str) -> StepStatus {
    let u = uid(step);
    state
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

#[derive(Debug, Clone)]
enum TestOp {
    Apply(Input),
    /// 進行の照合（TASK-210）の代わりに処理中を畳む擬似操作。
    FinishProcessing,
    /// 現在の世代・現在の Step・index 0 の `Op::Tick`。
    Tick,
}

fn ui(view: &str, path: &str) -> Input {
    Input::Ui {
        view: view.into(),
        path: path.into(),
    }
}

fn approval(decision: Decision, target: Option<&str>) -> Input {
    Input::Approval {
        decision,
        target: target.map(str::to_string),
    }
}

fn input_strategy() -> impl Strategy<Value = Input> {
    prop_oneof![
        3 => Just(Input::Command("/make-note first-note".into())),
        1 => Just(Input::Command("/make-note Bad_Name".into())),
        1 => Just(Input::Command("/nothing".into())),
        2 => Just(Input::Choice("opt-memo".into())),
        2 => Just(Input::Choice("opt-todo".into())),
        1 => Just(Input::Choice("opt-ghost".into())),
        3 => Just(ui("v-files", "notes/draft")),
        1 => Just(ui("v-files", "notes/other")),
        1 => Just(ui("v-other", "notes/draft")),
        3 => Just(approval(Decision::Approve, None)),
        1 => Just(approval(Decision::Approve, Some("notes/draft"))),
        1 => Just(approval(Decision::Reject, None)),
        1 => Just(approval(Decision::Revise, Some("notes/draft"))),
        1 => Just(Input::Text("文章".into())),
        1 => Just(Input::Cancel),
    ]
}

fn op_strategy() -> impl Strategy<Value = TestOp> {
    prop_oneof![
        10 => input_strategy().prop_map(TestOp::Apply),
        3 => Just(TestOp::FinishProcessing),
        1 => Just(TestOp::Tick),
    ]
}

fn confirm_strategy() -> impl Strategy<Value = ConfirmChoice> {
    prop_oneof![
        3 => Just(ConfirmChoice::PointAt),
        3 => Just(ConfirmChoice::Answer),
        3 => Just(ConfirmChoice::Approve),
        2 => Just(ConfirmChoice::Unset),
        1 => Just(ConfirmChoice::Skip),
    ]
}

/// 流した結果の件数。
#[derive(Debug, Default)]
struct Tally {
    /// 受付の件数。
    accepted: usize,
    /// 確認を要する Step の達成の件数。
    confirmed: usize,
    /// 不変条件に違反した内容（Step・操作の番号つき）。
    violations: Vec<String>,
}

fn needs_confirm(confirms: &Confirms, step: &str) -> bool {
    STEPS
        .iter()
        .position(|s| *s == step)
        .map(|i| confirms[i] != ConfirmChoice::Skip)
        .unwrap_or(true)
}

/// 擬似操作・進行・入力を 1 件流し、今回のイベントを返す。
fn step_op(session: Session, op: &TestOp) -> (Session, Vec<Event>) {
    let mut session = session;
    match op {
        TestOp::Apply(input) => {
            let ev = session.apply(Op::Input(input.clone())).to_vec();
            (session, ev)
        }
        TestOp::Tick => {
            let addr = session.state().run.current.clone().map(|step| TickAddr {
                generation: session.generation(),
                step,
                index: 0,
            });
            let ev = match addr {
                Some(a) => session.apply(Op::Tick(a)).to_vec(),
                None => Vec::new(),
            };
            (session, ev)
        }
        TestOp::FinishProcessing => {
            let ir = session.ir().clone();
            let mut state = session.into_state();
            state.run.busy = None;
            (Session::from_state(ir, state), Vec::new())
        }
    }
}

/// 操作列を流し、操作のたびに I1〜I4 を検査する。
fn verify(confirms: &Confirms, start: &str, ops: &[TestOp]) -> Tally {
    let mut session = Session::from_state(ir_with(confirms), entered(start));
    let mut tally = Tally::default();
    for (n, op) in ops.iter().enumerate() {
        let before = session.state().clone();
        let (next, events) = step_op(session, op);
        session = next;
        for ev in &events {
            if matches!(ev, Event::Accepted { .. }) {
                tally.accepted += 1;
            }
            let Event::Achieved { at, explicit_skip } = ev else {
                continue;
            };
            let step = at.step.0.as_str();
            if !needs_confirm(confirms, step) {
                continue;
            }
            tally.confirmed += 1;
            let was = status_of(&before, step);
            if was != StepStatus::Accepted || before.run.busy.is_some() {
                tally.violations.push(format!(
                    "I1 op#{n} {step}: achieved from {was:?} busy={:?} (skip={explicit_skip})",
                    before.run.busy
                ));
            }
            if events
                .iter()
                .any(|e| matches!(e, Event::Accepted { at: a } if a == at))
            {
                tally
                    .violations
                    .push(format!("I2 op#{n} {step}: accepted and achieved together"));
            }
            if !before
                .log
                .iter()
                .any(|e| matches!(e, Event::Accepted { at: a } if a == at))
            {
                tally
                    .violations
                    .push(format!("I3 op#{n} {step}: no earlier Accepted in the log"));
            }
        }
        let after = session.state();
        for step in STEPS {
            if needs_confirm(confirms, step)
                && status_of(after, step) == StepStatus::Achieved
                && !after
                    .log
                    .iter()
                    .any(|e| matches!(e, Event::Achieved { at, .. } if at.step.0 == step))
            {
                tally.violations.push(format!(
                    "I4 op#{n} {step}: Achieved status without a log entry"
                ));
            }
        }
    }
    tally
}

#[test]
fn p3_no_achievement_before_confirmation() {
    let strategy = (
        [confirm_strategy(), confirm_strategy(), confirm_strategy()],
        0usize..3,
        op_sequence(op_strategy()),
    );
    check(
        "p3-no-achievement-before-confirmation",
        DEFAULT_CASES,
        strategy,
        |(confirms, start, ops)| {
            let tally = verify(&confirms, STEPS[start], &ops);
            prop_assert_eq!(
                tally.violations.len(),
                0,
                "achievements before confirmation: {:?}",
                tally.violations
            );
            Ok(())
        },
    );
}

fn cmd(s: &str) -> TestOp {
    TestOp::Apply(Input::Command(s.into()))
}

fn choice(s: &str) -> TestOp {
    TestOp::Apply(Input::Choice(s.into()))
}

/// 列を流した後の、指定の Step の状態。
fn status_after(confirms: &Confirms, start: &str, ops: &[TestOp], step: &str) -> StepStatus {
    let mut session = Session::from_state(ir_with(confirms), entered(start));
    for op in ops {
        session = step_op(session, op).0;
    }
    status_of(session.state(), step)
}

#[test]
fn p3_fixed_sequence_confirms_all_three_kinds() {
    let confirms = [
        ConfirmChoice::PointAt,
        ConfirmChoice::Answer,
        ConfirmChoice::Approve,
    ];
    let to_miss = vec![
        cmd("/make-note first-note"),
        TestOp::FinishProcessing,
        TestOp::Apply(ui("v-files", "notes/other")),
    ];
    // 外れた指し先は受付済みのまま残る。
    assert_eq!(
        status_after(&confirms, "main/st-1", &to_miss, "main/st-1"),
        StepStatus::Accepted
    );
    let ops = vec![
        cmd("/make-note first-note"),
        TestOp::FinishProcessing,
        TestOp::Apply(ui("v-files", "notes/other")),
        TestOp::Apply(ui("v-files", "notes/draft")),
        choice("opt-memo"),
        choice("opt-todo"),
        choice("opt-memo"),
        TestOp::Apply(approval(Decision::Approve, None)),
        TestOp::Apply(approval(Decision::Approve, None)),
    ];
    let tally = verify(&confirms, "main/st-1", &ops);
    assert_eq!(tally.violations, Vec::<String>::new());
    assert_eq!(tally.confirmed, 3, "{tally:?}");
    assert_eq!(tally.accepted, 3, "{tally:?}");
}

#[test]
fn p3_missing_confirm_needs_approval_and_none_is_excluded() {
    // 宣言なしの Step は承認で初めて達成になる（R-22）。
    let confirms = [
        ConfirmChoice::Unset,
        ConfirmChoice::Skip,
        ConfirmChoice::Skip,
    ];
    let mut ops = vec![
        cmd("/make-note first-note"),
        TestOp::FinishProcessing,
        TestOp::Apply(ui("v-files", "notes/draft")),
    ];
    assert_eq!(
        status_after(&confirms, "main/st-1", &ops, "main/st-1"),
        StepStatus::Accepted
    );
    ops.push(TestOp::Apply(approval(Decision::Approve, None)));
    let tally = verify(&confirms, "main/st-1", &ops);
    assert_eq!(tally.violations, Vec::<String>::new());
    assert_eq!(tally.confirmed, 1, "{tally:?}");
    assert_eq!(
        status_after(&confirms, "main/st-1", &ops, "main/st-1"),
        StepStatus::Achieved
    );
}
