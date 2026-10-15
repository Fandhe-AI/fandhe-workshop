//! 判定結果の統合と `Judged` の材料（TASK-267、JUDGE-3）の試験。
//!
//! 試験データ `tests/data/judge_1/three-scenes.json` は、ir format 2 の形に合わせた手書きの最小の IR で、
//! 3 場面（引数なしのコマンドへの問い返し・必須引数の不足の拒否・同じ入力の受付）と、宣言の誤りの否定ケースを持つ。

mod common;

use std::collections::BTreeMap;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, JudgmentKind, RejectCategory};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::effect::{ConditionEval, NoConditions};
use fandhe_workshop_core::event::{Event, IgnoreReason};
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_core::judge::{
    judge_input, judge_tick, judged_event, JudgeDetail, JudgeError, JudgeOutcome,
};
use fandhe_workshop_core::session::{Busy, RunState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use proptest::prelude::*;
use serde_json::Value;

/// `when` を常に真とみなす評価器。
struct AlwaysTrue;

impl ConditionEval for AlwaysTrue {
    fn eval(&self, _when: &Value, _run: &RunState, _root: &Root) -> Option<bool> {
        Some(true)
    }
}

fn ir() -> Ir {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/judge_1/three-scenes.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn run_at(step: &str, status: StepStatus, busy: bool) -> RunState {
    let uid = StepUid(format!("main/{step}"));
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), status);
    let mut slices = BTreeMap::new();
    slices.insert(Root("main".to_string()), slice);
    RunState {
        current: Some(uid),
        busy: busy.then_some(Busy { done: 0, total: 2 }),
        slices,
        ..RunState::default()
    }
}

fn entered(step: &str) -> RunState {
    run_at(step, StepStatus::Entered, false)
}

fn cmd(s: &str) -> Input {
    Input::Command(s.to_string())
}

fn txt(s: &str) -> Input {
    Input::Text(s.to_string())
}

fn judge(run: &RunState, input: &Input) -> Judgment {
    judge_input(&ir(), run, input, &NoConditions)
        .unwrap()
        .judgment
}

fn judge_err(run: &RunState, input: &Input) -> JudgeError {
    judge_input(&ir(), run, input, &NoConditions).unwrap_err()
}

fn rejected(j: &Judgment) -> (RejectCategory, &str) {
    match j {
        Judgment::Rejected(r) => (r.category, r.text.as_str()),
        other => panic!("not rejected: {other:?}"),
    }
}

#[test]
fn three_scenes_are_distinct_outcomes() {
    let input = cmd("/implement-issue-tree 42");
    let new_idea = cmd("/new-idea");

    // ST-1.4 相当: 引数なしのコマンドは問い返し。
    let clarified = judge(&entered("st-1-4"), &new_idea);
    // ST-7.1 相当: 必須引数の不足は拒否（分類つき）。
    let refused = judge(&entered("st-7-1"), &input);
    // ST-7.3 相当: 同じ入力が受付になる。
    let accepted = judge(&entered("st-7-3"), &input);

    match &clarified {
        Judgment::Clarified(c) => {
            assert_eq!(c.text, "cl-new-idea-name");
            assert!(!c.next_action.as_str().trim().is_empty());
        }
        other => panic!("{other:?}"),
    }
    let (category, text) = rejected(&refused);
    assert_eq!(category, RejectCategory::MissingArg);
    assert_eq!(text, "rj-default-missing-arg");
    if let Judgment::Rejected(r) = &refused {
        assert!(!r.next_action.as_str().trim().is_empty());
    }
    assert_eq!(accepted, Judgment::Accepted(None));

    let kinds = [clarified.kind(), refused.kind(), accepted.kind()];
    assert_eq!(
        kinds,
        [
            JudgmentKind::Clarified,
            JudgmentKind::Rejected,
            JudgmentKind::Accepted
        ]
    );
    assert_ne!(kinds[0], kinds[1]);
    assert_ne!(kinds[1], kinds[2]);
    assert_ne!(kinds[0], kinds[2]);

    // 受付の規則の効果の適用は呼び出し側。どの規則かを付帯情報で返す。
    let out = judge_input(&ir(), &entered("st-7-3"), &input, &NoConditions).unwrap();
    assert_eq!(
        out.detail,
        JudgeDetail::Rule {
            rule_id: Some("r-impl-blocked".to_string())
        }
    );
}

