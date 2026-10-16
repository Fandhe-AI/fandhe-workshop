//! `Restored` による復習・変化の印の作り直しの試験（TASK-214 2/3、仕様 EVT-12・EVT-14、性質 P-1a・P-1c・P-1d・C-1）。
//!
//! `projection_derive.rs` は手で組んだイベント列で `Projection` を動かす。本ファイルは `apply()` が実際に出す
//! ログ（保存・復帰・停止・移動・確認を含む）で、復帰の後の復習が保存の時点（`Restored.log_len` までのイベント）の
//! 復習と一致すること、復帰の直後の変化の印が空であること、ログの並びと位置が書き出し・読み込みで変わらないことを
//! 確かめる。呼び出し元は CI の `make test` のみ。変異注入 `mutations/EVT12-9`〜`EVT12-12` が検出する。
//!
//! 範囲外: 複数の根の C-1 は `projection_derive.rs` が持つ。実体ごとの戻り方（MOD-9）は TASK-241。
//! 復帰前の進行の無視・処理中の再開は TASK-214 3/3。書き出し・読み込みの実装そのものは TASK-330
//! （ここでは参照の serde の往復を使う）。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::history_sample::{
    check_log_positions, check_roundtrip, load as load_history, reference_export, reference_import,
};
use common::property::{check, op_sequence, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::projection::{Projection, ReviewItem};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{apply, load_ir};
use proptest::prelude::*;

fn ir() -> Ir {
    let s = load();
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("read ir");
    load_ir(&bytes).expect("load ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn first_note() -> Op {
    Op::Input(Input::Command("/make-note first-note".into()))
}

fn save() -> Op {
    Op::Control(Control::Save)
}

fn restore(n: u32) -> Op {
    Op::Control(Control::Restore(CheckpointId(n)))
}

fn point_at() -> Op {
    Op::Input(Input::Ui {
        view: "v-files".into(),
        path: "notes/draft".into(),
    })
}

fn tick(ir: &Ir, state: SessionState) -> SessionState {
    match next_tick(&state) {
        Some(addr) => apply(ir, state, Op::Tick(addr)).0,
        None => state,
    }
}

fn tick_n(ir: &Ir, mut state: SessionState, n: u32) -> SessionState {
    for _ in 0..n {
        state = tick(ir, state);
    }
    state
}

/// 入口の写しを持つ初期状態から受付を行い、`ticks` 回進めた状態。
fn busy_state(ir: &Ir, ticks: u32) -> SessionState {
    tick_n(ir, apply(ir, load().initial, first_note()).0, ticks)
}

/// 処理が終わるまで進めて確認し、st-1 を達成させる。
fn achieve_st1(ir: &Ir, mut state: SessionState) -> SessionState {
    while state.run.busy.is_some() {
        state = tick(ir, state);
    }
    apply(ir, state, point_at()).0
}

fn review_of(state: &SessionState) -> Vec<ReviewItem> {
    Projection::from_events(&state.log).review().to_vec()
}

/// ログの先頭 `log_len` 件から導いた復習（保存の時点の復習）。範囲外なら試験を失敗させる。
fn review_at_save(log: &[Event], log_len: u32) -> Vec<ReviewItem> {
    let prefix = log
        .get(..log_len as usize)
        .expect("log_len must be within the log");
    Projection::from_events(prefix).review().to_vec()
}

fn restored_len(events: &[Event]) -> Option<u32> {
    events.iter().find_map(|e| match e {
        Event::Restored { log_len, .. } => Some(*log_len),
        _ => None,
    })
}

fn saved_len(events: &[Event]) -> Option<u32> {
    events.iter().find_map(|e| match e {
        Event::CheckpointSaved { log_len, .. } => Some(*log_len),
        _ => None,
    })
}

#[test]
fn evt14_review_after_restore_matches_saved_review() {
    let ir = ir();
    let (saved, save_events) = apply(&ir, busy_state(&ir, 3), save());
    let log_len = saved_len(&save_events).expect("saved");
    let saved_review = review_of(&saved);
    // 保存の後で復習を変える（処理を終えて確認し、st-1 を達成させる）。
    let advanced = achieve_st1(&ir, saved);
    assert_ne!(
        review_of(&advanced),
        saved_review,
        "scene must diverge from the save"
    );
    let (next, events) = apply(&ir, advanced, restore(1));
    assert_eq!(restored_len(&events), Some(log_len));
    assert!(matches!(
        next.log.get(log_len as usize),
        Some(Event::CheckpointSaved {
            id: CheckpointId(1),
            ..
        })
    ));
    assert_eq!(review_of(&next), saved_review);
    assert_eq!(review_of(&next), review_at_save(&next.log, log_len));
}

#[test]
fn evt14_restore_after_stop_brings_back_review() {
    let ir = ir();
    let (saved, save_events) = apply(&ir, busy_state(&ir, 2), save());
    let log_len = saved_len(&save_events).expect("saved");
    let saved_review = review_of(&saved);
    assert!(!saved_review.is_empty(), "saved scene must have an item");
    let stopped = apply(&ir, saved, Op::Control(Control::Stop)).0;
    assert_ne!(review_of(&stopped), saved_review, "stop resets the item");
    let next = apply(&ir, stopped, restore(1)).0;
    assert_eq!(review_of(&next), saved_review);
    assert_eq!(review_of(&next), review_at_save(&next.log, log_len));
}

#[test]
fn evt14_restore_same_checkpoint_twice_and_nested() {
    let ir = ir();
    // 保存 A -> 進行 -> 復帰 A -> 保存 B -> 達成 -> 復帰 A -> 復帰 B。
    let mut s = apply(&ir, busy_state(&ir, 1), save()).0;
    s = tick(&ir, s);
    let ops: [Op; 4] = [restore(1), save(), restore(1), restore(2)];
    for (i, op) in ops.into_iter().enumerate() {
        if i == 2 {
            s = achieve_st1(&ir, s);
        }
        let (next, events) = apply(&ir, s, op);
        if let Some(log_len) = restored_len(&events) {
            assert_eq!(
                review_of(&next),
                review_at_save(&next.log, log_len),
                "step {i}"
            );
            assert_eq!(check_log_positions(&next), Ok(()), "positions at {i}");
        }
        s = next;
    }
}

#[test]
fn p1c_marks_reset_on_restore() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 2), save()).0;
    // 確認で Step を移すと入口が基準点になり印が消える。印が残る形（処理を終えただけ）で復帰する。
    let mut advanced = saved;
    while advanced.run.busy.is_some() {
        advanced = tick(&ir, advanced);
    }
    let before = Projection::from_events(&advanced.log);
    assert!(
        !before.marks().is_empty(),
        "scene must have marks before the restore"
    );
    let next = apply(&ir, advanced, restore(1)).0;
    let after = Projection::from_events(&next.log);
    assert!(after.marks().is_empty());
}

