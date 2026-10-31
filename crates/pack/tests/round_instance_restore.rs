//! 同じ周回の別の実体を戻さない、実体単位だけの復帰の受け入れ試験（TASK-250、MOD-16。記録は `docs/testing.md` の 5.421）。
//!
//! MOD-16 は #781 の決定（2026-10-31）で「巻き戻しは実体単位だけ。周回単位の復帰・チェックポイントは足さない」と決まった。
//! 2 周目は 2 つの実体（アイデアの周回と実装の周回）から成る。前者で保存して後者まで進み、前者のチェックポイントへ戻ると、
//! 戻るのは前者だけで、後者の進み・世界状態・呼び出しの Step の状態、1 周目の切り身は残る。周回の「完了」も実体ごとの切り身に
//! あるだけで、ひとまとまりの記録を持たない。同じ周回の 2 つの実体で別々に保存・復帰でき、再読み込み・書き出し→読み込み・
//! 履歴の適用し直しのどれで開き直しても復帰の範囲は同じである。
//! 実体単位の戻り方の一般規則（MOD-9）は `round_slice_init` の `mod9_*` と core の `restore_owner_slice` が受け持ち、
//! 本ファイルはその周回の側から見た帰結を固定する。変異 `MOD16-1`（周回単位で戻す）の検出先になる。
//! 入力列は `common/rounds.rs` の計画を使い、未実装の部品のため `GoTo` で飛ばす箇所がある（5.258・5.260）。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::load_session;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::instance::{owner_of, root_of};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{
    begin_round2_plan, round1_plan, round2_plan, round2_plan_until_command, round_roots, run_plan,
    PlanStep,
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

/// 計画を `cand-session` の選択の位置で前後に分ける。入力列の写しを増やさず、元の計画から切り出す。
fn split_at_cand_session(plan: &[PlanStep]) -> (Vec<PlanStep>, Vec<PlanStep>) {
    let at = plan
        .iter()
        .position(|s| matches!(s, PlanStep::Op(Op::Input(Input::Choice(c))) if c == "cand-session"))
        .expect("cand-session in round 2 plan");
    (plan[..at].to_vec(), plan[at..].to_vec())
}

/// 2 周目の `st-2-4` までの前半（末尾で `st-2-4` に居ることを確かめる）と、`st-7-1` までの後半。
fn split_until_st71(ir: &Ir) -> (Vec<PlanStep>, Vec<PlanStep>) {
    let plan = round2_plan_until_command(ir, "/implement-issue-tree", "/st-7-1");
    let (mut head, tail) = split_at_cand_session(&plan);
    head.push(PlanStep::ExpectCurrentSuffix("/st-2-4"));
    (head, tail)
}

/// 2 周目の `st-2-4` までの前半と、周回の終わりまでの後半。
fn split_until_end(ir: &Ir) -> (Vec<PlanStep>, Vec<PlanStep>) {
    let (mut head, tail) = split_at_cand_session(&round2_plan(ir));
    head.push(PlanStep::ExpectCurrentSuffix("/st-2-4"));
    (head, tail)
}

fn save(rs: &mut RecordedSession) -> CheckpointId {
    let events = rs.apply(Op::Control(Control::Save)).to_vec();
    match events.as_slice() {
        [Event::CheckpointSaved { id, .. }] => *id,
        other => panic!("one CheckpointSaved expected: {other:?}"),
    }
}

fn state(rs: &RecordedSession) -> &SessionState {
    rs.session().state()
}

/// 呼び出しの Step は実体の表から引く（uid を直書きしない）。
fn call_step_of(ir: &Ir, root: &Root) -> StepUid {
    StepUid(
        ir.instances
            .iter()
            .find(|i| i.id == root.0)
            .and_then(|i| i.caller.clone())
            .expect("caller of the instance"),
    )
}

/// ログの `since` 以降の `Changed` を、宛先の持ち主の根ごとに集める。
fn changed_paths(s: &SessionState, ir: &Ir, since: usize) -> Vec<(Root, String)> {
    s.log
        .iter()
        .skip(since)
        .filter_map(|e| match e {
            Event::Changed { at, path, .. } => Some((owner_of(ir, &at.step), path.clone())),
            _ => None,
        })
        .collect()
}

/// `st-2-4` で保存し（持ち主は先に入る周回の実体）、`st-7-1`（もう一方の実体）まで進めた場面。
struct Scene {
    rs: RecordedSession,
    cp: CheckpointId,
    owner: Root,
    other: Root,
    r1: Vec<Root>,
    /// 保存の時点の状態。
    saved: SessionState,
    /// 保存の時点のログの長さ。
    since: usize,
}