#[test]
fn command_rules_and_shapes() {
    let run = entered("st-1-4");
    assert_eq!(judge(&run, &cmd("/new-idea foo")), Judgment::Accepted(None));
    let bad = judge(&run, &cmd("/new-idea Bad_Name"));
    assert_eq!(rejected(&bad).0, RejectCategory::BadArgShape);
    // 宣言と違う名前（教材の表にはある）は、この Step で使わない入力。
    let other = judge(&run, &cmd("/help"));
    assert_eq!(rejected(&other).0, RejectCategory::NotInThisStep);
    // 接頭辞が無い行を Command として受けた場合も同じ。
    let noprefix = judge(&run, &cmd("hello"));
    assert_eq!(rejected(&noprefix).0, RejectCategory::NotInThisStep);
    // 自然文・別方式の入力はこの Step では使わない。
    assert_eq!(
        rejected(&judge(&run, &txt("hello"))).0,
        RejectCategory::NotInThisStep
    );
    assert_eq!(
        rejected(&judge(&entered("st-text"), &cmd("/new-idea"))).0,
        RejectCategory::NotInThisStep
    );
    // 規則の拒否は文面の ID の分類に従う。
    let by_rule = judge(&entered("st-reject-rule"), &cmd("/new-idea"));
    assert_eq!(
        rejected(&by_rule),
        (RejectCategory::BadArgShape, "rj-rule-bad-shape")
    );
}

#[test]
fn order_busy_before_name_and_unknown_in_any_stage() {
    let busy = run_at("st-1-4", StepStatus::Entered, true);
    assert_eq!(
        rejected(&judge(&busy, &cmd("/new-idea foo"))).0,
        RejectCategory::Busy
    );
    assert_eq!(
        rejected(&judge(&busy, &cmd("/zzz"))).0,
        RejectCategory::Busy
    );
    for status in [
        StepStatus::NotStarted,
        StepStatus::Entered,
        StepStatus::Accepted,
        StepStatus::Achieved,
        StepStatus::Skipped,
    ] {
        let run = run_at("st-1-4", status, false);
        assert_eq!(
            rejected(&judge(&run, &cmd("/zzz"))).0,
            RejectCategory::UnknownCommand,
            "{status:?}"
        );
    }
    for status in [
        StepStatus::NotStarted,
        StepStatus::Achieved,
        StepStatus::Skipped,
    ] {
        let run = run_at("st-1-4", status, false);
        assert_eq!(
            rejected(&judge(&run, &cmd("/new-idea"))).0,
            RejectCategory::NotInThisStep
        );
    }
}

#[test]
fn ignored_without_current_step() {
    let none = RunState::default();
    let out = judge_input(&ir(), &none, &cmd("/new-idea"), &NoConditions).unwrap();
    assert_eq!(out.judgment, Judgment::Ignored);
    assert_eq!(out.detail, JudgeDetail::Ignored(IgnoreReason::NoCurrent));
    let mut unknown = entered("st-1-4");
    unknown.current = Some(StepUid("main/st-nope".to_string()));
    let out = judge_input(&ir(), &unknown, &cmd("/new-idea"), &NoConditions).unwrap();
    assert_eq!(out.detail, JudgeDetail::Ignored(IgnoreReason::NoCurrent));
}

#[test]
fn choice_and_confirm_stages() {
    let o1 = Input::Choice("o1".to_string());
    let o2 = Input::Choice("o2".to_string());
    assert_eq!(judge(&entered("st-choice"), &o1), Judgment::Accepted(None));
    assert_eq!(
        rejected(&judge(&entered("st-choice"), &o2)),
        (RejectCategory::NotInThisStep, "rj-wrong")
    );
    let acc = run_at("st-choice", StepStatus::Accepted, false);
    assert_eq!(judge(&acc, &o1), Judgment::Confirmed);
    assert_eq!(rejected(&judge(&acc, &o2)).1, "rj-wrong");
    assert_eq!(
        rejected(&judge(&acc, &cmd("/new-idea"))).0,
        RejectCategory::NotInThisStep
    );
    let approval = Input::Approval {
        decision: Decision::Approve,
        target: None,
    };
    assert_eq!(
        rejected(&judge(&entered("st-choice"), &approval)).0,
        RejectCategory::NotInThisStep
    );
}

