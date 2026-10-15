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

fn label_spec() -> AnswerSpec {
    serde_json::from_value(json!({
        "options": [
            {"id": "ok", "correct": true, "label": "正しい手順"},
            {"id": "asked-back", "correct": false}
        ],
        "on_wrong": "w"
    }))
    .expect("answer")
}

#[test]
fn pack23_label_is_displayed_and_missing_label_falls_back_to_id() {
    let a = label_spec();
    assert!(a.check().is_ok());
    assert_eq!(a.display_name("ok"), Some("正しい手順"));
    assert_eq!(a.display_name("asked-back"), Some("asked-back"));
}

#[test]
fn pack23_unknown_option_has_no_display_name() {
    let a = label_spec();
    assert_eq!(a.display_name("nope"), None);
    assert_eq!(a.selection_log_text("nope"), None);
    assert_eq!(a.recorded_value("nope"), None);
}

#[test]
fn pack23_blank_label_is_rejected() {
    for l in ["", "  ", "\u{3000}\n"] {
        let a: AnswerSpec = serde_json::from_value(json!({
            "options": [{"id": "x", "correct": true, "label": l}], "on_wrong": "w"
        }))
        .expect("readable");
        assert_eq!(a.check(), Err(ConfirmSpecError::EmptyName), "{l:?}");
        // 検証を経なくても空の表示は出さない。
        assert_eq!(a.display_name("x"), Some("x"));
    }
}

#[test]
fn pack23_recorded_value_is_the_id_not_the_label() {
    let a = label_spec();
    assert_eq!(a.recorded_value("ok"), Some("ok"));
    assert_eq!(a.recorded_value("asked-back"), Some("asked-back"));
}

#[test]
fn pack23_selection_log_text_uses_display_name() {
    let a = label_spec();
    assert_eq!(
        a.selection_log_text("ok").as_deref(),
        Some("選択: 正しい手順")
    );
    assert_eq!(
        a.selection_log_text("asked-back").as_deref(),
        Some("選択: asked-back")
    );
    assert_eq!(selection_log_text("{x}%s"), "選択: {x}%s");
}

#[test]
fn pack23_answer_spec_of_reads_only_answer() {
    let ok = json!({"kind": "answer", "options": [{"id": "a", "correct": true}], "on_wrong": "w"});
    assert!(answer_spec_of(&ok).is_some());
    assert!(answer_spec_of(&json!({"kind": "approve"})).is_none());
    assert!(answer_spec_of(&json!({"kind": "answer", "options": 1})).is_none());
    assert!(answer_spec_of(&json!({"options": []})).is_none());
    assert!(answer_spec_of(&json!("answer")).is_none());
}

#[test]
fn confirm_spec_contract_version_is_3() {
    assert_eq!(CONFIRM_SPEC_CONTRACT_VERSION, 3);
}

fn answer_with(options: serde_json::Value) -> AnswerSpec {
    serde_json::from_value(json!({"on_wrong": "rj", "options": options})).unwrap()
}

#[test]
fn pack15_on_wrong_on_correct_option_is_rejected() {
    let a = answer_with(json!([{"id": "a", "correct": true, "on_wrong": "rj2"}]));
    assert_eq!(a.check(), Err(ConfirmSpecError::OnWrongOnCorrectOption));
}

#[test]
fn pack15_on_wrong_on_wrong_option_and_null_are_accepted() {
    let a = answer_with(json!([
        {"id": "a", "correct": true, "on_wrong": null},
        {"id": "b", "correct": false, "on_wrong": "rj2"},
        {"id": "c", "correct": false}
    ]));
    assert_eq!(a.check(), Ok(()));
}

#[test]
fn pack15_blank_option_on_wrong_is_empty_name() {
    let a = answer_with(json!([
        {"id": "a", "correct": true},
        {"id": "b", "correct": false, "on_wrong": "  "}
    ]));
    assert_eq!(a.check(), Err(ConfirmSpecError::EmptyName));
}

#[test]
fn pack15_first_error_is_deterministic() {
    // 重複 ID と正答の on_wrong が両方あるとき、先に見る重複が返る。
    let a = answer_with(json!([
        {"id": "a", "correct": false},
        {"id": "a", "correct": true, "on_wrong": "rj2"}
    ]));
    assert_eq!(a.check(), Err(ConfirmSpecError::Duplicate));
}

#[test]
fn pack15_error_display_has_no_input_value() {
    let a = answer_with(json!([{"id": "a", "correct": true, "on_wrong": "SECRET-ID"}]));
    let msg = a.check().unwrap_err().to_string();
    assert_eq!(msg, "on_wrong on a correct option");
    assert!(!msg.contains("SECRET"));
}
