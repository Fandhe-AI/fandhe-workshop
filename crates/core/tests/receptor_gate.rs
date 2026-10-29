//! 受け口の有効化の関門の試験（TASK-191 1/2、EXT-10・D-53）。
//! 既定は無効で、環境の許可と受講者の同意の両方があるときだけ有効になることを固定する。

use fandhe_workshop_core::receptor_gate::{
    activation, is_enabled, Activation, DisabledReason, LearnerConsent, ReceptorId,
    ReceptorSettings, RECEPTOR_GATE_CONTRACT_VERSION,
};

const A: ReceptorId = ReceptorId::new("test-a");
const B: ReceptorId = ReceptorId::new("test-b");

#[test]
fn ext10_default_is_disabled() {
    let got = activation(&ReceptorSettings::default(), &LearnerConsent::default(), A);
    assert_eq!(got, Activation::Disabled(DisabledReason::NotConfigured));
}

#[test]
fn ext10_settings_without_consent_is_disabled() {
    let s = ReceptorSettings::default().allow(A);
    let got = activation(&s, &LearnerConsent::default(), A);
    assert_eq!(got, Activation::Disabled(DisabledReason::NoConsent));
}

#[test]
fn ext10_consent_without_settings_is_disabled() {
    let mut c = LearnerConsent::default();
    c.grant(A);
    let got = activation(&ReceptorSettings::default(), &c, A);
    assert_eq!(got, Activation::Disabled(DisabledReason::NotConfigured));
}

#[test]
fn ext10_both_enables_and_revoke_disables() {
    let s = ReceptorSettings::default().allow(A);
    let mut c = LearnerConsent::default();
    c.grant(A);
    assert_eq!(activation(&s, &c, A), Activation::Enabled);
    assert!(is_enabled(&s, &c, A));
    c.revoke(A);
    assert_eq!(
        activation(&s, &c, A),
        Activation::Disabled(DisabledReason::NoConsent)
    );
    assert!(!is_enabled(&s, &c, A));
}

#[test]
fn ext10_each_receptor_is_independent() {
    let s = ReceptorSettings::default().allow(A).allow(B);
    let mut c = LearnerConsent::default();
    c.grant(A);
    assert_eq!(activation(&s, &c, A), Activation::Enabled);
    assert_eq!(
        activation(&s, &c, B),
        Activation::Disabled(DisabledReason::NoConsent)
    );
}

#[test]
fn ext10_contract_version_is_one() {
    assert_eq!(RECEPTOR_GATE_CONTRACT_VERSION, 1);
}