#[test]
fn text_stage() {
    let run = entered("st-text");
    let out = judge_input(&ir(), &run, &txt("hello"), &NoConditions).unwrap();
    match &out.judgment {
        Judgment::Accepted(Some(note)) => {
            assert_eq!(note.text, "rj-default-free-text-unjudged");
            assert!(!note.next_action.as_str().is_empty());
        }
        other => panic!("{other:?}"),
    }
    match &out.detail {
        JudgeDetail::Text(t) => {
            assert_eq!(t.text, "hello");
            assert_eq!(t.effects.len(), 1);
        }
        other => panic!("{other:?}"),
    }
    // 範囲外は bad_arg_shape の拒否。
    assert_eq!(
        rejected(&judge(&run, &txt(""))).0,
        RejectCategory::BadArgShape
    );
    assert_eq!(
        rejected(&judge(&run, &txt(&"あ".repeat(21)))).0,
        RejectCategory::BadArgShape
    );
    // `on.text` の拒否・問い返しの規則が先に当たれば、その結果。
    let rej_step = entered("st-text-reject");
    let out = judge_input(&ir(), &rej_step, &txt("hello"), &AlwaysTrue).unwrap();
    assert_eq!(
        rejected(&out.judgment),
        (RejectCategory::BadArgShape, "rj-rule-bad-shape")
    );
    // `when` が評価できなければ飛ばし、後ろの受付の規則になる。
    assert!(matches!(
        judge(&rej_step, &txt("hello")),
        Judgment::Accepted(Some(_))
    ));
    assert!(matches!(
        judge(&entered("st-text-clarify"), &txt("hello")),
        Judgment::Clarified(_)
    ));
    // 注記の文面を解決できなくても受付は妨げない。
    assert_eq!(
        judge(&entered("st-text-nonote"), &txt("hello")),
        Judgment::Accepted(None)
    );
}

#[test]
fn cancel_follows_rule_or_is_not_in_this_step() {
    let out = judge_input(&ir(), &entered("st-cancel"), &Input::Cancel, &NoConditions).unwrap();
    assert_eq!(out.judgment, Judgment::Accepted(None));
    assert_eq!(
        out.detail,
        JudgeDetail::Rule {
            rule_id: Some("r-cancel".to_string())
        }
    );
    assert_eq!(
        rejected(&judge(&entered("st-1-4"), &Input::Cancel)).0,
        RejectCategory::NotInThisStep
    );
}

#[test]
fn tick_cases() {
    let mut run = run_at("st-1-4", StepStatus::Accepted, true);
    run.generation = Generation(3);
    let addr = |generation: u64, step: &str, index: u32| TickAddr {
        generation: Generation(generation),
        step: StepUid(step.to_string()),
        index,
    };
    let ok = judge_tick(&run, &addr(3, "main/st-1-4", 0));
    assert_eq!(ok.judgment, Judgment::Progressed);
    for stale in [
        addr(2, "main/st-1-4", 0),
        addr(3, "main/st-7-1", 0),
        addr(3, "main/st-1-4", 1),
    ] {
        let out = judge_tick(&run, &stale);
        assert_eq!(out.judgment, Judgment::Ignored);
        assert_eq!(out.detail, JudgeDetail::Ignored(IgnoreReason::Stale));
    }
    let mut idle = run.clone();
    idle.busy = None;
    let out = judge_tick(&idle, &addr(3, "main/st-1-4", 0));
    assert_eq!(out.detail, JudgeDetail::Ignored(IgnoreReason::NoProcessing));
    let out = judge_tick(&RunState::default(), &addr(1, "main/st-1-4", 0));
    assert_eq!(out.detail, JudgeDetail::Ignored(IgnoreReason::NoCurrent));
}

#[test]
fn wrong_declarations_fail_closed_with_typed_errors() {
    const MARK: &str = "SECRET-MARK";
    let marked = cmd(&format!("/new-idea {MARK}"));
    let cases = [
        ("st-bad-rule", JudgeError::InvalidDeclaration),
        ("st-bad-rejection-id", JudgeError::UnknownRejection),
        ("st-free-note-cat", JudgeError::InvalidRejectionCategory),
    ];
    for (step, expected) in cases {
        let err = judge_err(&entered(step), &marked);
        assert_eq!(err, expected, "{step}");
        let shown = err.to_string();
        assert!(!shown.contains(MARK) && !shown.contains("rj-"), "{shown}");
    }
    // 次の行動が空白だけの文面。
    assert_eq!(
        judge_err(&entered("st-empty-na"), &cmd("/help")),
        JudgeError::EmptyNextAction
    );
    // 既定の文面が無い分類。
    let mut no_defaults = ir();
    no_defaults.defaults.clear();
    let err = judge_input(
        &no_defaults,
        &entered("st-plain"),
        &cmd("/help"),
        &NoConditions,
    )
    .unwrap_err();
    assert_eq!(err, JudgeError::NoRejectionText);
    // 問い返しの文面が無い。
    let mut no_cl = ir();
    no_cl.clarifications.clear();
    let err =
        judge_input(&no_cl, &entered("st-1-4"), &cmd("/new-idea"), &NoConditions).unwrap_err();
    assert_eq!(err, JudgeError::UnknownClarification);
}

const NODES: [&str; 16] = [
    "st-1-4",
    "st-7-1",
    "st-7-3",
    "st-bad-rule",
    "st-bad-rejection-id",
    "st-reject-rule",
    "st-free-note-cat",
    "st-text",
    "st-text-reject",
    "st-text-clarify",
    "st-text-nonote",
    "st-choice",
    "st-cancel",
    "st-empty-na",
    "st-plain",
    "st-missing",
];

