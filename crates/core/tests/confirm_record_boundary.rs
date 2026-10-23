//! 確認の記録から導出する自力の境界の通し試験（TASK-306 2/2、JUDGE-2・JUDGE-7・GUIDE-18）。
//!
//! 「入力を受け付けた」「確認の操作で達成になった」「確認の記録から自力を導く」は別の事象である。
//! 本試験は、受付だけ → 確認の操作 → 記録の導出の 3 段階で、`StepStatus`・`Event::Achieved` の件数・
//! `LoCheckRecord`・`FirstAnswerTally`（`lo_first_answer::tally`）が期待どおりに変わることを同じ試験の中で並べる。
//! 要点は 2 つある。確認の操作で達成になっても、最初の回答が誤答なら自力に数えないこと。自然文を受け付けても、
//! 文の内容の良し悪しは学習目標の判定に入らないこと。
//!
//! - `Event` から `CheckObservation` への写像は TASK-304 1/3 の担当で未実装のため、本試験の `choose` が代用する。
//!   TASK-304 の結線後は、その写像へ置き換える。
//! - ヒントを開く `Control::OpenHint` は結線済み（TASK-295。`Event::HintOpened` を出す）。ただし `Event::HintOpened` から
//!   `CheckObservation::HintOpened` への写像は上と同じ TASK-304 1/3（#876）の担当で未実装であり、比較の表示に対応する
//!   イベントも無い。そのためヒントの段と前後の比較の観測は列へ直接差し込む。TASK-304 の結線後は、その写像へ置き換える。
//! - 断定しないこと: ヒントの段による自力の規則（GUIDE-18 で未決）、`confirm: none`（`confirm_none_achieve`、TASK-208）、章の完了。
//!
//! 呼び出し元は CI の `make test` のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckMethod, CheckObservation, CheckTarget, FirstAnswer, LoCheckRecord,
    Reselection,
};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::lo_first_answer::{tally, FirstAnswerTally};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use serde_json::{json, Value};

const ST2: &str = "main/st-2";

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-2 の宣言のうち、指定の欄だけを差し替えた教材。
fn ir_with_st2(patch: Value) -> Ir {
    let mut ir = base_ir();
    for n in &mut ir.nodes {
        if n.uid == ST2 {
            let mut v = serde_json::to_value(&n.step).expect("to value");
            for (k, val) in patch.as_object().expect("object") {
                v[k] = val.clone();
            }
            n.step = serde_json::from_value::<Step>(v).expect("from value");
        }
    }
    ir
}

/// st-2 の確認を、`opt-memo` が正答の answer にした教材。受付の規則（`opt-memo`・`opt-todo`）はそのまま使う。
fn answer_ir() -> Ir {
    ir_with_st2(json!({ "confirm": {
        "kind": "answer",
        "on_wrong": "rj-wrong",
        "options": [
            { "id": "opt-memo", "correct": true },
            { "id": "opt-todo", "correct": false }
        ]
    }}))
}

/// st-2 を自然文の Step（確認は approve）にした教材。
fn text_ir() -> Ir {
    ir_with_st2(json!({
        "input": { "kind": "text", "min": 3, "max": 200 },
        "rules": [{
            "id": "r-text",
            "on": { "command": null, "missing": [], "args": {}, "option": null, "approval": null,
                    "target": null, "ui": null, "text": true, "cancel": null },
            "when": null,
            "outcome": { "kind": "accept", "effects": [] }
        }],
        "confirm": { "kind": "approve" }
    }))
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

fn achieved_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

/// st-2 向けの選択を送り、記録に使う観測を返す。
///
/// 送る前の st-2 が `Accepted`（確認の段階）のときだけ回答として数える。受付（`Entered` → `Accepted`）は回答に
/// 数えない。正誤は、その apply の戻りに `Achieved` があるか（外れは `InputReceived` だけ）で決める。
fn choose(s: &mut Session, choice: &str) -> Option<CheckObservation> {
    let confirming = status(s, ST2) == StepStatus::Accepted;
    let events = s.apply(Op::Input(Input::Choice(choice.into()))).to_vec();
    confirming.then(|| CheckObservation::Answered {
        step: ST2.into(),
        correct: achieved_count(&events) > 0,
    })
}

fn hint(opened: u32) -> CheckObservation {
    CheckObservation::HintOpened {
        step: ST2.into(),
        opened,
    }
}

fn comparison() -> CheckObservation {
    CheckObservation::ComparisonShown { step: ST2.into() }
}

fn targets(method: CheckMethod) -> Vec<CheckTarget> {
    vec![CheckTarget {
        step: ST2.into(),
        lo: vec!["LO-2".into()],
        method,
    }]
}

fn record(obs: &[CheckObservation], method: CheckMethod) -> LoCheckRecord {
    let mut r = derive_records(&targets(method), obs);
    assert_eq!(r.len(), 1);
    r.remove(0)
}

fn self_tally(obs: &[CheckObservation]) -> FirstAnswerTally {
    tally(&derive_records(&targets(CheckMethod::Answer), obs), "LO-2").expect("valid records")
}

/// st-2 を `opt-memo` で受け付けて `Accepted` にする。
fn accepted_session() -> Session {
    let mut s = Session::from_state(answer_ir(), entered(ST2));
    assert_eq!(choose(&mut s, "opt-memo"), None);
    assert_eq!(status(&s, ST2), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), 0);
    s
}

#[test]
fn stage_accept_only_is_unanswered_and_not_achieved() {
    let s = accepted_session();
    assert_eq!(status(&s, "main/st-3"), StepStatus::default());
    let r = record(&[], CheckMethod::Answer);
    assert_eq!(r.first_answer, FirstAnswer::Unanswered);
    let t = self_tally(&[]);
    assert_eq!((t.unanswered, t.self_reliant), (1, 0));
}

