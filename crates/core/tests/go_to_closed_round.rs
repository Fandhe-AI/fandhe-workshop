//! 閉じた周回の Step への移動 `Control::GoTo` の拒否の境界の試験（TASK-249、MOD-15・D-24）。
//!
//! 手で組んだ小さな IR（章 `ch-a`・`ch-b` の呼び出しの Step `main/a1`・`main/b1` が、実体 `m@a`・`m@b` を呼ぶ）を使う。
//! 教材 1 本分の受け入れは pack の `round_closed_goto`（`mod15_*`）が持つ。変異は `MOD15-1`〜`MOD15-3`。

mod common;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Stamp, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseNextAction, RefuseReason};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::{apply, load_ir, SessionState};
use serde_json::{json, Value};

fn node(base: &Value, scope: &str, unit: &str, sid: &str, enter: Option<&str>) -> Value {
    let mut n = base.clone();
    n["uid"] = json!(format!("{scope}/{sid}"));
    n["scope"] = json!(scope);
    n["unit"] = json!(unit);
    n["step"]["id"] = json!(sid);
    n["step"]["next"] = json!([]);
    if let Some(to) = enter {
        n["enter"] = json!(to);
    }
    n
}

/// 実体の一覧を差し替えた IR。`caller` の欠落・不在・巡回で錨が決まらない場面にも使う。
fn ir_with(instances: Value) -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let bytes = std::fs::read(path).expect("read min-3step");
    let mut v: Value = serde_json::from_slice(&bytes).expect("json");
    let base = v["nodes"][0].clone();
    v["nodes"] = json!([
        node(&base, "main", "ch-x", "x", None),
        node(&base, "main", "ch-a", "a0", None),
        node(&base, "main", "ch-a", "a1", Some("m@a/s1")),
        node(&base, "main", "ch-b", "b1", Some("m@b/s1")),
        node(&base, "m@a", "m", "s1", None),
        node(&base, "m@a", "m", "s2", None),
        node(&base, "m@b", "m", "s1", None),
    ]);
    v["instances"] = instances;
    v["starts"] = json!([
        {"id": "s-a", "node": "main/a1", "effects": []},
        {"id": "s-b", "node": "main/b1", "effects": []},
    ]);
    let bytes = serde_json::to_vec(&v).expect("serialize");
    load_ir(&bytes).expect("load_ir")
}

fn ir() -> Ir {
    ir_with(json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@a", "module": "m", "args": {}, "caller": "main/a1"},
        {"id": "m@b", "module": "m", "args": {}, "caller": "main/b1"},
    ]))
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn go(ir: &Ir, st: SessionState, to: &str) -> (SessionState, Vec<Event>) {
    apply(ir, st, Op::Control(Control::GoTo(uid(to))))
}

fn begin(ir: &Ir, st: SessionState, id: &str) -> SessionState {
    let (st, ev) = apply(ir, st, Op::Control(Control::Begin(id.to_string())));
    assert!(
        ev.iter().any(|e| matches!(e, Event::Begun { .. })),
        "{ev:?}"
    );
    st
}

fn start_state() -> SessionState {
    let mut st = SessionState::default();
    st.run.current = Some(uid("main/x"));
    st
}

fn is_moved(ev: &[Event]) -> bool {
    ev.iter().any(|e| matches!(e, Event::Moved { .. }))
}

fn is_closed_refusal(ev: &[Event]) -> bool {
    matches!(
        ev,
        [Event::Refused {
            op: ControlKind::GoTo,
            reason: RefuseReason::ClosedRound,
            rejection: None,
            ..
        }]
    )
}

#[test]
fn goto_to_closed_round_is_refused_without_change() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let before = begin(&ir, st, "s-b");
    let (after, ev) = go(&ir, before.clone(), "m@a/s1");
    assert!(is_closed_refusal(&ev), "{ev:?}");
    assert_eq!(
        ev.first(),
        Some(&Event::Refused {
            at: Stamp {
                generation: before.run.generation,
                step: before.run.current.clone().expect("current"),
            },
            op: ControlKind::GoTo,
            reason: RefuseReason::ClosedRound,
            rejection: None,
        })
    );
    assert_eq!(
        RefuseReason::ClosedRound.next_action(),
        Some(RefuseNextAction::ContinueCurrentRound)
    );
    assert_eq!(after.run, before.run);
    assert_eq!(after.entry, before.entry);
    assert_eq!(after.checkpoints, before.checkpoints);
    assert_eq!(after.drafts, before.drafts);
    assert_eq!(after.hints_opened, before.hints_opened);
    assert_eq!(after.log.len(), before.log.len() + 1);
    // 呼び出しの Step への移動も同じ
    let (after, ev) = go(&ir, before.clone(), "main/a1");
    assert!(is_closed_refusal(&ev), "{ev:?}");
    assert_eq!(after.run, before.run);
}

