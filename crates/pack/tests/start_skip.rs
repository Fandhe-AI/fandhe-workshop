//! 途中開始で飛ばした Step の省略を、コンパイルした IR で確かめる試験（TASK-217 1/2、EVT-21・MOD-6）。
//!
//! `module-reuse-rounds` の 2 周目の実体 `issue-to-merge@round2` の `s-6-1` から始めると、同じ実体の `st-3-2`・`st-4-1`・
//! `st-5-1` が `Skipped` になり、ほかの実体の切り身は開始の前と変わらないことを確かめる。
//! 1 周目を流し終えた状態からの開始でも、省略は開始する周回の実体の Step だけで前の周回の切り身が変わらないこと
//! （TASK-239 2/2。`mod6_*`。記録は `docs/testing.md` の 5.329）も確かめる。
//! 書き出し→読み込み・適用し直しでの件数の保持（TASK-217 2/2）もここで確かめる（一般の件数の試験は
//! `crates/core/tests/begin_skip_keep.rs`）。開始点の効果の `${名前}` の置換は `start_instance.rs`（TASK-239）。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::testdata;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::instance::status_of;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;

const START: &str = "issue-to-merge@round2#s-6-1";

#[test]
fn midstart_in_round2_skips_earlier_steps_of_the_same_instance() {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    assert!(ir.starts.iter().any(|s| s.id == START), "start id");
    let mut session = Session::from_state(ir.clone(), SessionState::default());
    let before = session.state().run.clone();
    let events = session
        .apply(Op::Control(Control::Begin(START.to_string())))
        .to_vec();
    let uid = |id: &str| StepUid(format!("issue-to-merge@round2/{id}"));
    for id in ["st-3-2", "st-4-1", "st-5-1"] {
        assert_eq!(
            status_of(&ir, &session.state().run, &uid(id)),
            StepStatus::Skipped,
            "{id}"
        );
    }
    assert_eq!(
        status_of(&ir, &session.state().run, &uid("st-6-1")),
        StepStatus::Entered
    );
    let skipped: Vec<StepUid> = events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(skipped, vec![uid("st-3-2"), uid("st-4-1"), uid("st-5-1")]);
    for (root, slice) in &before.slices {
        if root.0 != "issue-to-merge@round2" {
            assert_eq!(
                session.state().run.slices.get(root),
                Some(slice),
                "{root:?}"
            );
        }
    }
    for (root, slice) in &session.state().run.slices {
        if root.0.starts_with("issue-to-merge@round1") {
            assert!(slice.status.is_empty(), "{root:?}");
        }
    }
}

/// 実測例の 3 件が、書き出し→読み込み・適用し直しの後も 3 件のままである（TASK-217 2/2、EVT-21）。
#[test]
fn round2_skips_survive_export_import() {
    use fandhe_workshop_contract::content_hash::content_hash;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_core::history_load::load_history;
    use fandhe_workshop_core::history_open::{Opened, OpeningCaps};
    use fandhe_workshop_core::history_replay::replay;
    use fandhe_workshop_core::recorded_session::RecordedSession;
    use fandhe_workshop_core::version_record::record_versions;

    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let start = RecordedSession::first_start(&ir);
    let mut s = RecordedSession::new(
        Session::from_state(ir.clone(), SessionState::default()),
        start,
    );
    s.apply(Op::Control(Control::Begin(START.to_string())));
    let before = s.session().state().clone();
    let count = |state: &SessionState| {
        state
            .log
            .iter()
            .filter(|e| matches!(e, Event::Skipped { .. }))
            .count()
    };
    assert_eq!(count(&before), 3);

    let env = s.envelope_for_save().expect("envelope");
    let exported = export_bytes(&env).expect("export");
    let opening = record_versions(&ir, &content_hash(&ir).expect("hash")).expect("versions");
    let opened = load_history(
        &exported,
        &ir,
        &opening,
        &OpeningCaps::current(),
        SessionState::default(),
    );
    let Opened::Open { state } = opened else {
        panic!("history must open: {:?}", opened.verdict());
    };
    assert_eq!(state, before);
    assert_eq!(count(&state), 3);
    let replayed = replay(&ir, SessionState::default(), s.ops()).expect("replay");
    assert_eq!(replayed, before);
}

fn module_ir() -> fandhe_workshop_contract::ir::Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

/// 1 周目の計画を流し終えた状態。`GoTo` で飛ばした Step が `NotStarted` のまま残る（空振りしない前提）。
fn played_round1(ir: &fandhe_workshop_contract::ir::Ir) -> Session {
    let mut session = Session::new(ir.clone());
    let events = rounds::run_plan(&mut session, &rounds::round1_plan(ir));
    let cur = session.state().run.current.clone().expect("current").0;
    assert_eq!(cur, "main/r1-impl", "{events:?}");
    session
}

