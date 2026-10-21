//! 承認の差し戻しの再承認を、Step を分けて書く形の試験（TASK-62 1/2、PACK-32・D-69）。
//!
//! 決定（D-69、2026-10-21）は「1 Step の `approval` で承認 → 差し戻し → 再承認を表す語は足さない」である。
//! 教材は、差し戻しの Step と再承認の Step を分け、差し戻しの後の遷移を `next` に書く。本試験は、その形の
//! 実パック（`tests/data/revise-split-pack`）について次を確かめる。
//!
//! - 新しい語彙の版を宣言しなくても（`pack.vocab: "1.0"` のまま）`validate` が診断 0 件で通ること
//! - 承認の `input` が欄を持たない `{"kind":"approval"}` のままで、IR の `next` が再承認の Step を指すこと
//! - コンパイルした IR を core の `Session` で動かすと、差し戻し → 再承認の順に進み、早すぎる承認と
//!   もう一度の差し戻しが拒否されること
//! - 1 Step で再承認を表すつもりの欄は語彙ではなく、`E-PARSE` になること（語を足していないことの対照）
//!
//! 確かめないこと: 画面の表示、`fandhe-pack test` の結線、`next` の `when` による枝分かれ（core は無条件の最初の辺だけを
//! たどる）。試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::{copy_dir, Tmp};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Session, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::DiagCode;
use serde_json::Value;

const CHAPTER: &str = "chapters/c1/chapter.yaml";
const APPROVAL_INPUT: &str =
    "    input: {kind: approval}\n    rules:\n      - id: r-approve-fixed\n";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/revise-split-pack")
}

fn uid(step: &str) -> StepUid {
    StepUid(format!("main/{step}"))
}

fn session_at(step: &str) -> Session {
    let ir: Ir = load_ir(&compile_dir(&pack_dir()).unwrap()).unwrap();
    let u = uid(step);
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(u.root(), slice);
    let run = RunState {
        generation: Generation(1),
        current: Some(u),
        slices,
        ..RunState::default()
    };
    Session::from_state(
        ir,
        SessionState {
            run,
            ..SessionState::default()
        },
    )
}

fn approval(decision: Decision) -> Input {
    Input::Approval {
        decision,
        target: None,
    }
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

fn is_not_in_this_step(events: &[Event]) -> bool {
    events.iter().any(|e| match e {
        Event::Judged { judgment, .. } => matches!(
            judgment,
            Judgment::Rejected(r) if r.category == RejectCategory::NotInThisStep
        ),
        _ => false,
    })
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

fn draft_state(s: &Session) -> String {
    s.state().run.world["plan/draft"].attrs["state"].clone()
}

#[test]
fn pack32_split_pack_passes_validate_without_new_vocab() {
    let text = std::fs::read_to_string(pack_dir().join("workshop.yaml")).unwrap();
    assert!(text.contains("vocab: '1.0'"), "語彙の版は 1.0 のまま");
    let diags = validate_dir(&pack_dir());
    assert!(diags.is_empty(), "{diags:?}");
}

#[test]
fn pack32_split_pack_compiles_with_plain_approval_inputs() {
    let ir: Value = serde_json::from_slice(&compile_dir(&pack_dir()).unwrap()).unwrap();
    let node = |uid: &str| {
        ir["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["uid"] == uid)
            .unwrap()
            .clone()
    };
    let revise = node("main/st-revise");
    let reapprove = node("main/st-reapprove");
    assert_eq!(
        revise["step"]["input"],
        serde_json::json!({"kind": "approval"})
    );
    assert_eq!(
        reapprove["step"]["input"],
        serde_json::json!({"kind": "approval"})
    );
    assert_eq!(revise["step"]["next"][0]["to"], "main/st-reapprove");
    assert_eq!(reapprove["step"]["next"][0]["to"], "main/st-end");
}

#[test]
fn pack32_revise_then_reapprove_runs_in_core() {
    let mut s = session_at("st-revise");

    // 差し戻しの Step では、承認も却下もまだ受け付けない。
    for early in [Decision::Approve, Decision::Reject] {
        let ev = send(&mut s, approval(early));
        assert!(is_not_in_this_step(&ev), "{ev:?}");
        assert_eq!(achieved(&ev), 0, "{ev:?}");
        assert_eq!(s.state().run.current, Some(uid("st-revise")));
    }

    // 差し戻すと達成になり、再承認の Step へ移る。
    let ev = send(&mut s, approval(Decision::Revise));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(status(&s, "st-revise"), StepStatus::Achieved);
    assert_eq!(s.state().run.current, Some(uid("st-reapprove")));
    assert_eq!(draft_state(&s), "差し戻し");

    // 再承認の Step では、もう一度の差し戻しと却下を受け付けない。
    for again in [Decision::Revise, Decision::Reject] {
        let ev = send(&mut s, approval(again));
        assert!(is_not_in_this_step(&ev), "{ev:?}");
        assert_eq!(achieved(&ev), 0, "{ev:?}");
        assert_eq!(s.state().run.current, Some(uid("st-reapprove")));
        assert_eq!(draft_state(&s), "差し戻し");
    }

    // 承認すると達成になり、終端へ移る。
    let ev = send(&mut s, approval(Decision::Approve));
    assert_eq!(achieved(&ev), 1, "{ev:?}");
    assert_eq!(status(&s, "st-reapprove"), StepStatus::Achieved);
    assert_eq!(s.state().run.current, Some(uid("st-end")));
    assert_eq!(draft_state(&s), "承認済み");
}

#[test]
fn pack32_one_step_reapproval_field_is_not_vocabulary() {
    let tmp = Tmp::new("revise-split-field");
    copy_dir(&pack_dir(), &tmp.0);
    let path = tmp.0.join(CHAPTER);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches(APPROVAL_INPUT).count(), 1, "置換元がある");
    let with_field = text.replace(
        APPROVAL_INPUT,
        "    input: {kind: approval, reapprove: true}\n    rules:\n      - id: r-approve-fixed\n",
    );
    std::fs::write(&path, with_field).unwrap();

    let validated = validate_dir(&tmp.0);
    assert!(
        validated
            .iter()
            .any(|d| matches!(d.code(), DiagCode::Parse)),
        "{validated:?}"
    );
    let compiled = compile_dir(&tmp.0).expect_err("欄を足した教材は IR を出さない");
    assert!(
        compiled.iter().any(|d| matches!(d.code(), DiagCode::Parse)),
        "{compiled:?}"
    );
}