/// 宣言が正しく、必ず `Ok` になる Step。
fn is_well_formed(step: &str) -> bool {
    !matches!(
        step,
        "st-bad-rule" | "st-bad-rejection-id" | "st-free-note-cat" | "st-empty-na"
    )
}

fn any_input() -> impl Strategy<Value = Input> {
    let strings = prop::sample::select(vec![
        "/new-idea",
        "/new-idea foo",
        "/new-idea Bad_Name",
        "/implement-issue-tree",
        "/implement-issue-tree 42",
        "/implement-issue-tree 42 --external-checks x",
        "/implement-issue-tree --external-checks",
        "/help",
        "/zzz",
        "/",
        "hello",
        "",
        "   ",
        "あいうえおかきくけこさしすせそたちつてとなにぬねの",
    ]);
    prop_oneof![
        strings.clone().prop_map(|s| Input::Command(s.to_string())),
        strings.prop_map(|s| Input::Text(s.to_string())),
        prop::sample::select(vec!["o1", "o2", "zzz"]).prop_map(|s| Input::Choice(s.to_string())),
        (
            prop::sample::select(vec!["v-cards", "v-x"]),
            prop::sample::select(vec!["cards/a", "x"])
        )
            .prop_map(|(v, p)| Input::Ui {
                view: v.to_string(),
                path: p.to_string()
            }),
        (
            prop::sample::select(vec![Decision::Approve, Decision::Reject, Decision::Revise]),
            prop::option::of(Just("t".to_string()))
        )
            .prop_map(|(decision, target)| Input::Approval { decision, target }),
        Just(Input::Cancel),
    ]
}

fn any_status() -> impl Strategy<Value = StepStatus> {
    prop::sample::select(vec![
        StepStatus::NotStarted,
        StepStatus::Entered,
        StepStatus::Accepted,
        StepStatus::Achieved,
        StepStatus::Skipped,
    ])
}

#[test]
fn every_input_lands_in_exactly_one_kind() {
    let ir = ir();
    let strategy = (
        prop::sample::select(NODES.to_vec()),
        any_status(),
        any::<bool>(),
        any::<bool>(),
        any_input(),
    );
    check(
        "judge-every-input-one-kind",
        DEFAULT_CASES,
        strategy,
        move |(step, status, busy, no_current, input)| {
            let mut run = run_at(step, status, busy);
            if no_current {
                run.current = None;
            }
            let before = run.clone();
            let always = AlwaysTrue;
            let conds: [&dyn ConditionEval; 2] = [&NoConditions, &always];
            for cond in conds {
                let result = judge_input(&ir, &run, &input, cond);
                prop_assert_eq!(&run, &before);
                let out: JudgeOutcome = match result {
                    Ok(out) => out,
                    Err(e) => {
                        // 誤った宣言の Step だけが型つきのエラーになる。
                        prop_assert!(!is_well_formed(step), "{e}");
                        continue;
                    }
                };
                prop_assert!(JudgmentKind::ALL.contains(&out.judgment.kind()));
                match &out.judgment {
                    Judgment::Rejected(r) => {
                        prop_assert!(!r.next_action.as_str().trim().is_empty())
                    }
                    Judgment::Clarified(c) => {
                        prop_assert!(!c.next_action.as_str().trim().is_empty())
                    }
                    _ => {}
                }
                // 現在の Step が無い入力だけが無視になる。進行は入力から出ない。
                prop_assert_eq!(
                    out.judgment == Judgment::Ignored,
                    no_current || step == "st-missing"
                );
                prop_assert!(out.judgment != Judgment::Progressed);
                let at = Stamp {
                    generation: Generation(1),
                    step: StepUid(format!("main/{step}")),
                };
                let ev = judged_event(at, out.judgment.clone());
                let text = serde_json::to_string(&ev).unwrap();
                let back: Event = serde_json::from_str(&text).unwrap();
                prop_assert_eq!(back, ev);
            }
            Ok(())
        },
    );
}

#[test]
fn well_formed_steps_never_error_for_entered_commands() {
    // 良い宣言の Step は、どの入力でも Err にならない（受付へ倒れず、必ず 6 種のどれかになる）。
    for step in NODES.iter().filter(|s| is_well_formed(s)) {
        for input in [
            cmd("/new-idea"),
            cmd("/implement-issue-tree 42"),
            cmd("/zzz"),
            txt("hello"),
            Input::Cancel,
        ] {
            for status in [StepStatus::Entered, StepStatus::Accepted] {
                let run = run_at(step, status, false);
                assert!(
                    judge_input(&ir(), &run, &input, &NoConditions).is_ok(),
                    "{step} {input:?} {status:?}"
                );
            }
        }
    }
}
