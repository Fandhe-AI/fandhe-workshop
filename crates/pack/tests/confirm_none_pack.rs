//! 確認を省く明示の Step の、受付と同時の達成と静的検査の組の試験（TASK-208、EVT-5）。教材テストに当たる。
//!
//! `ch1-pack` の `st-1-2`・`st-1-3`（`confirm: none` と理由）をコンパイルした IR を core の `Session` で動かし、
//! 受付の 1 回の操作で達成になって次の Step に進むことを確かめる。明示の無い Step（`st-1-1` の `point_at`、
//! `confirm` の欠落、理由の欠けた `none`）は、静的検査が `E-NO-CONFIRM` で報告し、実行では受付と同時に
//! 達成にならないことを組にして確かめる。台本の実行器 `fandhe-pack test`（TASK-404）は未実装のため、
//! 本試験がその代わりの証拠である。

mod common;

use common::{ch1, copy_dir, Tmp};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;
use serde_json::{json, Value};

const CHAPTER: &str = "chapters/ch01/chapter.yaml";
const REASON_LINE: &str =
    "      reason: 補完して送った結果が左のカードにそのまま出るため、別の確認を置かない。\n";

fn ir_of(dir: &std::path::Path) -> Ir {
    load_ir(&compile_dir(dir).expect("compile")).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn entered(step: &str) -> Session {
    session_in(ir_of(&ch1()), step)
}

fn session_in(ir: Ir, step: &str) -> Session {
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    Session::from_state(
        ir,
        SessionState {
            run,
            ..SessionState::default()
        },
    )
}

fn send(s: &mut Session, input: Input) -> Vec<Event> {
    s.apply(Op::Input(input)).to_vec()
}

fn achieved(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
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

fn set_confirm(ir: &mut Ir, step: &str, confirm: Option<Value>) {
    let n = ir
        .nodes
        .iter_mut()
        .find(|n| n.uid == step)
        .expect("node exists");
    n.step.confirm = confirm;
}

#[test]
fn the_two_explicit_skip_steps_achieve_on_accept() {
    let mut total = 0;
    let mut s = entered("main/st-1-2");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert!(ev.iter().any(|e| matches!(e, Event::Accepted { .. })));
    assert_eq!(status(&s, "main/st-1-2"), StepStatus::Achieved);
    assert_eq!(s.state().run.current, Some(uid("main/st-1-3")));
    total += achieved(&ev);

    let mut s = entered("main/st-1-3");
    let ev = send(&mut s, Input::Text("one\ntwo".into()));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(status(&s, "main/st-1-3"), StepStatus::Achieved);
    assert_eq!(s.state().run.current, Some(uid("main/st-1-4")));
    total += achieved(&ev);

    assert_eq!(total, 2);
}

#[test]
fn point_at_step_stops_at_accepted_until_confirmed() {
    let mut s = entered("main/st-1-1");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 0, "{ev:?}");
    assert_eq!(status(&s, "main/st-1-1"), StepStatus::Accepted);
    let ev = send(
        &mut s,
        Input::Ui {
            view: "v-cards".into(),
            path: "cards/help-1".into(),
        },
    );
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(s.state().run.current, Some(uid("main/st-1-2")));
}

/// `st-1-2` の確認を書き換えた複製を作る。
fn ch1_with(replacement: &str) -> Tmp {
    let t = Tmp::new("none-pack");
    copy_dir(&ch1(), &t.0);
    let path = t.0.join(CHAPTER);
    let text = std::fs::read_to_string(&path).expect("read chapter");
    assert_eq!(text.matches(REASON_LINE).count(), 1, "anchor line");
    std::fs::write(&path, text.replace(REASON_LINE, replacement)).expect("write chapter");
    t
}

fn no_confirm_count(d: &[fandhe_workshop_pack::Diagnostic]) -> usize {
    d.iter()
        .filter(|x| matches!(x.code(), DiagCode::NoConfirm))
        .count()
}

#[test]
fn explicit_reason_passes_check_and_achieves_on_accept() {
    let t = ch1_with(REASON_LINE);
    assert_eq!(no_confirm_count(&validate_dir(&t.0)), 0);
    let mut s = session_in(ir_of(&t.0), "main/st-1-2");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
}

#[test]
fn empty_reason_is_reported_and_not_achieved_at_runtime() {
    let t = ch1_with("      reason: ''\n");
    assert_eq!(no_confirm_count(&validate_dir(&t.0)), 1);
    // コンパイルも拒否するため、実行は IR の写しで確かめる（検査を通っていない IR の fail-closed）。
    assert!(compile_dir(&t.0).is_err());
    let mut ir = ir_of(&ch1());
    set_confirm(
        &mut ir,
        "main/st-1-2",
        Some(json!({ "kind": "none", "reason": "" })),
    );
    let mut s = session_in(ir, "main/st-1-2");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 0, "{ev:?}");
    assert!(ev.iter().any(|e| matches!(e, Event::Warning { .. })));
    assert_eq!(status(&s, "main/st-1-2"), StepStatus::Entered);
}

#[test]
fn missing_confirm_is_reported_and_not_achieved_on_accept() {
    let t = ch1_with("");
    // `kind: none` の行だけが残り、理由の無い宣言として報告される。
    assert_eq!(no_confirm_count(&validate_dir(&t.0)), 1);

    // 宣言そのものを外した IR の写し。`approve` 扱いで、受付と同時には達成にならない。
    let mut ir = ir_of(&ch1());
    set_confirm(&mut ir, "main/st-1-2", None);
    let mut s = session_in(ir, "main/st-1-2");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 0, "{ev:?}");
    assert_eq!(status(&s, "main/st-1-2"), StepStatus::Accepted);

    let mut ir = ir_of(&ch1());
    set_confirm(&mut ir, "main/st-1-2", Some(json!({ "kind": "approve" })));
    let mut s = session_in(ir, "main/st-1-2");
    let ev = send(&mut s, Input::Command("/help".into()));
    assert_eq!(achieved(&ev), 0, "{ev:?}");
}
