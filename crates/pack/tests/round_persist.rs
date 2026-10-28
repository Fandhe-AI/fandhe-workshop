//! 2 周分の履歴の保存と復元で、4 実体の切り身が等しいことの受け入れ試験（TASK-247。MOD-14・EVT-21。記録は `docs/testing.md` の 5.339）。
//!
//! `module-reuse-rounds` の 1 周目と 2 周目（実体は周回ごとに 2 つ、計 4 つ）を `RecordedSession` で流して保存し、次の 2 経路で
//! 開き直しても、4 実体それぞれの切り身（`Slice`。状態・選択・フラグ・問い返し）が保存の前と等しいことを確かめる。
//! (1) 再読み込み: 起動の分岐 `boot_session`（web の復元と同じ関数）。
//! (2) 書き出し → 読み込み: `export_bytes` と `load_session`（別のオリジンでの読み込み、SAVE-16）。
//! あわせて、1 周目を終えた後の 2 周目の途中開始で残る `Skipped`（EVT-21）が、どちらの経路でも失われないことを確かめる。
//! 試作の `reload_keeps_per_instance_state_separate_across_rounds` に当たる。
//!
//! 切り身は世代やログに依存しないので完全一致で比べる。失敗の文言には根・Step の ID と時点の名前だけを出し、下書き・入力の本文は出さない。
//! 置き場が pack なのは、形式 B の教材のコンパイルが pack にしかなく、core から pack へは依存できないため（`round_three.rs` と同じ）。
//! 入力列は `common/rounds.rs` の計画を使い、未実装の部品のため `GoTo` で飛ばす箇所がある（5.258・5.260）。
//! 呼び出し元は CI の `make test` だけである。変異 `MOD14-*` の検出先になる。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{load_session, LoadedSession};
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use rounds::{
    begin_round2_plan, round1_plan, round2_plan, round2_plan_until_create_tree, round_roots,
    run_plan, PlanStep,
};

