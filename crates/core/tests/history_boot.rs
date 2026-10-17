//! 起動の分岐 `boot_session` の試験（TASK-319 1/3、SAVE-3・SAVE-28）。
//!
//! 保存済みの履歴が無い場合と、未操作（`ops` が 0 件）の場合に、新しい状態で始まり世代が上がらないことを確かめる。
//! 陽性の対照として、操作 1 件の履歴は復元され、世代が +1 になり `Reloaded` が 1 件足されることを 1 地点だけ確かめる
//! （復元の一致の網羅は TASK-319 の別の sub-issue が持つ）。開けない履歴は初期状態のまま始まる。

mod common;

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{op_specs, Harness};
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{HistoryVersions, Verdict};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_boot::{boot_session, BootOutcome};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{load_ir, Session, SessionState};

fn ir_and_initial() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

fn boot(saved: Option<&[u8]>, ir: &Ir, initial: &SessionState) -> BootOutcome {
    boot_session(
        saved,
        ir.clone(),
        &opening(ir),
        &OpeningCaps::current(),
        initial.clone(),
        "s-1",
    )
}

fn reloaded_count(state: &SessionState) -> usize {
    state
        .log
        .iter()
        .filter(|e| matches!(e, Event::Reloaded { .. }))
        .count()
}

/// 操作を 1 件記録した実行環境から、保存される値を作る。
fn saved_after_one_draft(ir: &Ir, initial: &SessionState) -> Vec<u8> {
    let mut rs = RecordedSession::new(Session::from_state(ir.clone(), initial.clone()), "s-1");
    rs.draft("未送信のメモ".to_owned());
    let env = rs.envelope_for_save().expect("envelope");
    export_bytes(&env).expect("export")
}

#[test]
fn 保存済みの履歴が無ければ新しく始まり世代は上がらない() {
    let (ir, initial) = ir_and_initial();
    let BootOutcome::Fresh(rs) = boot(None, &ir, &initial) else {
        panic!("must be fresh");
    };
    assert_eq!(rs.session().state(), &initial);
    assert_eq!(rs.session().generation(), initial.run.generation);
    assert_eq!(rs.session().log().len(), initial.log.len());
    assert_eq!(reloaded_count(rs.session().state()), 0);
    assert!(rs.ops().is_empty());
    assert!(rs.envelope_for_save().is_none(), "未操作は保存しない");
}

#[test]
fn 任意の初期状態で保存済みが無ければ初期状態のまま() {
    let (ir, initial) = ir_and_initial();
    check(
        "boot-none-is-identity",
        DEFAULT_CASES,
        op_specs(),
        |specs| {
            let mut h = Harness::pure(&ir, initial.clone());
            for s in &specs {
                h.step(s);
            }
            let any_initial = h.state().clone();
            let BootOutcome::Fresh(rs) = boot(None, &ir, &any_initial) else {
                return Err(proptest::test_runner::TestCaseError::fail("not fresh"));
            };
            proptest::prop_assert_eq!(rs.session().state(), &any_initial);
            proptest::prop_assert!(rs.ops().is_empty());
            Ok(())
        },
    );
}

#[test]
fn 操作が_0_件の包みは未操作として新しく始まる() {
    let (ir, initial) = ir_and_initial();
    // 外から置かれた 0 件の包み（通常は保存されない）。
    let env = build_envelope(&ir, "s-1", Vec::new(), initial.clone()).expect("build");
    let bytes = export_bytes(&env).expect("export");
    let BootOutcome::Fresh(rs) = boot(Some(&bytes), &ir, &initial) else {
        panic!("empty ops must start fresh");
    };
    assert_eq!(rs.session().state(), &initial);
    assert_eq!(rs.session().generation(), initial.run.generation);
    assert_eq!(reloaded_count(rs.session().state()), 0);
    assert!(rs.envelope_for_save().is_none());
}

#[test]
fn 対照_操作が_1_件あれば復元され世代が_1_上がる() {
    let (ir, initial) = ir_and_initial();
    let bytes = saved_after_one_draft(&ir, &initial);
    let BootOutcome::Restored { session, notes } = boot(Some(&bytes), &ir, &initial) else {
        panic!("must be restored");
    };
    assert!(notes.is_none());
    let state = session.session().state();
    assert_eq!(
        state.run.generation.0,
        initial.run.generation.0 + 1,
        "復元は再読み込みとして世代を 1 上げる"
    );
    assert_eq!(reloaded_count(state), 1);
    assert_eq!(state.log.len(), initial.log.len() + 1);
    // 記録は読み込んだ ops に Reload が 1 件続く。
    assert_eq!(session.ops().len(), 2);
    assert!(session.envelope_for_save().is_some());
}

#[test]
fn 開けない保存済みの値は初期状態のまま始まる() {
    let (ir, initial) = ir_and_initial();
    for bytes in [&b""[..], &b"{not json"[..], &b"[]"[..]] {
        let BootOutcome::NotOpened { session, verdict } = boot(Some(bytes), &ir, &initial) else {
            panic!("must not open");
        };
        assert!(matches!(verdict, Verdict::Refused { .. }));
        assert_eq!(session.session().state(), &initial);
        assert!(session.envelope_for_save().is_none());
    }
}

#[test]
fn 別の教材の履歴は開かない() {
    let (ir, initial) = ir_and_initial();
    let bytes = saved_after_one_draft(&ir, &initial);
    let mut other = ir.clone();
    other.pack.id = "other-pack".to_owned();
    let out = boot_session(
        Some(&bytes),
        other,
        &opening(&ir),
        &OpeningCaps::current(),
        initial.clone(),
        "s-1",
    );
    let BootOutcome::NotOpened { verdict, .. } = out else {
        panic!("other pack must not open");
    };
    assert_eq!(
        verdict,
        Verdict::Refused {
            code: LoadReason::OtherPack
        }
    );
}
