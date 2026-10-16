//! 拒否の場面の判定（処理中 `busy`・この Step では使わない `not_in_this_step`・教材未対応 `not_supported`）の試験
//! （TASK-269、JUDGE-4・JUDGE-5）。
//!
//! 受け入れ条件は 3 つで、1 つずつ試験にする。
//!
//! - 3 場面で期待する分類が 100% 返る（[`playthrough_pairs_are_classified_100_percent`] と性質テスト）
//! - 次の行動が空の拒否が 0 件である（同じ試験で数える）
//! - 処理中の入力が `busy` になる（`Session` の経路で、同じ入力・別の入力のどちらも確かめる）
//!
//! 誤入力の組の出所は `tests/data/reject_scene_1/judgment-ir.json` で、pack の `judgment-pack` の compile の結果の写しである
//! （`e-busy-1`・`e-busy-2`・`e-nits-1〜5`・`e-unsupported-1`・`e-unsupported-2` に対応する。core は pack に依存できないため、
//! 手元の写しを使う。写しのずれは pack の `error_input_expect.rs` が検出する）。
//! 拒否の前後の状態の同一性の性質テストは TASK-270、入力欄に残すことは TASK-271 が受け持つため、ここでは例の確認に留める。

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::command_class::{classify_command, CommandClass};
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::judge::judge_input;
use fandhe_workshop_core::session::{Busy, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;

fn judgment_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data/reject_scene_1/judgment-ir.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn min3() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn uid(step: &str) -> StepUid {
    StepUid(format!("main/{step}"))
}

fn run_at(step: &str, status: StepStatus, busy: bool) -> RunState {
    let u = uid(step);
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

/// 開始点の Step が `Entered` で、処理中でない状態の `Session`。
fn session_at(ir: Ir, step: &str) -> Session {
    let state = SessionState {
        run: run_at(step, StepStatus::Entered, false),
        ..SessionState::default()
    };
    Session::from_state(ir, state)
}

fn cmd(s: &str) -> Input {
    Input::Command(s.to_string())
}

/// 戻りのイベントが `InputReceived` と拒否の `Judged` だけなら、その拒否を返す。
fn only_rejection(events: &[Event]) -> Option<(RejectCategory, bool)> {
    match events {
        [Event::InputReceived { .. }, Event::Judged {
            judgment: Judgment::Rejected(r),
            ..
        }] => Some((r.category, !r.next_action.as_str().trim().is_empty())),
        _ => None,
    }
}

struct Pair {
    id: &'static str,
    start: &'static str,
    pre: Option<&'static str>,
    input: Input,
    expect: RejectCategory,
}

fn pairs() -> Vec<Pair> {
    use RejectCategory::{Busy as B, NotInThisStep as N, NotSupported as S};
    let p = |id, start, pre, input, expect| Pair {
        id,
        start,
        pre,
        input,
        expect,
    };
    vec![
        p(
            "e-busy-1",
            "st-clarify",
            Some("/make-idea first-idea"),
            cmd("/make-idea second-idea"),
            B,
        ),
        p(
            "e-busy-2",
            "st-one-arg",
            Some("/start-run 42"),
            cmd("/start-run 43"),
            B,
        ),
        p("e-nits-1", "st-clarify", None, cmd("/help"), N),
        p("e-nits-2", "st-text", None, cmd("/make-idea first-idea"), N),
        p("e-nits-3", "st-choice", None, cmd("/help"), N),
        p(
            "e-nits-4",
            "st-approval",
            None,
            cmd("/make-idea first-idea"),
            N,
        ),
        p(
            "e-nits-5",
            "st-approval",
            None,
            Input::Text("承認の代わりに文を書く".into()),
            N,
        ),
        p("e-unsupported-1", "st-clarify", None, cmd("/sync-cloud"), S),
        p("e-unsupported-2", "st-text", None, cmd("/sync-cloud"), S),
        // 接頭辞の無い行・どの規則にも当たらない入力も、この Step では使わない入力である。
        p(
            "plain-line",
            "st-clarify",
            None,
            Input::Text("こんにちは".into()),
            N,
        ),
        p("skill-in-choice", "st-choice", None, cmd("/make-idea x"), N),
    ]
}

#[test]
fn playthrough_pairs_are_classified_100_percent() {
    let all = pairs();
    let total = all.len();
    let mut matched = 0;
    let mut empty_next = 0;
    for pair in &all {
        let mut session = session_at(judgment_ir(), pair.start);
        if let Some(pre) = pair.pre {
            session.apply(Op::Input(cmd(pre)));
            assert!(
                session.state().run.busy.is_some(),
                "{}: pre must start processing",
                pair.id
            );
        }
        let events = session.apply(Op::Input(pair.input.clone())).to_vec();
        let (category, has_next) = only_rejection(&events)
            .unwrap_or_else(|| panic!("{}: not a plain rejection: {events:?}", pair.id));
        if category == pair.expect {
            matched += 1;
        }
        if !has_next {
            empty_next += 1;
        }
    }
    assert_eq!(matched, total, "classification rate must be 100%");
    assert_eq!(empty_next, 0, "no rejection may have an empty next action");
}

#[test]
fn rejections_do_not_change_the_step_or_world() {
    for pair in pairs().iter().filter(|p| p.pre.is_none()) {
        let mut session = session_at(judgment_ir(), pair.start);
        let before = session.state().run.clone();
        session.apply(Op::Input(pair.input.clone()));
        assert_eq!(session.state().run, before, "{}", pair.id);
    }
}

/// 処理中にして、状態を返す。
fn processing_session() -> Session {
    let mut session = session_at(judgment_ir(), "st-clarify");
    session.apply(Op::Input(cmd("/make-idea first-idea")));
    assert!(session.state().run.busy.is_some());
    session
}

#[test]
fn any_other_input_while_processing_is_busy() {
    let inputs = [
        cmd("/make-idea first-idea"),
        cmd("/make-idea second-idea"),
        cmd("/help"),
        cmd("/sync-cloud"),
        cmd("/nothing"),
        cmd("/publish-run"),
        cmd("plain line without prefix"),
        Input::Text("自然文".into()),
        Input::Choice("opt".into()),
        Input::Ui {
            view: "v".into(),
            path: "p".into(),
        },
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
    ];
    let mut session = processing_session();
    for input in &inputs {
        let before = session.state().run.clone();
        let events = session.apply(Op::Input(input.clone())).to_vec();
        assert_eq!(
            only_rejection(&events),
            Some((RejectCategory::Busy, true)),
            "{input:?}: {events:?}"
        );
        assert_eq!(session.state().run, before, "{input:?}");
    }
}

#[test]
fn cancel_is_not_busy() {
    let mut session = processing_session();
    let events = session.apply(Op::Input(Input::Cancel)).to_vec();
    assert!(
        !matches!(only_rejection(&events), Some((RejectCategory::Busy, _))),
        "{events:?}"
    );
}

#[test]
fn busy_wins_over_command_classes() {
    let ir = judgment_ir();
    for name in ["sync-cloud", "nothing", "publish-run", "help"] {
        let run = run_at("st-clarify", StepStatus::Entered, true);
        let out = judge_input(&ir, &run, &cmd(&format!("/{name}")), &NoConditions).expect("judge");
        assert!(
            matches!(&out.judgment, Judgment::Rejected(r) if r.category == RejectCategory::Busy),
            "{name}: {:?}",
            out.judgment
        );
    }
}

fn category_of(ir: &Ir, run: &RunState, raw: &str) -> Option<RejectCategory> {
    match judge_input(ir, run, &cmd(raw), &NoConditions)
        .expect("judge")
        .judgment
    {
        Judgment::Rejected(r) => Some(r.category),
        _ => None,
    }
}

fn all_statuses() -> [StepStatus; 5] {
    [
        StepStatus::NotStarted,
        StepStatus::Entered,
        StepStatus::Accepted,
        StepStatus::Achieved,
        StepStatus::Skipped,
    ]
}

#[test]
fn not_supported_in_every_status_even_when_the_step_declares_it() {
    let mut ir = judgment_ir();
    // st-clarify の宣言が sync-cloud を受け付けると書いていても、教材未対応が先に返る。
    let node = ir
        .nodes
        .iter_mut()
        .find(|n| n.uid == "main/st-clarify")
        .expect("node");
    node.step.input =
        Some(serde_json::json!({ "kind": "command", "name": "sync-cloud", "args": [] }));
    for st in all_statuses() {
        let run = run_at("st-clarify", st, false);
        assert_eq!(
            category_of(&ir, &run, "/sync-cloud"),
            Some(RejectCategory::NotSupported),
            "{st:?}"
        );
    }
}

#[test]
fn builtin_and_skill_are_not_in_this_step_when_not_accepted() {
    let ir = judgment_ir();
    for st in all_statuses() {
        let run = run_at("st-text", st, false);
        assert_eq!(
            category_of(&ir, &run, "/help"),
            Some(RejectCategory::NotInThisStep),
            "{st:?}"
        );
        assert_eq!(
            category_of(&ir, &run, "/make-idea x"),
            Some(RejectCategory::NotInThisStep),
            "{st:?}"
        );
    }
}

#[test]
fn other_ir_files_classify_the_three_scenes() {
    let ir = min3();
    let run = run_at("st-1", StepStatus::Entered, false);
    assert_eq!(
        category_of(&ir, &run, "/sync-cloud"),
        Some(RejectCategory::NotSupported)
    );
    assert_eq!(
        category_of(&ir, &run, "/publish-note"),
        Some(RejectCategory::WrongOrder)
    );
    assert_eq!(
        category_of(&ir, &run, "/nothing"),
        Some(RejectCategory::UnknownCommand)
    );
}

fn names() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("help"),
        Just("make-idea"),
        Just("publish-run"),
        Just("start-run"),
        Just("sync-cloud"),
        Just("nothing"),
    ]
}