#[test]
fn first_confirm_correct_achieves_and_counts_as_self_reliant() {
    let mut s = accepted_session();
    let obs: Vec<_> = choose(&mut s, "opt-memo").into_iter().collect();
    assert_eq!(achieved_count(s.log()), 1);
    assert_eq!(status(&s, ST2), StepStatus::Achieved);
    assert_eq!(status(&s, "main/st-3"), StepStatus::Entered);
    let r = record(&obs, CheckMethod::Answer);
    assert_eq!(
        (r.first_answer, r.reselection),
        (FirstAnswer::Correct, Reselection::None)
    );
    assert_eq!(self_tally(&obs).self_reliant, 1);
}

#[test]
fn reselection_achieves_but_is_not_self_reliant() {
    let mut s = accepted_session();
    let mut obs = Vec::new();
    obs.extend(choose(&mut s, "opt-todo"));
    // 誤答は外れで、受付済みのまま達成にならない。
    assert_eq!(status(&s, ST2), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), 0);
    obs.extend(choose(&mut s, "opt-memo"));
    // 確認の操作で達成になる。しかし最初の回答は誤答なので自力ではない（3 事象の分離）。
    assert_eq!(achieved_count(s.log()), 1);
    assert_eq!(status(&s, ST2), StepStatus::Achieved);
    let r = record(&obs, CheckMethod::Answer);
    assert_eq!(
        (r.first_answer, r.reselection, r.after_comparison),
        (FirstAnswer::Wrong, Reselection::Correct, false)
    );
    let t = self_tally(&obs);
    assert_eq!((t.self_reliant, t.reselected_without_comparison), (0, 1));
}

#[test]
fn reselection_after_comparison_is_recorded_apart() {
    let mut s = accepted_session();
    let mut obs = Vec::new();
    obs.extend(choose(&mut s, "opt-todo"));
    // 比較の表示に対応するイベントが無いため、観測を直接差し込む。達成には影響しない。
    obs.push(comparison());
    obs.extend(choose(&mut s, "opt-memo"));
    assert_eq!(achieved_count(s.log()), 1);
    assert!(record(&obs, CheckMethod::Answer).after_comparison);
    let t = self_tally(&obs);
    assert_eq!((t.self_reliant, t.reselected_after_comparison), (0, 1));
    assert_eq!(t.reselected_without_comparison, 0);
}

#[test]
fn wrong_without_reselection_stays_accepted_and_not_correct() {
    let mut s = accepted_session();
    let obs: Vec<_> = choose(&mut s, "opt-todo").into_iter().collect();
    assert_eq!(status(&s, ST2), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), 0);
    let t = self_tally(&obs);
    assert_eq!((t.not_correct, t.self_reliant), (1, 0));
}

#[test]
fn hint_stage_is_recorded_but_does_not_change_current_tally() {
    // ヒントの段と自力の組み合わせは GUIDE-18 で未決（TASK-304 の決定待ち）。現行の `tally` は `max_hint` を
    // 読まないため、段によらず結果が同じことだけを断定し、新しい規則は作らない。
    let mut baseline = None;
    for h in 0..=3u32 {
        let mut s = accepted_session();
        let mut obs: Vec<_> = (1..=h).map(hint).collect();
        obs.extend(choose(&mut s, "opt-memo"));
        assert_eq!(achieved_count(s.log()), 1, "h={h}");
        assert_eq!(record(&obs, CheckMethod::Answer).max_hint, h);
        let t = self_tally(&obs);
        assert_eq!(*baseline.get_or_insert(t), t, "h={h}");
    }
}

#[test]
fn answers_after_achievement_do_not_rewrite_first_answer() {
    let mut s = accepted_session();
    let obs: Vec<_> = choose(&mut s, "opt-memo").into_iter().collect();
    // 達成の後は st-2 が `Accepted` でないため、st-2 向けの選択は観測にならない。
    assert_eq!(choose(&mut s, "opt-todo"), None);
    assert_eq!(achieved_count(s.log()), 1);
    assert_eq!(
        record(&obs, CheckMethod::Answer).first_answer,
        FirstAnswer::Correct
    );
    assert_eq!(self_tally(&obs).self_reliant, 1);
}

/// 自然文を受け付け、承認で達成にする。受付の段階と達成の段階の達成件数・最終の状態を返す。
fn run_text(text: &str) -> (usize, usize, StepStatus) {
    let mut s = Session::from_state(text_ir(), entered(ST2));
    s.apply(Op::Input(Input::Text(text.into())));
    assert_eq!(status(&s, ST2), StepStatus::Accepted, "{text:?}");
    let after_accept = achieved_count(s.log());
    s.apply(Op::Input(Input::Approval {
        decision: Decision::Approve,
        target: None,
    }));
    (after_accept, achieved_count(s.log()), status(&s, ST2))
}

#[test]
fn free_text_accept_alone_is_not_achievement() {
    let (after_accept, _, _) = run_text("メモです");
    assert_eq!(after_accept, 0);
}

#[test]
fn free_text_content_quality_does_not_enter_lo_judgment() {
    let short = "ok!";
    let long = "一行目の長い説明。\n二行目も続く。\n".repeat(5);
    assert_eq!(run_text(short), run_text(&long));
    assert_eq!(run_text(short), (0, 1, StepStatus::Achieved));
    // 文の内容は観測にならない（回答として数えるのは選択だけ）ので、同じ対象から導く記録は両実行で一致する。
    let obs: Vec<CheckObservation> = Vec::new();
    let a = record(&obs, CheckMethod::Approve);
    let b = record(&obs, CheckMethod::Approve);
    assert_eq!(a, b);
    assert_eq!(a.first_answer, FirstAnswer::Unanswered);
}
