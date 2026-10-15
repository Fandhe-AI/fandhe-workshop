//! 確認 4 種の宣言の型と、受付・達成の分け方の契約の適合試験（TASK-47、PACK-13）。
//!
//! 語彙の文書との一致は `confirm_spec_vocabulary.rs`、ここでは型の形・意味の制約・R-22 の静的な規則を確かめる。

use fandhe_workshop_contract::confirm_spec::*;
use serde_json::json;

#[test]
fn four_kinds_are_read() {
    let p: PointAtSpec = serde_json::from_value(
        json!({"targets": [{"view": "files", "path": "a"}], "on_miss": "m"}),
    )
    .expect("point_at");
    assert!(p.check().is_ok());
    let a: AnswerSpec = serde_json::from_value(json!({
        "options": [{"id": "x", "correct": true, "label": "X"}, {"id": "y", "correct": false, "on_wrong": "w"}],
        "on_wrong": "w"
    }))
    .expect("answer");
    assert!(a.check().is_ok());
    assert!(serde_json::from_value::<ApproveSpec>(json!({})).is_ok());
    let n: NoneSpec = serde_json::from_value(json!({"reason": "why"})).expect("none");
    assert!(n.has_reason());
}

#[test]
fn unknown_fields_are_rejected() {
    assert!(serde_json::from_value::<ApproveSpec>(json!({"x": 1})).is_err());
    assert!(serde_json::from_value::<NoneSpec>(json!({"reason": "r", "x": 1})).is_err());
    assert!(serde_json::from_value::<PointAtSpec>(json!({"targets": [], "script": "x"})).is_err());
}

#[test]
fn none_reason_must_be_non_blank() {
    for r in [
        json!({}),
        json!({"reason": ""}),
        json!({"reason": " \u{3000}\n"}),
    ] {
        let n: NoneSpec = serde_json::from_value(r).expect("readable");
        assert!(!n.has_reason());
    }
}

#[test]
fn answer_semantics() {
    let opt = |id: &str, correct: bool| AnswerOption {
        id: id.into(),
        correct,
        on_wrong: None,
        label: None,
    };
    let spec = |options: Vec<AnswerOption>, on_wrong: &str| AnswerSpec {
        options,
        on_wrong: on_wrong.into(),
    };
    assert_eq!(spec(vec![], "w").check(), Err(ConfirmSpecError::EmptyList));
    assert_eq!(
        spec(vec![opt("a", false)], "w").check(),
        Err(ConfirmSpecError::NoCorrectOption)
    );
    assert_eq!(
        spec(vec![opt("a", true), opt("a", false)], "w").check(),
        Err(ConfirmSpecError::Duplicate)
    );
    assert_eq!(
        spec(vec![opt(" ", true)], "w").check(),
        Err(ConfirmSpecError::EmptyName)
    );
    assert_eq!(
        spec(vec![opt("a", true)], "").check(),
        Err(ConfirmSpecError::EmptyName)
    );
}

#[test]
fn point_at_semantics() {
    let t = |view: &str, path: &str| UiTarget {
        view: view.into(),
        path: path.into(),
    };
    let spec = |targets| PointAtSpec {
        targets,
        on_miss: None,
    };
    assert_eq!(spec(vec![]).check(), Err(ConfirmSpecError::EmptyList));
    assert_eq!(
        spec(vec![t("", "p")]).check(),
        Err(ConfirmSpecError::EmptyName)
    );
    assert_eq!(
        spec(vec![t("v", " ")]).check(),
        Err(ConfirmSpecError::EmptyName)
    );
}

/// R-22 の静的な規則。省略の明示（`none`）だけが受付と同時の達成で、確認が無い場合も受付で達成にしない。
#[test]
fn achievement_table() {
    assert_eq!(achievement(Some(ConfirmKind::None)), Achievement::OnAccept);
    for k in [
        ConfirmKind::PointAt,
        ConfirmKind::Answer,
        ConfirmKind::Approve,
    ] {
        assert_eq!(achievement(Some(k)), Achievement::AfterConfirm(k));
    }
    assert_ne!(achievement(None), Achievement::OnAccept);
}
