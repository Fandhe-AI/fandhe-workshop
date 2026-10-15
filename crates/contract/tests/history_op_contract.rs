//! `HistoryOp` の直列化の形・拒否・決定値の適合試験（TASK-309 1/3）。
//! 操作 `E` には core に依存しない試験用の代役を使う。

use fandhe_workshop_contract::history::{
    ExportForm, HistoryOp, EXPORT_FORM, EXPORT_INCLUDES_DRAFT, HISTORY_CONTRACT_VERSION,
    OPS_RECORDS_FAST_FORWARD,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum StubOp {
    Input(String),
    Tick(u32),
    Control(String),
}

type Op = HistoryOp<StubOp>;

fn json(op: &Op) -> String {
    serde_json::to_string(op).unwrap()
}

#[test]
fn each_variant_has_exact_wire_form() {
    assert_eq!(json(&Op::Submit("/help".into())), r#"{"submit":"/help"}"#);
    assert_eq!(json(&Op::Draft("/help".into())), r#"{"draft":"/help"}"#);
    assert_eq!(json(&Op::Reload), r#""reload""#);
    assert_eq!(
        json(&Op::Engine(StubOp::Input("x".into()))),
        r#"{"input":"x"}"#
    );
    assert_eq!(json(&Op::Engine(StubOp::Tick(3))), r#"{"tick":3}"#);
    assert_eq!(
        json(&Op::Engine(StubOp::Control("save".into()))),
        r#"{"control":"save"}"#
    );
}

#[test]
fn draft_submit_draft_sequence_is_byte_exact_and_roundtrips() {
    let ops: Vec<Op> = vec![
        Op::Draft("/help".into()),
        Op::Submit("/help".into()),
        Op::Draft("未送信のメモ".into()),
    ];
    let s = serde_json::to_string(&ops).unwrap();
    assert_eq!(
        s,
        r#"[{"draft":"/help"},{"submit":"/help"},{"draft":"未送信のメモ"}]"#
    );
    assert_eq!(serde_json::from_str::<Vec<Op>>(&s).unwrap(), ops);
}

#[test]
fn mixed_sequence_keeps_order() {
    let ops: Vec<Op> = vec![
        Op::Engine(StubOp::Tick(1)),
        Op::Reload,
        Op::Draft("a".into()),
        Op::Engine(StubOp::Input("b".into())),
        Op::Submit("c".into()),
        Op::Engine(StubOp::Control("go".into())),
        Op::Reload,
    ];
    let s = serde_json::to_string(&ops).unwrap();
    assert_eq!(serde_json::from_str::<Vec<Op>>(&s).unwrap(), ops);
}

#[test]
fn unknown_or_malformed_ops_are_rejected() {
    for bad in [
        r#"{"enter":"x"}"#,
        r#"{"fast_forward":true}"#,
        r#"{"draft":1}"#,
        r#"{"submit":null}"#,
        r#""unknown""#,
        r#"{"submit":"a","draft":"b"}"#,
        r#"{}"#,
        r#"null"#,
    ] {
        assert!(serde_json::from_str::<Op>(bad).is_err(), "accepted: {bad}");
    }
}

#[test]
fn long_multibyte_submit_is_kept_whole() {
    let text: String = "あa".repeat(10_000);
    assert_eq!(text.chars().count(), 20_000);
    let op = Op::Submit(text.clone());
    let back: Op = serde_json::from_str(&json(&op)).unwrap();
    assert_eq!(back, Op::Submit(text));
}

#[test]
fn untrusted_text_is_not_altered() {
    let text = "<script>alert(1)</script>&\"'";
    let back: Op = serde_json::from_str(&json(&Op::Draft(text.into()))).unwrap();
    assert_eq!(back, Op::Draft(text.into()));
}

#[test]
fn decisions_are_fixed() {
    assert_eq!(EXPORT_FORM, ExportForm::OpsAndSnapshot);
    let (draft, fast_forward) = (EXPORT_INCLUDES_DRAFT, OPS_RECORDS_FAST_FORWARD);
    assert!(draft);
    assert!(!fast_forward);
    assert_eq!(HISTORY_CONTRACT_VERSION, 1);
}