fn scene_at_st71(ir: &Ir) -> Scene {
    let (r1, _) = round_roots(ir, "s-r1");
    let (r2, _) = round_roots(ir, "s-r2");
    assert_eq!(r2.len(), 2, "round 2 has two instances");
    let (head, tail) = split_until_st71(ir);
    let mut rs = recorded(ir);
    run_plan(&mut rs, &round1_plan(ir));
    run_plan(&mut rs, &begin_round2_plan());
    run_plan(&mut rs, &head);
    let owner = root_of(&state(&rs).run.current.clone().expect("current"));
    let other = r2
        .iter()
        .find(|r| **r != owner)
        .cloned()
        .expect("other instance of round 2");
    let since = state(&rs).log.len();
    let cp = save(&mut rs);
    let saved = state(&rs).clone();
    run_plan(&mut rs, &tail);
    Scene {
        rs,
        cp,
        owner,
        other,
        r1,
        saved,
        since,
    }
}

#[test]
fn mod16_restore_in_round2_keeps_the_other_instance_progress() {
    let ir = ir();
    let mut sc = scene_at_st71(&ir);
    let owner_call = call_step_of(&ir, &sc.owner);
    let other_call = call_step_of(&ir, &sc.other);
    let st = |id: &str| rounds::step_in(&ir, std::slice::from_ref(&sc.other), id);

    // 復帰の前提。崩れたら試験が意味を失う。
    let before = state(&sc.rs).clone();
    let s = sc.rs.session();
    assert_eq!(s.step_status(&st("st-3-2")), StepStatus::Achieved);
    assert_eq!(s.step_status(&st("st-6-1")), StepStatus::Achieved);
    assert_eq!(s.step_status(&st("st-7-1")), StepStatus::Entered);
    assert_eq!(s.step_status(&owner_call), StepStatus::Achieved);
    assert_eq!(s.step_status(&other_call), StepStatus::Entered);
    let written = changed_paths(&before, &ir, sc.since);
    let by_other: Vec<&String> = written
        .iter()
        .filter(|(r, _)| *r == sc.other)
        .map(|(_, p)| p)
        .collect();
    let by_owner: Vec<&String> = written
        .iter()
        .filter(|(r, _)| *r == sc.owner)
        .map(|(_, p)| p)
        .collect();
    assert!(!by_other.is_empty(), "the other instance must write paths");
    assert!(!by_owner.is_empty(), "the owner must write paths");

    let events = sc.rs.apply(Op::Control(Control::Restore(sc.cp))).to_vec();
    assert!(
        events
            .iter()
            .all(|e| matches!(e, Event::Restored { .. } | Event::Changed { .. })),
        "{events:?}"
    );
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Restored { .. }))
            .count(),
        1
    );

    let after = state(&sc.rs);
    let cp = &after.checkpoints[&sc.cp];
    assert_eq!(after.checkpoints.len(), 1);
    assert_eq!(cp.owner.as_ref(), Some(&sc.owner));
    // 持ち主だけが保存の時点に戻る。
    assert_eq!(after.run.current, sc.saved.run.current);
    assert_eq!(
        after.run.slices.get(&sc.owner),
        cp.run.slices.get(&sc.owner)
    );
    assert_eq!(
        sc.rs.session().step_status(&owner_call),
        StepStatus::Entered
    );
    // 同じ周回のもう一方の実体は復帰の直前のまま。
    assert_eq!(
        after.run.slices.get(&sc.other),
        before.run.slices.get(&sc.other)
    );
    let s = sc.rs.session();
    assert_eq!(s.step_status(&st("st-3-2")), StepStatus::Achieved);
    assert_eq!(s.step_status(&st("st-6-1")), StepStatus::Achieved);
    assert_eq!(s.step_status(&st("st-7-1")), StepStatus::Entered);
    assert_eq!(s.step_status(&other_call), StepStatus::Entered);
    // 1 周目の切り身は変わらない。
    for r in &sc.r1 {
        assert_eq!(after.run.slices.get(r), before.run.slices.get(r), "{r:?}");
    }
    // もう一方だけが書いた経路は復帰の直前のまま、持ち主だけが書いた経路は保存の時点の値に戻る。
    for p in by_other.iter().filter(|p| !by_owner.contains(p)) {
        assert_eq!(after.run.world.get(*p), before.run.world.get(*p), "{p}");
    }
    for p in by_owner.iter().filter(|p| !by_other.contains(p)) {
        assert_eq!(after.run.world.get(*p), cp.run.world.get(*p), "{p}");
    }
    println!(
        "OBS ii-a: after restore, {}/st-3-2={:?} owner_call={:?} other_call={:?} current={:?}",
        sc.other.0,
        s.step_status(&st("st-3-2")),
        s.step_status(&owner_call),
        s.step_status(&other_call),
        after.run.current.as_ref().map(|u| u.0.as_str()),
    );
}