#[test]
fn goto_via_main_step_cannot_reenter_closed_round() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let st = begin(&ir, st, "s-b");
    // main の Step は閉鎖対象外なので移れるが、そこから閉じた周回へは戻れない
    let (st, ev) = go(&ir, st, "main/x");
    assert!(is_moved(&ev), "{ev:?}");
    let before = st.clone();
    let (after, ev) = go(&ir, st, "m@a/s1");
    assert!(is_closed_refusal(&ev), "{ev:?}");
    assert_eq!(after.run, before.run);
    assert_eq!(after.entry, before.entry);
    let (_, ev) = go(&ir, before, "main/a1");
    assert!(is_closed_refusal(&ev), "{ev:?}");
}

#[test]
fn goto_via_same_chapter_main_step_cannot_reenter_closed_round() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let st = begin(&ir, st, "s-b");
    // 閉じた周回 a と同じ章の main の通常 Step へは移れるが、そこから a の実体へは戻れない
    let (st, ev) = go(&ir, st, "main/a0");
    assert!(is_moved(&ev), "{ev:?}");
    let before = st.clone();
    let (after, ev) = go(&ir, st, "m@a/s1");
    assert!(is_closed_refusal(&ev), "{ev:?}");
    assert_eq!(after.run, before.run);
    assert_eq!(after.entry, before.entry);
}

#[test]
fn goto_within_same_round_still_moves() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let st = begin(&ir, st, "s-b");
    // 現在の周回（b）の中
    let (_, ev) = go(&ir, st, "m@b/s1");
    assert!(is_moved(&ev));
    // a だけを開始した後の a の中
    let st = begin(&ir, start_state(), "s-a");
    let (_, ev) = go(&ir, st, "m@a/s2");
    assert!(is_moved(&ev));
}

#[test]
fn goto_back_before_later_round_begins_still_moves() {
    let ir = ir();
    // Begin を経ずに GoTo で後の周回へ入り、前の周回へ戻る
    let (st, ev) = go(&ir, start_state(), "m@b/s1");
    assert!(is_moved(&ev));
    let (_, ev) = go(&ir, st, "m@a/s1");
    assert!(is_moved(&ev));
}

#[test]
fn goto_to_plain_main_step_is_not_closed() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let st = begin(&ir, st, "s-b");
    let (_, ev) = go(&ir, st, "main/x");
    assert!(is_moved(&ev));
}

#[test]
fn unresolvable_anchor_falls_back_to_plain_move() {
    // caller が無い・存在しない・互いを呼ぶ巡回でも止まり、従来の移動になる
    let variants = [
        json!([
            {"id": "main", "module": null, "args": {}, "caller": null},
            {"id": "m@a", "module": "m", "args": {}},
            {"id": "m@b", "module": "m", "args": {}},
        ]),
        json!([
            {"id": "main", "module": null, "args": {}, "caller": null},
            {"id": "m@a", "module": "m", "args": {}, "caller": "nowhere/x"},
            {"id": "m@b", "module": "m", "args": {}, "caller": "nowhere/y"},
        ]),
        json!([
            {"id": "main", "module": null, "args": {}, "caller": null},
            {"id": "m@a", "module": "m", "args": {}, "caller": "m@b/s1"},
            {"id": "m@b", "module": "m", "args": {}, "caller": "m@a/s1"},
        ]),
    ];
    for instances in variants {
        let ir = ir_with(instances);
        let mut st = SessionState::default();
        st.run.current = Some(uid("m@b/s1"));
        st.log.push(Event::Begun {
            at: Stamp {
                generation: st.run.generation,
                step: uid("m@b/s1"),
            },
            start: "s-b".to_string(),
            new_generation: st.run.generation,
        });
        let (_, ev) = go(&ir, st, "m@a/s1");
        assert!(is_moved(&ev), "{ev:?}");
    }
}

#[test]
fn restore_to_closed_round_checkpoint_still_works_and_moves_inside_are_allowed() {
    let ir = ir();
    let st = begin(&ir, start_state(), "s-a");
    let (st, ev) = go(&ir, st, "m@a/s2");
    assert!(is_moved(&ev));
    let (st, _) = apply(&ir, st, Op::Control(Control::Save));
    let st = begin(&ir, st, "s-b");
    let gen_before = st.run.generation;
    let (st, ev) = apply(&ir, st, Op::Control(Control::Restore(CheckpointId(1))));
    assert!(
        ev.iter().any(|e| matches!(e, Event::Restored { .. })),
        "{ev:?}"
    );
    assert!(st.run.generation > gen_before);
    // 復帰の後、前の周回の中の移動は許す
    let (st, ev) = go(&ir, st, "m@a/s1");
    assert!(is_moved(&ev), "{ev:?}");
    // 後の周回へ進む移動も許す
    let (_, ev) = go(&ir, st, "m@b/s1");
    assert!(is_moved(&ev));
}
