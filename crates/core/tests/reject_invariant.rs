//! 拒否の後に状態が変わらないことの性質テスト（TASK-270、JUDGE-5）。
//!
//! 拒否された入力（`Event::Judged` が `Judgment::Rejected`）は、Step を受付済みにせず、世界状態も変えない。
//! 例の確認は `reject_scene.rs`（TASK-269）が持ち、ここではランダムな入力列で、拒否のたびに次を確かめる。
//!
//! 1. 戻りのイベントがちょうど `InputReceived` と拒否の `Judged` の 2 件である（`Accepted`・`Changed`・`Warning` を含まない）
//! 2. 操作の後の `run`（現在の Step・状態・世界状態・処理中・世代・案内レベルを含む）が操作の前と一致する
//! 3. `log` 以外の欄（`entry`・`checkpoints`・`next_checkpoint`・`drafts`・`hints_opened`）が操作の前と一致し、`log` は追記だけである
//! 4. 操作の前に `Accepted` でなかった現在の Step が、操作の後も `Accepted` でない
//! 5. 拒否の次の行動が空でない（JUDGE-4 の再確認）
//!
//! 受け入れ条件の「ランダムな入力列（1,000 通り以上）」は [`CASES`] である。拒否の 7 分類
//! （`RejectCategory::ALL`）が 1 件以上出たことも確かめる（件数は種に依存するため assert しない）。
//!
//! IR の出所は 2 つで、`min-3step.json`（`op_sequence_properties.rs` と同じ。固定・誘導つきの操作列）と
//! `tests/data/reject_scene_1/judgment-ir.json`（`reject_scene.rs` と同じ。Step の状態を指定した入力列）である。
//! 範囲外: 入力欄に残すこと（TASK-271）、問い返し（JUDGE-6）、移動・チェックポイントの `Refused`（EVT-11・EVT-12）。
//! 変異注入の定義は `tests/mutations/JUDGE5-4.mutation` 以降。

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{guided_op_specs, op_specs, run_with, Applied, OpSpec};
use common::property::check;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

/// ランダムな入力列の通り数。受け入れ条件は 1,000 通り以上である。
const CASES: u32 = 1_000;

/// 分類ごとの拒否の件数。添字は `RejectCategory::ALL` の位置である。
type Counts = RefCell<[u32; 7]>;

fn slot(category: RejectCategory) -> usize {
    RejectCategory::ALL
        .iter()
        .position(|c| *c == category)
        .unwrap_or(0)
}

fn report(name: &str, counts: &Counts) {
    let c = counts.borrow();
    let line: Vec<String> = RejectCategory::ALL
        .iter()
        .zip(c.iter())
        .map(|(cat, n)| format!("{cat:?}={n}"))
        .collect();
    println!("REJECT-COVERAGE name={name} {}", line.join(" "));
}

/// 拒否なら不変条件を確かめ、拒否の分類を返す。拒否でなければ `None`。
fn check_rejection(
    before: &SessionState,
    after: &SessionState,
    events: &[Event],
) -> Result<Option<RejectCategory>, TestCaseError> {
    let rejection = events.iter().find_map(|e| match e {
        Event::Judged {
            judgment: Judgment::Rejected(r),
            ..
        } => Some(r.clone()),
        _ => None,
    });
    let Some(rejection) = rejection else {
        return Ok(None);
    };
    prop_assert!(
        matches!(events, [Event::InputReceived { .. }, Event::Judged { .. }]),
        "a rejection must emit exactly InputReceived and Judged: {events:?}"
    );
    prop_assert_eq!(&after.run, &before.run, "run changed by a rejection");
    let mut after_without_log = after.clone();
    after_without_log.log = before.log.clone();
    prop_assert_eq!(
        &after_without_log,
        before,
        "a field other than the log changed by a rejection"
    );
    let mut expected_log = before.log.clone();
    expected_log.extend(events.iter().cloned());
    prop_assert_eq!(&after.log, &expected_log, "the log must only be appended");
    let accepted = |s: &SessionState| {
        s.run.current.as_ref().is_some_and(|uid| {
            s.run
                .slices
                .get(&uid.root())
                .and_then(|sl| sl.status.get(uid))
                == Some(&StepStatus::Accepted)
        })
    };
    if !accepted(before) {
        prop_assert!(!accepted(after), "a rejection accepted the step");
    }
    prop_assert!(
        !rejection.next_action.as_str().trim().is_empty(),
        "a rejection must have a next action"
    );
    Ok(Some(rejection.category))
}

fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

fn exercise_op_specs<S>(name: &str, strategy: S) -> Counts
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let (ir, initial) = load_min_ir();
    let counts: Counts = RefCell::new([0; 7]);
    check(name, CASES, strategy, |specs| {
        run_with(&ir, initial.clone(), &specs, |_, applied: &Applied| {
            if applied.op.is_none() {
                return Ok(());
            }
            if let Some(cat) = check_rejection(&applied.before, &applied.after, &applied.events)? {
                counts.borrow_mut()[slot(cat)] += 1;
            }
            Ok(())
        })
        .map(|_| ())
    });
    counts
}

/// 3 本とも 7 分類すべてに届く（実測 2026-10-21。届かなくなったら戦略を直し、この assert を外さない）。
fn assert_all_categories(name: &str, counts: &Counts) {
    report(name, counts);
    for (cat, n) in RejectCategory::ALL.iter().zip(counts.borrow().iter()) {
        assert!(
            *n > 0,
            "{name}: rejection category {cat:?} was never exercised"
        );
    }
}

#[test]
fn rejections_keep_state_on_random_sequences() {
    let counts = exercise_op_specs("reject-invariant-fixed", op_specs());
    assert_all_categories("fixed", &counts);
}

#[test]
fn rejections_keep_state_on_guided_sequences() {
    let counts = exercise_op_specs("reject-invariant-guided", guided_op_specs());
    assert_all_categories("guided", &counts);
}

// --- Step の状態を指定する入力列（reject_scene.rs と同じ IR と状態の作り方） ---

const STEPS: [&str; 6] = [
    "st-clarify",
    "st-two-args",
    "st-one-arg",
    "st-text",
    "st-choice",
    "st-approval",
];

const STATUSES: [StepStatus; 5] = [
    StepStatus::NotStarted,
    StepStatus::Entered,
    StepStatus::Accepted,
    StepStatus::Achieved,
    StepStatus::Skipped,
];

const COMMANDS: [&str; 12] = [
    "/help",
    "/make-idea first-idea",
    "/make-idea Bad_Name",
    "/make-idea",
    "/start-run",
    "/start-run abc",
    "/start-run 42",
    "/start-run 42 mode",
    "/publish-run",
    "/sync-cloud",
    "/nothing",
    "plain line",
];

fn judgment_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/reject_scene_1/judgment-ir.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn run_at(step: &str, status: StepStatus, busy: bool) -> RunState {
    let u = StepUid(format!("main/{step}"));
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), status);
    let mut slices = BTreeMap::new();
    slices.insert(Root("main".to_string()), slice);
    RunState {
        generation: Generation(1),
        current: Some(u),
        busy: busy.then_some(Busy { done: 0, total: 1 }),
        slices,
        ..RunState::default()
    }
}

fn input_strategy() -> impl Strategy<Value = Input> {
    prop_oneof![
        6 => (0..COMMANDS.len()).prop_map(|i| Input::Command(COMMANDS[i].to_string())),
        2 => Just(Input::Text("ノートを作りたい".to_string())),
        2 => prop_oneof![Just("opt-a"), Just("opt-none")].prop_map(|s| Input::Choice(s.to_string())),
        1 => Just(Input::Ui { view: "v-files".to_string(), path: "notes/draft".to_string() }),
        2 => prop_oneof![Just(Decision::Approve), Just(Decision::Reject), Just(Decision::Revise)]
            .prop_map(|d| Input::Approval { decision: d, target: None }),
        1 => Just(Input::Cancel),
    ]
}

#[test]
fn rejections_keep_state_from_any_status() {
    let ir = judgment_ir();
    let counts: Counts = RefCell::new([0; 7]);
    let strategy = (
        0..STEPS.len(),
        0..STATUSES.len(),
        any::<bool>(),
        proptest::collection::vec(input_strategy(), 1..=20),
    );
    check(
        "reject-invariant-any-status",
        CASES,
        strategy,
        |(s, st, busy, inputs)| {
            let state = SessionState {
                run: run_at(STEPS[s], STATUSES[st], busy),
                ..SessionState::default()
            };
            let mut session = Session::from_state(ir.clone(), state);
            for input in inputs {
                let before = session.state().clone();
                let events = session.apply(Op::Input(input)).to_vec();
                if let Some(cat) = check_rejection(&before, session.state(), &events)? {
                    counts.borrow_mut()[slot(cat)] += 1;
                }
            }
            Ok(())
        },
    );
    assert_all_categories("any-status", &counts);
}
