//! 履歴を開いたときだけ実行環境（`Session`）を作る入口 `load_session` の試験（TASK-330 2/2、SAVE-16）。
//!
//! 開く場合は適用し直した状態が写しと等しい `Session` が返り、表示だけ・開かないでは `Session` を作らない。
//! 契約の試料（`two-rounds`）は再読み込みの意味の実装前の写しで開けないため、試料の `ops` を適用し直した結果を
//! 写しにした「開ける包み」を試験の中で作る。

mod common;

use common::contract_sample::{load as load_sample, testdata_dir};
use common::history_sample::read_bytes;
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history_version::{HistoryVersions, Verdict};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{
    load_session, parse_history_envelope, parse_history_json,
};
use fandhe_workshop_core::history_open::OpeningCaps;
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::version_record::record_versions;
use fandhe_workshop_core::{load_ir, SessionState};

fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(path).expect("read min-3step")).expect("load_ir")
}

fn opening(ir: &Ir) -> HistoryVersions {
    record_versions(ir, &content_hash(ir).expect("hash")).expect("versions")
}

/// 試料の `ops` を適用し直した結果を写しにした、開ける包みのバイト列と、その写し。
fn loadable() -> (Vec<u8>, SessionState) {
    let ir = ir();
    let value = parse_history_json(&read_bytes("two-rounds")).expect("json");
    let env = parse_history_envelope(value).expect("envelope");
    let state = replay(&ir, load_sample().initial, &env.ops).expect("replay");
    let built = build_envelope(&ir, &env.start, env.ops, state.clone()).expect("build");
    (export_bytes(&built).expect("export"), state)
}

fn load(
    bytes: &[u8],
    edit: impl FnOnce(&mut HistoryVersions),
) -> Result<SessionState, Verdict<LoadReason>> {
    let ir = ir();
    let mut op = opening(&ir);
    edit(&mut op);
    load_session(
        bytes,
        ir,
        &op,
        &OpeningCaps::current(),
        load_sample().initial,
    )
    .map(|l| l.session.state().clone())
}

#[test]
fn open_history_yields_a_session_equal_to_the_snapshot() {
    let (bytes, state) = loadable();
    assert_eq!(load(&bytes, |_| {}), Ok(state));
}

#[test]
fn view_only_yields_no_session() {
    let (bytes, _) = loadable();
    let v = load(&bytes, |o| o.engine = "2.0.0".to_owned()).expect_err("does not open");
    assert!(matches!(
        v,
        Verdict::ViewOnly { .. } | Verdict::Refused { .. }
    ));
    assert!(v.code().is_some());
}

#[test]
fn refused_yields_no_session() {
    let v = load(b"not json", |_| {}).expect_err("refused");
    assert_eq!(
        v,
        Verdict::Refused {
            code: LoadReason::NotJson
        }
    );
}

#[test]
fn tampered_snapshot_is_replay_mismatch() {
    let (bytes, _) = loadable();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
    v["snapshot"]["next_checkpoint"] = serde_json::json!(9999);
    let tampered = serde_json::to_vec(&v).expect("to_vec");
    let verdict = load(&tampered, |_| {}).expect_err("refused");
    assert_eq!(verdict.code(), Some(&LoadReason::ReplayMismatch));
}