/// 状態の全切り身から、`Skipped` の Step と持ち主の根を集める。
fn skipped_in_slices(state: &SessionState) -> Vec<(fandhe_workshop_core::addr::Root, StepUid)> {
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

/// 1 周目を流し終えた状態（1 周目の切り身が空でなく、未着手の Step が残る）から 2 周目の途中で開始しても、
/// 省略は 2 周目の実体の 3 件だけで、1 周目の切り身と世界状態の既存の値は変わらない（TASK-239 2/2、MOD-6）。
#[test]
fn mod6_start_after_round1_skips_only_round2_instance_steps() {
    let ir = module_ir();
    let (r1_roots, _) = rounds::round_roots(&ir, "s-r1");
    // `round1_plan` は `GoTo` で飛ばした Step も `Entered` にして残さないため、1 周目の `st-4-1`・`st-5-1` を未着手に戻し、
    // 同じ `unit` で `scope` だけが違う未着手の Step を作る（`scope` の条件を外す変異で省略に混ざることを検出するため）。
    let mut state = played_round1(&ir).state().clone();
    for id in ["st-4-1", "st-5-1"] {
        let u = StepUid(format!("issue-to-merge@round1/{id}"));
        let owner = fandhe_workshop_core::instance::owner_of(&ir, &u);
        let removed = state
            .run
            .slices
            .get_mut(&owner)
            .expect("slice")
            .status
            .remove(&u);
        assert!(removed.is_some(), "{id}");
    }
    let mut session = Session::from_state(ir.clone(), state);
    let pre = session.state().run.clone();
    assert!(skipped_in_slices(session.state()).is_empty());
    // 前提: 1 周目の切り身が空でなく、2 周目の開始の節点より前に 1 周目の未着手の Step がある。
    assert!(r1_roots
        .iter()
        .any(|r| pre.slices.get(r).is_some_and(|s| !s.status.is_empty())));
    let start_idx = ir
        .nodes
        .iter()
        .position(|n| n.uid == "issue-to-merge@round2/st-6-1")
        .expect("start node");
    let leftover = ir
        .nodes
        .iter()
        .take(start_idx)
        .filter(|n| n.uid.starts_with("issue-to-merge@round1/") && n.step.use_module.is_none())
        .filter(|n| status_of(&ir, &pre, &StepUid(n.uid.clone())) == StepStatus::NotStarted)
        .count();
    assert!(leftover >= 1, "round1 not-started steps must remain");

    let events = session
        .apply(Op::Control(Control::Begin(START.to_string())))
        .to_vec();
    let uid = |id: &str| StepUid(format!("issue-to-merge@round2/{id}"));
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Begun { .. }))
            .count(),
        1
    );
    let skipped: Vec<StepUid> = events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(skipped, vec![uid("st-3-2"), uid("st-4-1"), uid("st-5-1")]);
    let all = skipped_in_slices(session.state());
    assert_eq!(all.len(), 3, "{all:?}");
    assert!(
        all.iter().all(|(r, _)| r.0 == "issue-to-merge@round2"),
        "{all:?}"
    );
    assert_eq!(
        session
            .state()
            .log
            .iter()
            .filter(|e| matches!(e, Event::Skipped { .. }))
            .count(),
        3
    );
    assert_eq!(
        status_of(&ir, &session.state().run, &uid("st-6-1")),
        StepStatus::Entered
    );
    assert_eq!(session.state().run.current.as_ref(), Some(&uid("st-6-1")));
    // 1 周目の各根の切り身は不変。2 周目の根以外の全切り身も不変。
    for r in &r1_roots {
        assert_eq!(
            session.state().run.slices.get(r),
            pre.slices.get(r),
            "{r:?}"
        );
    }
    for (root, slice) in &pre.slices {
        if root.0 != "issue-to-merge@round2" {
            assert_eq!(
                session.state().run.slices.get(root),
                Some(slice),
                "{root:?}"
            );
        }
    }
    // 世界状態: 既存の経路の値は不変で、増えたのは 2 周目の開始の 1 件だけ。
    let after = &session.state().run.world;
    let before_json = serde_json::to_value(&pre.world).expect("json");
    let after_json = serde_json::to_value(after).expect("json");
    assert_ne!(
        before_json, after_json,
        "start effect must change the world"
    );
}

/// 2 周目を始めた後の途中開始でも、同じ周回の別の実体と 1 周目の切り身は変わらず、省略は未着手の実体の Step だけである。
#[test]
fn mod6_start_after_round2_begin_keeps_sibling_instance() {
    let ir = module_ir();
    let mut session = played_round1(&ir);
    rounds::run_plan(&mut session, &rounds::begin_round2_plan());
    let pre = session.state().run.clone();
    let events = session
        .apply(Op::Control(Control::Begin(START.to_string())))
        .to_vec();
    // 途中開始が成功した（拒否されていない）ことを、開始の 1 件・現在の Step・省略の 3 件で固定する。
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Begun { .. }))
            .count(),
        1
    );
    assert_eq!(
        session.state().run.current.as_ref(),
        Some(&StepUid("issue-to-merge@round2/st-6-1".to_string()))
    );
    let skipped: Vec<StepUid> = events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } => Some(at.step.clone()),
            _ => None,
        })
        .collect();
    let uid = |id: &str| StepUid(format!("issue-to-merge@round2/{id}"));
    assert_eq!(skipped, vec![uid("st-3-2"), uid("st-4-1"), uid("st-5-1")]);
    for e in &events {
        if let Event::Skipped { at } = e {
            assert!(at.step.0.starts_with("issue-to-merge@round2/"), "{at:?}");
            assert_eq!(status_of(&ir, &pre, &at.step), StepStatus::NotStarted);
        }
    }
    for (root, slice) in &pre.slices {
        if root.0 != "issue-to-merge@round2" {
            assert_eq!(
                session.state().run.slices.get(root),
                Some(slice),
                "{root:?}"
            );
        }
    }
    assert!(skipped_in_slices(session.state())
        .iter()
        .all(|(r, _)| r.0 == "issue-to-merge@round2"));
}