const MIDSTART: &str = "issue-to-merge@round2#s-6-1";

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn versions(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 1 周目の 2 実体と 2 周目の 2 実体。実体の ID は IR から求める（直書きしない）。
fn four_roots(ir: &Ir) -> Vec<Root> {
    let mut roots = round_roots(ir, "s-r1").0;
    roots.extend(round_roots(ir, "s-r2").0);
    assert_eq!(roots.len(), 4, "two instances per round");
    roots
}

fn recorded(ir: &Ir) -> RecordedSession {
    RecordedSession::new(
        Session::from_state(ir.clone(), SessionState::default()),
        RecordedSession::first_start(ir),
    )
}

fn save() -> PlanStep {
    PlanStep::Op(Op::Control(Control::Save))
}

fn open_hint() -> PlanStep {
    PlanStep::Op(Op::Control(Control::OpenHint))
}

/// 1 周目 → 保存 → 2 周目の開始 → 2 周目 → 保存（`history_size_rounds.rs` の条件 A と同じ形）。
fn two_rounds(ir: &Ir) -> RecordedSession {
    let mut s = recorded(ir);
    let mut plan = round1_plan(ir);
    plan.push(save());
    plan.extend(begin_round2_plan());
    plan.extend(round2_plan(ir));
    plan.push(save());
    run_plan(&mut s, &plan);
    s
}

fn save_bytes(s: &RecordedSession) -> Vec<u8> {
    export_bytes(&s.envelope_for_save().expect("envelope")).expect("export")
}

/// 起動の分岐での再読み込み。変換して開いたものではないことも確かめる。
fn reload(ir: &Ir, bytes: &[u8]) -> RecordedSession {
    let outcome = boot_session(
        Some(bytes),
        ir.clone(),
        &versions(ir),
        &OpeningCaps::current(),
        SessionState::default(),
        &RecordedSession::first_start(ir),
    );
    let BootOutcome::Restored { session, notes } = outcome else {
        panic!("history must be restored");
    };
    assert!(notes.is_none(), "not converted");
    session
}

/// 別のオリジンでの読み込み（新しい状態から開く）。
fn import(ir: &Ir, bytes: &[u8]) -> LoadedSession {
    let loaded = load_session(
        bytes,
        ir.clone(),
        &versions(ir),
        &OpeningCaps::current(),
        SessionState::default(),
    )
    .unwrap_or_else(|v| panic!("history must open: {v:?}"));
    assert!(loaded.notes.is_none(), "not converted");
    loaded
}

fn assert_slices_eq(a: &Session, b: &Session, roots: &[Root], point: &str) {
    for r in roots {
        assert_eq!(a.slice(r), b.slice(r), "slice of {r:?} at {point}");
    }
    assert_eq!(a.state().run.slices, b.state().run.slices, "{point}");
}

/// 4 根すべてに中身があり、互いに違う（比較が空振りしない前提）。
fn assert_populated(s: &Session, roots: &[Root]) {
    for r in roots {
        let sl = s.slice(r).unwrap_or_else(|| panic!("slice of {r:?}"));
        assert!(!sl.status.is_empty(), "empty slice of {r:?}");
    }
    for (i, a) in roots.iter().enumerate() {
        for b in &roots[i + 1..] {
            assert_ne!(s.slice(a), s.slice(b), "{a:?} vs {b:?}");
        }
    }
}

/// 世代の +1 とログ末尾の `Reloaded` 1 件を除いて、`before` と `after` が等しい。
fn assert_only_reload_differs(before: &SessionState, after: &SessionState) {
    let (mut a, mut b) = (after.clone(), before.clone());
    a.run.generation = Generation(0);
    b.run.generation = Generation(0);
    a.log.clear();
    b.log.clear();
    assert_eq!(a, b);
    assert_eq!(after.run.generation.0, before.run.generation.0 + 1);
    let n = before.log.len();
    assert_eq!(after.log.get(..n), Some(before.log.as_slice()));
    assert_eq!(after.log.len(), n + 1);
    assert!(matches!(after.log.last(), Some(Event::Reloaded { .. })));
}

fn skipped_events(state: &SessionState) -> Vec<StepUid> {
    state
        .log
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect()
}

/// 切り身の `Skipped` の Step（持ち主の根つき）。
fn skipped_in_slices(state: &SessionState) -> Vec<(Root, StepUid)> {
    let mut v = Vec::new();
    for (root, slice) in &state.run.slices {
        for (uid, st) in &slice.status {
            if *st == StepStatus::Skipped {
                v.push((root.clone(), uid.clone()));
            }
        }
    }
    v
}

fn restore1() -> Op {
    Op::Control(Control::Restore(CheckpointId(1)))
}

#[test]
fn mod14_two_rounds_history_keeps_four_slices_across_reload() {
    let ir = ir();
    let roots = four_roots(&ir);
    let mut s = two_rounds(&ir);
    assert_populated(s.session(), &roots);
    for round in 1..=3 {
        let before = s.session().state().clone();
        let bytes = save_bytes(&s);
        let after = reload(&ir, &bytes);
        let point = format!("reload {round}");
        assert_slices_eq(s.session(), after.session(), &roots, &point);
        assert_only_reload_differs(&before, after.session().state());
        s = after;
    }
}

#[test]
fn mod14_two_rounds_history_keeps_four_slices_across_export_import() {
    let ir = ir();
    let roots = four_roots(&ir);
    let s = two_rounds(&ir);
    assert_populated(s.session(), &roots);
    let loaded = import(&ir, &save_bytes(&s));
    assert_slices_eq(s.session(), &loaded.session, &roots, "import");
    assert_eq!(loaded.session.state(), s.session().state());
    assert_eq!(loaded.start, s.start());
    assert_eq!(loaded.ops.as_slice(), s.ops());
}

/// 試作の `reload_keeps_per_instance_state_separate_across_rounds` に当たる。再読み込み・読み込みの後も、
/// 操作（復帰）が持ち主の根だけを動かし、4 根の切り身が元と等しいままである。
#[test]
fn mod14_reload_keeps_per_instance_state_separate_across_rounds() {
    let ir = ir();
    let roots = four_roots(&ir);
    let mut plan = round1_plan(&ir);
    plan.push(save());
    plan.extend(begin_round2_plan());
    plan.extend(round2_plan_until_create_tree(&ir));
    plan.push(open_hint());
    let mut s = recorded(&ir);
    run_plan(&mut s, &plan);
    assert_populated(s.session(), &roots);

    let bytes = save_bytes(&s);
    let mut reloaded = reload(&ir, &bytes);
    let mut imported = import(&ir, &bytes).session;
    assert_slices_eq(s.session(), reloaded.session(), &roots, "reload");
    assert_slices_eq(s.session(), &imported, &roots, "import");
    assert_eq!(
        s.session().state().hints_opened,
        reloaded.session().state().hints_opened
    );
    assert_eq!(
        s.session().state().hints_opened,
        imported.state().hints_opened
    );

    // 開き直した後の復帰も、元と同じ根だけを動かす。
    s.apply(restore1());
    reloaded.apply(restore1());
    imported.apply(restore1());
    assert_slices_eq(
        s.session(),
        reloaded.session(),
        &roots,
        "restore after reload",
    );
    assert_slices_eq(s.session(), &imported, &roots, "restore after import");
}

/// EVT-21。1 周目を終えた後の 2 周目の途中開始で飛ばした 3 件が、再読み込みと書き出し → 読み込みの後も残る。
#[test]
fn evt21_round2_midstart_skips_survive_reload_and_export_import_after_round1() {
    let ir = ir();
    let roots = four_roots(&ir);
    let mut s = recorded(&ir);
    run_plan(&mut s, &round1_plan(&ir));
    let round1_before: Vec<_> = roots[..2]
        .iter()
        .map(|r| s.session().slice(r).cloned())
        .collect();
    let events = s
        .apply(Op::Control(Control::Begin(MIDSTART.to_string())))
        .to_vec();
    let from_events = events
        .iter()
        .filter(|e| matches!(e, Event::Skipped { .. }))
        .count();
    assert_eq!(from_events, 3, "skipped by the start");
    let check = |st: &SessionState, point: &str| {
        assert_eq!(skipped_events(st).len(), 3, "log at {point}");
        let in_slices = skipped_in_slices(st);
        assert_eq!(in_slices.len(), 3, "slices at {point}");
        for (root, _) in &in_slices {
            assert_eq!(root.0, "issue-to-merge@round2", "owner at {point}");
        }
        for (root, slice) in &st.run.slices {
            let achieved = slice.status.values().any(|v| *v == StepStatus::Achieved);
            assert!(
                !achieved || roots[..2].contains(root),
                "achieved in {root:?} at {point}"
            );
        }
    };
    check(s.session().state(), "begin");
    for (r, before) in roots[..2].iter().zip(&round1_before) {
        assert_eq!(s.session().slice(r), before.as_ref(), "round 1 {r:?}");
    }

    let bytes = save_bytes(&s);
    let mut reloaded = reload(&ir, &bytes);
    check(reloaded.session().state(), "reload");
    assert_slices_eq(s.session(), reloaded.session(), &roots, "reload");
    let imported = import(&ir, &bytes).session;
    check(imported.state(), "import");
    assert_slices_eq(s.session(), &imported, &roots, "import");

    // 復元の後に進めても、省略は増えず達成にもならない。
    reloaded.apply(Op::Control(Control::OpenHint));
    check(reloaded.session().state(), "after reload and op");
}
