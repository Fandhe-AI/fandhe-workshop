//! 閉じた周回の Step への移動の拒否の受け入れ試験（TASK-249、MOD-15・D-24。記録は `docs/testing.md` の 5.418）。
//!
//! `module-reuse-rounds` を `compile_dir` した IR で、2 周目を開始した後に 1 周目の Step へ `Control::GoTo` で戻ろうとすると、
//! 理由（`ClosedRound`）と次の行動を付けて拒否され、位置・世代・世界状態・切り身・入口の写し・チェックポイント・下書き・ヒントが
//! 変わらないことを確かめる。閉じていない周回への移動と、チェックポイントへの復帰は従来どおり動く。再読み込み・書き出し→読み込み・
//! 履歴の適用し直しのどれで開き直しても、同じ移動が同じ拒否になる。
//! 規則の境界は core の `go_to_closed_round` が受け持つ。変異 `MOD15-4` の検出先になる。
//! 入力列は `common/rounds.rs` の計画を使い、未実装の部品のため `GoTo` で飛ばす箇所がある（5.258・5.260）。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseNextAction, RefuseReason};
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_session;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::instance::owner_of;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{
    begin_round2_plan, round1_plan, round2_plan, round_roots, run_plan, step_in, steps, PlanStep,
};

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn versions(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

fn recorded(ir: &Ir) -> RecordedSession {
    RecordedSession::new(
        Session::from_state(ir.clone(), SessionState::default()),
        RecordedSession::first_start(ir),
    )
}

fn control(c: Control) -> PlanStep {
    PlanStep::Op(Op::Control(c))
}

/// 1 周目（呼び出しの Step を含む）の移動先すべて。実体の表から引き、uid を直書きしない。
fn round1_targets(ir: &Ir) -> Vec<StepUid> {
    let (r1, _) = round_roots(ir, "s-r1");
    let mut targets = steps(ir, &r1);
    for i in &ir.instances {
        if r1.contains(&Root(i.id.clone())) {
            targets.push(StepUid(i.caller.clone().expect("caller")));
        }
    }
    assert!(targets.len() > 4, "targets must cover both instances");
    targets
}

/// 1 周目を流し、`Begin` の前の 1 周目の `st-2-1` で cp1 を保存し、2 周目を開始して入力列を流し、cp2 を保存する。
/// 戻り値は手動の保存の ID（cp1・cp2）つき。章の終わりの自動保存が ID を使うため、番号を直書きしない（TASK-339 1/2）。
fn two_rounds(ir: &Ir) -> (RecordedSession, CheckpointId, CheckpointId) {
    let mut rs = recorded(ir);
    run_plan(&mut rs, &round1_plan(ir));
    let (r1, _) = round_roots(ir, "s-r1");
    rs.apply(Op::Control(Control::GoTo(step_in(ir, &r1, "st-2-1"))));
    rs.apply(Op::Control(Control::Save));
    let cp1 = CheckpointId(rs.session().state().next_checkpoint.0 - 1);
    let mut plan = begin_round2_plan();
    plan.extend(round2_plan(ir));
    plan.push(control(Control::Save));
    run_plan(&mut rs, &plan);
    let cp2 = CheckpointId(rs.session().state().next_checkpoint.0 - 1);
    (rs, cp1, cp2)
}

fn assert_closed_refusal(events: &[Event]) {
    assert!(
        matches!(
            events,
            [Event::Refused {
                op: ControlKind::GoTo,
                reason: RefuseReason::ClosedRound,
                rejection: None,
                ..
            }]
        ),
        "{events:?}"
    );
}

/// 状態の全欄が操作の前と同じで、ログだけが 1 件増えている。
fn assert_unchanged_but_log(before: &SessionState, after: &SessionState) {
    assert_eq!(after.run, before.run);
    assert_eq!(after.entry, before.entry);
    assert_eq!(after.checkpoints, before.checkpoints);
    assert_eq!(after.next_checkpoint, before.next_checkpoint);
    assert_eq!(after.drafts, before.drafts);
    assert_eq!(after.hints_opened, before.hints_opened);
    assert_eq!(after.log.len(), before.log.len() + 1);
    assert_eq!(
        after.log.get(..before.log.len()),
        Some(before.log.as_slice())
    );
}

#[test]
fn mod15_goto_to_closed_round1_step_is_refused_without_change() {
    let ir = ir();
    let (mut rs, _, _) = two_rounds(&ir);
    rs.draft("下書き".to_string());
    for to in round1_targets(&ir) {
        let before = rs.session().state().clone();
        let events = rs.apply(Op::Control(Control::GoTo(to.clone()))).to_vec();
        assert_closed_refusal(&events);
        assert_unchanged_but_log(&before, rs.session().state());
        assert!(!events.iter().any(|e| matches!(e, Event::Changed { .. })));
    }
    assert_eq!(
        RefuseReason::ClosedRound.next_action(),
        Some(RefuseNextAction::ContinueCurrentRound)
    );
}

#[test]
fn mod15_goto_within_open_rounds_still_moves() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    // `Begin` の前の 1 周目の中の移動
    let mut rs = recorded(&ir);
    run_plan(&mut rs, &round1_plan(&ir));
    let t = step_in(&ir, &r1, "st-2-1");
    let events = rs.apply(Op::Control(Control::GoTo(t.clone()))).to_vec();
    assert!(events.iter().any(|e| matches!(e, Event::Moved { .. })));
    assert_eq!(rs.session().state().run.current.as_ref(), Some(&t));
    // 2 周目の中の別の Step
    run_plan(&mut rs, &begin_round2_plan());
    let t2 = step_in(&ir, &r2, "st-2-1");
    let events = rs.apply(Op::Control(Control::GoTo(t2.clone()))).to_vec();
    assert!(events.iter().any(|e| matches!(e, Event::Moved { .. })));
    assert_eq!(rs.session().state().run.current.as_ref(), Some(&t2));
    let t3 = step_in(&ir, &r2, "st-3-2");
    let events = rs.apply(Op::Control(Control::GoTo(t3.clone()))).to_vec();
    assert!(events.iter().any(|e| matches!(e, Event::Moved { .. })));
}