#[test]
fn mod16_restore_after_round2_ends_has_no_round_completion_to_revert() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let (head, tail) = split_until_end(&ir);
    let mut rs = recorded(&ir);
    run_plan(&mut rs, &round1_plan(&ir));
    run_plan(&mut rs, &begin_round2_plan());
    run_plan(&mut rs, &head);
    let owner = root_of(&state(&rs).run.current.clone().expect("current"));
    let other = r2.iter().find(|r| **r != owner).cloned().expect("other");
    let cp = save(&mut rs);
    run_plan(&mut rs, &tail);

    let other_call = call_step_of(&ir, &other);
    let owner_call = call_step_of(&ir, &owner);
    assert_eq!(rs.session().step_status(&other_call), StepStatus::Achieved);
    // 分岐で通らない Step があるため全 Step ではなく、中ほどと最後の Step の達成を前提にする。
    for id in ["st-3-2", "st-7-4"] {
        let u = rounds::step_in(&ir, std::slice::from_ref(&other), id);
        assert_eq!(rs.session().step_status(&u), StepStatus::Achieved, "{u:?}");
    }
    let before = state(&rs).clone();

    rs.apply(Op::Control(Control::Restore(cp)));
    let after = state(&rs);
    // 持ち主だけが戻り、周回の完了を表す記録は戻らない（各実体の切り身にあるだけ）。
    assert_eq!(
        after.run.slices.get(&owner),
        after.checkpoints[&cp].run.slices.get(&owner)
    );
    assert_eq!(rs.session().step_status(&owner_call), StepStatus::Entered);
    assert_eq!(after.run.slices.get(&other), before.run.slices.get(&other));
    assert_eq!(rs.session().step_status(&other_call), StepStatus::Achieved);
    assert_eq!(after.run.completed, after.checkpoints[&cp].run.completed);
    for r in &r1 {
        assert_eq!(after.run.slices.get(r), before.run.slices.get(r), "{r:?}");
    }
}

#[test]
fn mod16_checkpoints_in_one_round_restore_each_owner_separately() {
    let ir = ir();
    let mut sc = scene_at_st71(&ir);
    let cp1 = sc.cp;
    let cp2 = save(&mut sc.rs);
    let (r2, _) = round_roots(&ir, "s-r2");
    {
        let cps = &state(&sc.rs).checkpoints;
        assert_eq!(cps.len(), 2);
        // 保存の持ち主は実体の根で、周回でまとまった識別子は無い。
        assert_eq!(cps[&cp1].owner.as_ref(), Some(&sc.owner));
        assert_eq!(cps[&cp2].owner.as_ref(), Some(&sc.other));
        assert!(r2.contains(&sc.owner) && r2.contains(&sc.other));
    }
    let at_cp2_other = state(&sc.rs).checkpoints[&cp2]
        .run
        .slices
        .get(&sc.other)
        .cloned();
    let before1 = state(&sc.rs).clone();

    // cp1 は持ち主 1 だけを戻す。
    sc.rs.apply(Op::Control(Control::Restore(cp1)));
    let after1 = state(&sc.rs).clone();
    assert_eq!(
        after1.run.slices.get(&sc.owner),
        after1.checkpoints[&cp1].run.slices.get(&sc.owner)
    );
    assert_eq!(
        after1.run.slices.get(&sc.other),
        before1.run.slices.get(&sc.other)
    );

    // 続けて cp2 へ戻ると、もう一方だけが cp2 の写しになり、持ち主 1 は cp1 の復帰の直後のまま。
    sc.rs.apply(Op::Control(Control::Restore(cp2)));
    let after2 = state(&sc.rs);
    assert_eq!(after2.run.slices.get(&sc.other), at_cp2_other.as_ref());
    assert_eq!(
        after2.run.slices.get(&sc.owner),
        after1.run.slices.get(&sc.owner)
    );
    assert_eq!(after2.run.current, after2.checkpoints[&cp2].run.current);
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

#[test]
fn mod16_restore_scope_is_the_same_after_reload() {
    let ir = ir();
    let mut sc = scene_at_st71(&ir);
    let env = sc.rs.envelope_for_save().expect("envelope");
    let bytes = export_bytes(&env).expect("export");
    let replayed = replay(&ir, SessionState::default(), &env.ops).expect("replay");
    let loaded = load_session(
        &bytes,
        ir.clone(),
        &versions(&ir),
        &OpeningCaps::current(),
        SessionState::default(),
    )
    .unwrap_or_else(|v| panic!("history must open: {v:?}"));

    let reopened = [
        boot(&ir, &bytes),
        RecordedSession::new(loaded.session, loaded.start.clone()),
        RecordedSession::new(
            Session::from_state(ir.clone(), replayed),
            RecordedSession::first_start(&ir),
        ),
    ];
    sc.rs.apply(Op::Control(Control::Restore(sc.cp)));
    let want = state(&sc.rs).run.clone();
    for mut r in reopened {
        r.apply(Op::Control(Control::Restore(sc.cp)));
        let got = &state(&r).run;
        assert_eq!(got.current, want.current);
        assert_eq!(got.slices, want.slices);
        assert_eq!(got.world, want.world);
    }
}