fn status_strategy() -> impl Strategy<Value = StepStatus> {
    prop_oneof![
        Just(StepStatus::NotStarted),
        Just(StepStatus::Entered),
        Just(StepStatus::Accepted),
        Just(StepStatus::Achieved),
        Just(StepStatus::Skipped),
    ]
}

/// 語彙 v1 の 3 節の規則（処理中、教材にない名前、教材未対応、段階、順序違い、この Step では使わない）から作る期待。
/// 受け付ける組み合わせは `None`（受付の判定へ進むので、拒否の分類は問わない）。
fn oracle(
    ir: &Ir,
    step_uid: &str,
    status: StepStatus,
    busy: bool,
    name: &str,
) -> Option<RejectCategory> {
    if busy {
        return Some(RejectCategory::Busy);
    }
    let Some(kind) = ir.commands.get(name).and_then(|v| v.as_str()) else {
        return Some(RejectCategory::UnknownCommand);
    };
    if kind == "unsupported" {
        return Some(RejectCategory::NotSupported);
    }
    let node = ir.nodes.iter().find(|n| n.uid == step_uid).expect("node");
    let declared = node
        .step
        .input
        .as_ref()
        .filter(|i| i.get("kind").and_then(|k| k.as_str()) == Some("command"))
        .and_then(|i| i.get("name"))
        .and_then(|n| n.as_str());
    if status == StepStatus::Entered && declared == Some(name) {
        return None;
    }
    if kind == "flow" {
        Some(RejectCategory::WrongOrder)
    } else {
        Some(RejectCategory::NotInThisStep)
    }
}