#[test]
fn mod15_restore_still_works_after_refusal() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (mut rs, cp1, cp2) = two_rounds(&ir);
    let to = step_in(&ir, &r1, "st-3-2");
    let events = rs.apply(Op::Control(Control::GoTo(to))).to_vec();
    assert_closed_refusal(&events);
    let r2_owner = owner_of(&ir, rs.session().state().run.current.as_ref().expect("cur"));
    for id in [cp1, cp2, cp1, cp2] {
        let events = rs.apply(Op::Control(Control::Restore(id))).to_vec();
        assert!(
            events.iter().any(|e| matches!(e, Event::Restored { .. })),
            "restore {id:?}: {events:?}"
        );
    }
    // 最後は cp2（2 周目）。持ち主の根が 2 周目の側にある。
    let cur = rs.session().state().run.current.clone().expect("current");
    assert_eq!(owner_of(&ir, &cur), r2_owner);
}

fn boot(ir: &Ir, bytes: &[u8]) -> RecordedSession {
    let outcome = boot_session(
        Some(bytes),
        ir.clone(),
        &versions(ir),
        &OpeningCaps::current(),
        SessionState::default(),
        &RecordedSession::first_start(ir),
    );
    let BootOutcome::Restored { session, .. } = outcome else {
        panic!("history must be restored");
    };
    session
}

/// 開き直した状態が、保存した状態と位置・世界・切り身・下書き・チェックポイントで一致する。
fn assert_same_progress(a: &SessionState, b: &SessionState) {
    assert_eq!(a.run.current, b.run.current);
    assert_eq!(a.run.world, b.run.world);
    assert_eq!(a.run.slices, b.run.slices);
    assert_eq!(a.drafts, b.drafts);
    assert_eq!(a.checkpoints, b.checkpoints);
}

#[test]
fn mod15_refusal_reproduces_after_reload() {
    let ir = ir();
    let (mut rs, _, _) = two_rounds(&ir);
    let to = step_in(&ir, &round_roots(&ir, "s-r1").0, "st-3-2");
    let ops_before = rs.ops().len();
    let events = rs.apply(Op::Control(Control::GoTo(to.clone()))).to_vec();
    assert_closed_refusal(&events);
    assert_eq!(
        rs.ops().len(),
        ops_before + 1,
        "the refused GoTo is recorded"
    );
    let saved = rs.session().state().clone();
    let env = rs.envelope_for_save().expect("envelope");

    // (i) 履歴の適用し直し
    let replayed = replay(&ir, SessionState::default(), &env.ops).expect("replay");
    assert_eq!(replayed, saved);

    // (ii) 起動の分岐での再読み込み、(iii) 書き出し → 読み込み
    let bytes = export_bytes(&env).expect("export");
    let booted = boot(&ir, &bytes);
    assert_same_progress(&saved, booted.session().state());
    let loaded = load_session(
        &bytes,
        ir.clone(),
        &versions(&ir),
        &OpeningCaps::current(),
        SessionState::default(),
    )
    .unwrap_or_else(|v| panic!("history must open: {v:?}"));
    assert_same_progress(&saved, loaded.session.state());

    for mut reopened in [
        booted,
        RecordedSession::new(loaded.session, loaded.start.clone()),
        RecordedSession::new(
            Session::from_state(ir.clone(), replayed),
            RecordedSession::first_start(&ir),
        ),
    ] {
        let before = reopened.session().state().clone();
        let events = reopened
            .apply(Op::Control(Control::GoTo(to.clone())))
            .to_vec();
        assert_closed_refusal(&events);
        assert_unchanged_but_log(&before, reopened.session().state());
    }

    // 再読み込み（世代 +1）の後も拒否される
    rs.reload();
    let before = rs.session().state().clone();
    let events = rs.apply(Op::Control(Control::GoTo(to))).to_vec();
    assert_closed_refusal(&events);
    assert_unchanged_but_log(&before, rs.session().state());
}

#[test]
fn mod15_fresh_round2_start_also_closes_round1() {
    let ir = ir();
    let mut rs = recorded(&ir);
    run_plan(
        &mut rs,
        &[PlanStep::Op(Op::Control(Control::Begin(
            "s-r2-fresh".into(),
        )))],
    );
    let to = step_in(&ir, &round_roots(&ir, "s-r1").0, "st-2-1");
    let before = rs.session().state().clone();
    let events = rs.apply(Op::Control(Control::GoTo(to))).to_vec();
    assert_closed_refusal(&events);
    assert_unchanged_but_log(&before, rs.session().state());
}
