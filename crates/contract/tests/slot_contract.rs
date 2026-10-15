//! 差し込みの枠（`slot`）の契約の適合試験（PACK-16、TASK-46 2/2）。
//!
//! 変異注入（`tests/mutations/PACK16-*.mutation`）が検出する対象として、構文の誤り・再走査をしない性質・
//! エラーの固定文を公開 API だけで確かめる。表の中身（分類の制限・空になりうるか）は pack の結合試験と
//! `slot.rs` の単体試験が確かめる。

use fandhe_workshop_contract::slot::{
    parse_slots, substitute, SlotName, SlotSyntaxError, SlotValues, SubstituteError,
    SLOT_CONTRACT_VERSION,
};

fn values() -> SlotValues {
    SlotValues {
        input_head: Some("/hlep".to_string()),
        nearest_command: None,
        step_title: "題名".to_string(),
        step_do: Some("やること".to_string()),
        remaining: Some(2),
        total: Some(5),
    }
}

#[test]
fn pack16_contract_version_is_1() {
    assert_eq!(SLOT_CONTRACT_VERSION, 1);
}

#[test]
fn pack16_parse_rejects_unclosed() {
    assert_eq!(parse_slots("{{step_title"), Err(SlotSyntaxError::Unclosed));
    assert_eq!(parse_slots("a {{x|y"), Err(SlotSyntaxError::Unclosed));
}

#[test]
fn pack16_parse_rejects_empty_name_and_nesting() {
    assert_eq!(parse_slots("{{ }}"), Err(SlotSyntaxError::EmptyName));
    assert_eq!(parse_slots("{{a|{{b}}"), Err(SlotSyntaxError::Nested));
}

#[test]
fn pack16_substitute_does_not_rescan_inserted_values() {
    let mut v = values();
    v.input_head = Some("{{step_title}}".to_string());
    assert_eq!(
        substitute("[{{input_head}}]", &v).as_deref(),
        Ok("[{{step_title}}]")
    );
}

#[test]
fn pack16_substitute_fails_closed() {
    assert_eq!(
        substitute("{{nearest|x}}", &values()),
        Err(SubstituteError::Unknown)
    );
    assert!(substitute("{{step_title", &values()).is_err());
}

#[test]
fn pack16_names_are_the_closed_set_of_six() {
    let names: Vec<&str> = SlotName::ALL.iter().map(|n| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "input_head",
            "nearest_command",
            "step_title",
            "step_do",
            "remaining",
            "total"
        ]
    );
}

#[test]
fn pack16_errors_do_not_embed_input() {
    let secret = "SECRET-SLOT-VALUE";
    let e = substitute(&format!("{{{{{secret}}}}}"), &values()).unwrap_err();
    assert!(!e.to_string().contains(secret));
    let e = parse_slots(&format!("{{{{{secret}")).unwrap_err();
    assert!(!e.to_string().contains(secret));
}
