//! ホストが持つ受け口の既定が無効であることの試験（TASK-191 1/2、EXT-10・D-53）。
//! 同意は保存せず、履歴を開く（実行環境の差し替え）と未同意に戻ることも固定する。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::receptor_gate::{
    Activation, DisabledReason, ReceptorId, ReceptorSettings,
};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, ImportOutcome};
use serde_json::Value;

const R: ReceptorId = ReceptorId::new("test-receptor");

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json");
    load_ir(&std::fs::read(p).unwrap()).unwrap()
}

fn state() -> SessionState {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let text = serde_json::to_string(&s["initial"])
        .unwrap()
        .replace("\"main/st-1\"", "\"main/st-1-4\"");
    serde_json::from_str(&text).unwrap()
}

fn host() -> EngineHost {
    EngineHost::with_session(Session::from_state(ir(), state()), standard_registry()).unwrap()
}

#[test]
fn ext10_host_default_is_disabled() {
    assert_eq!(
        EngineHost::new().receptor_activation(R),
        Activation::Disabled(DisabledReason::NotConfigured)
    );
}

#[test]
fn ext10_host_needs_settings_and_consent() {
    let mut h = EngineHost::new().with_receptor_settings(ReceptorSettings::default().allow(R));
    assert_eq!(
        h.receptor_activation(R),
        Activation::Disabled(DisabledReason::NoConsent)
    );
    h.grant_receptor_consent(R);
    assert_eq!(h.receptor_activation(R), Activation::Enabled);
    h.revoke_receptor_consent(R);
    assert_eq!(
        h.receptor_activation(R),
        Activation::Disabled(DisabledReason::NoConsent)
    );
}

#[test]
fn ext10_opening_history_resets_consent() {
    let ir = ir();
    let env = build_envelope(&ir, &ir.starts[0].id, vec![], state()).unwrap();
    let bytes = export_bytes(&env).unwrap();
    let mut h = host().with_receptor_settings(ReceptorSettings::default().allow(R));
    h.grant_receptor_consent(R);
    assert_eq!(h.receptor_activation(R), Activation::Enabled);
    let outcome = h.open_history(&bytes);
    assert!(!matches!(outcome, ImportOutcome::Refused(_)), "{outcome:?}");
    assert_eq!(
        h.receptor_activation(R),
        Activation::Disabled(DisabledReason::NoConsent)
    );
}