#[test]
fn property_classification_matches_the_vocabulary_rules() {
    let ir = judgment_ir();
    let uids: Vec<String> = ir.nodes.iter().map(|n| n.uid.clone()).collect();
    check(
        "reject-scene",
        DEFAULT_CASES,
        (0..uids.len(), status_strategy(), any::<bool>(), names()),
        |(i, st, busy, name)| {
            let step = uids[i].trim_start_matches("main/");
            let run = run_at(step, st, busy);
            let got = judge_input(&ir, &run, &cmd(&format!("/{name}")), &NoConditions)
                .map_err(|e| TestCaseError::fail(e.to_string()))?;
            let got_cat = match &got.judgment {
                Judgment::Rejected(r) => {
                    prop_assert!(!r.next_action.as_str().trim().is_empty());
                    Some(r.category)
                }
                _ => None,
            };
            match oracle(&ir, &uids[i], st, busy, name) {
                Some(expect) => prop_assert_eq!(got_cat, Some(expect)),
                None => {
                    // 受け付ける組み合わせは、4 つの分類のどれにもならない。
                    use RejectCategory::{Busy, NotSupported, UnknownCommand, WrongOrder};
                    prop_assert!(!matches!(
                        got_cat,
                        Some(Busy | NotSupported | UnknownCommand | WrongOrder)
                    ));
                }
            }
            Ok(())
        },
    );
}

#[test]
fn command_class_agrees_with_judge_for_rejections() {
    let ir = judgment_ir();
    for node in &ir.nodes {
        let step = &node.step;
        for name in [
            "help",
            "make-idea",
            "publish-run",
            "start-run",
            "sync-cloud",
            "nothing",
        ] {
            let lexed = lex_command("/", &format!("/{name}")).expect("lex");
            for st in all_statuses() {
                let class = classify_command(&ir, step, st, &lexed).expect("classify");
                let run = run_at(node.uid.trim_start_matches("main/"), st, false);
                let got = category_of(&ir, &run, &format!("/{name}"));
                if class != CommandClass::Accepting {
                    assert_eq!(got, class.category(), "{} {name} {st:?}", node.uid);
                }
            }
        }
    }
}