#[test]
fn p1a_incremental_matches_batch_across_restore() {
    let ir = ir();
    let mut s = busy_state(&ir, 2);
    let mut inc = Projection::new();
    let ops = [
        save(),
        Op::Control(Control::Stop),
        restore(1),
        first_note(),
        restore(1),
    ];
    for op in ops {
        s = apply(&ir, s, op).0;
        inc.extend(&s.log);
        assert_eq!(inc, Projection::from_events(&s.log));
    }
    // 1 件ずつの増分も一括と一致する。
    let mut one = Projection::new();
    for k in 1..=s.log.len() {
        one.extend(s.log.get(..k).expect("prefix"));
    }
    assert_eq!(one, Projection::from_events(&s.log));
}

/// 状態の包みを組み立て、書き出し・読み込みの往復で並び・位置・導出が変わらないことを確かめる。
fn check_export_import(ir: &Ir, state: &SessionState) -> Result<(), String> {
    check_log_positions(state)?;
    let start = load_history("empty").start;
    let envelope = build_envelope(ir, &start, Vec::new(), state.clone())
        .map_err(|e| format!("build: {e:?}"))?;
    check_roundtrip(
        reference_export,
        |b| reference_import(b).map_err(|e| e.to_string()),
        &envelope,
    )?;
    let back = reference_import(&reference_export(&envelope)).map_err(|e| e.to_string())?;
    if Projection::from_events(&back.snapshot.log) != Projection::from_events(&state.log) {
        return Err("projection changed by the roundtrip".into());
    }
    Ok(())
}

