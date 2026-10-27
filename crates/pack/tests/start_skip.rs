//! 途中開始で飛ばした Step の省略を、コンパイルした IR で確かめる試験（TASK-217 1/2、EVT-21・MOD-6）。
//!
//! `module-reuse-rounds` の 2 周目の実体 `issue-to-merge@round2` の `s-6-1` から始めると、同じ実体の `st-3-2`・`st-4-1`・
//! `st-5-1` が `Skipped` になり、ほかの実体の切り身は開始の前と変わらないことを確かめる。
//! 書き出し→読み込み・適用し直しでの件数の保持（TASK-217 2/2）もここで確かめる（一般の件数の試験は
//! `crates/core/tests/begin_skip_keep.rs`）。開始点の効果の `${名前}` の置換は `start_instance.rs`（TASK-239）。

mod common;

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
