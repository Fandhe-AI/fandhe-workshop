//! 起動の入口（`open_bundled_ir`）が開始点に `Control::Begin` と同じ規則で入ることの native 試験（#2164）。
//!
//! effects つきの開始点で開くと世界状態に反映され、未操作では保存せず（SAVE-28）、操作の後は再読み込み・
//! 履歴の読み込みの後も同じ状態に戻ることを、web の保存の経路（`history_to_save`・`boot`・`open_history`）で確かめる。
//! 開始条件が成り立たない開始点は断る。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_workshop_contract::storage::DeliveryScope;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::Session;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::start_select::{open_bundled_ir, BundledIrRefusal, StartRefusal};

fn ir_bytes() -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    v["starts"] = serde_json::json!([
        {
            "id": "s-fx",
            "node": "main/st-2",
            "guidance": "C",
            "effects": [{ "set_flag_global": "from-start" }]
        },
        {
            "id": "s-need",
            "node": "main/st-2",
            "effects": [],
            "requires": { "step": { "step": "st-1", "state": "achieved" } }
        },
    ]);
    serde_json::to_vec(&v).unwrap()
}

fn page(start: &str) -> EngineHost {
    let (session, id) = open_bundled_ir(&ir_bytes(), Some(start)).unwrap();
    EngineHost::with_session_at(session, standard_registry(), id).unwrap()
}

fn flag_set(h: &EngineHost) -> bool {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .global_flags
        .contains("from-start")
}

fn go_st3(h: &mut EngineHost) {
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-3".to_owned(),
    )))));
}

#[test]
fn 開始点の_effects_が起動の状態に反映される() {
    let h = page("s-fx");
    assert!(flag_set(&h));
    let s: &Session = h.model().session().unwrap();
    assert_eq!(s.state().run.current, Some(StepUid("main/st-2".to_owned())));
}

#[test]
fn 未操作の再読み込みは保存せず新しく始まる() {
    let h = page("s-fx");
    assert_eq!(h.history_to_save(), None, "begin は ops に積まれない");
    let mut h2 = page("s-fx");
    assert_eq!(h2.boot(None), BootKind::Fresh);
    assert!(flag_set(&h2));
}

#[test]
fn 操作の後は再読み込みと履歴の読み込みで同じ状態に戻る() {
    let mut h = page("s-fx");
    go_st3(&mut h);
    let saved = h.history_to_save().expect("操作の後は保存する");
    let before = h.model().session().unwrap().generation().0;

    let mut h2 = page("s-fx");
    assert_eq!(h2.boot(Some(saved.as_bytes())), BootKind::Restored);
    assert!(flag_set(&h2), "initial の effects が再生の前提に残る");
    assert_eq!(h2.model().session().unwrap().generation().0, before + 1);

    let mut h3 = page("s-fx");
    assert_eq!(h3.open_history(saved.as_bytes()), ImportOutcome::Open);
    assert!(flag_set(&h3));
    assert_eq!(
        h3.model().session().unwrap().state().run.current,
        Some(StepUid("main/st-3".to_owned()))
    );
}

#[test]
fn 開始条件が成り立たない開始点は開かず固定の理由で断る() {
    let Err(BundledIrRefusal::Start(refusal)) = open_bundled_ir(&ir_bytes(), Some("s-need")) else {
        panic!("断られるはず");
    };
    assert_eq!(refusal, StartRefusal::Unmet);
    assert_eq!(refusal.reason_code(), "start_unmet");
    let h = EngineHost::refused_start(refusal);
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
}