#[test]
fn evt14_log_order_survives_export_import() {
    let ir = ir();
    let saved = apply(&ir, busy_state(&ir, 3), save()).0;
    let advanced = achieve_st1(&ir, saved);
    let restored = apply(&ir, advanced, restore(1)).0;
    assert!(restored
        .log
        .iter()
        .any(|e| matches!(e, Event::Restored { .. })));
    assert_eq!(check_export_import(&ir, &restored), Ok(()));
}

#[derive(Debug, Clone)]
enum Act {
    Next,
    Stale,
    Resend,
    Reject,
    Choose,
    Approve,
    Decline,
    Confirm,
    Stop,
    GoTo(u8),
    Save,
    Restore(u32),
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        4 => Just(Act::Next),
        1 => Just(Act::Stale),
        2 => Just(Act::Resend),
        1 => Just(Act::Reject),
        1 => Just(Act::Choose),
        1 => Just(Act::Approve),
        1 => Just(Act::Decline),
        2 => Just(Act::Confirm),
        1 => Just(Act::Stop),
        2 => (1u8..=3).prop_map(Act::GoTo),
        3 => Just(Act::Save),
        4 => (1u32..=4).prop_map(Act::Restore),
    ]
}

fn to_op(state: &SessionState, act: &Act) -> Option<Op> {
    Some(match act {
        Act::Next => Op::Tick(next_tick(state)?),
        Act::Stale => {
            let mut addr = next_tick(state)?;
            addr.generation.0 = addr.generation.0.checked_sub(1)?;
            Op::Tick(addr)
        }
        Act::Resend => first_note(),
        Act::Reject => Op::Input(Input::Command("/nothing".into())),
        Act::Choose => Op::Input(Input::Choice("opt-memo".into())),
        Act::Approve => Op::Input(Input::Approval {
            decision: Decision::Approve,
            target: None,
        }),
        Act::Decline => Op::Input(Input::Approval {
            decision: Decision::Reject,
            target: None,
        }),
        Act::Confirm => point_at(),
        Act::Stop => Op::Control(Control::Stop),
        Act::GoTo(n) => Op::Control(Control::GoTo(uid(&format!("main/st-{n}")))),
        Act::Save => save(),
        Act::Restore(n) => restore(*n),
    })
}

#[test]
fn c1_p1d_restore_rebuilds_review_from_saved_prefix() {
    check(
        "c1-p1d-restore-rebuilds-review-from-saved-prefix",
        DEFAULT_CASES,
        op_sequence(act_strategy()),
        |acts| {
            let ir = ir();
            let mut state = load().initial;
            let mut inc = Projection::new();
            for act in &acts {
                let Some(op) = to_op(&state, act) else {
                    continue;
                };
                let (next, events) = apply(&ir, state, op);
                state = next;
                inc.extend(&state.log);
                let batch = Projection::from_events(&state.log);
                // P-1a: 増分と一括の一致。
                prop_assert_eq!(&inc, &batch);
                if let Some(log_len) = restored_len(&events) {
                    // P-1d・C-1: 復帰の直後の復習は、保存の位置までのイベントから作った復習と一致する。
                    let expected = review_at_save(&state.log, log_len);
                    prop_assert_eq!(batch.review(), expected.as_slice());
                    // P-1c: 復帰は基準点で、直後の印は空。
                    prop_assert!(batch.marks().is_empty());
                }
                prop_assert_eq!(check_log_positions(&state), Ok(()));
            }
            prop_assert_eq!(check_export_import(&ir, &state), Ok(()));
            Ok(())
        },
    );
}
