//! `pack.appearance` の閉じた集合と読み取りの契約試験（TASK-53 1/2、PACK-21）。

use fandhe_workshop_contract::appearance::{read_appearance, AppearanceReadError, Theme};
use serde_json::json;

#[test]
fn absent_and_empty_are_standard() {
    assert_eq!(read_appearance(None), Ok(Theme::Standard));
    assert_eq!(read_appearance(Some(&json!({}))), Ok(Theme::Standard));
}

#[test]
fn four_values_are_read() {
    for (s, t) in [
        ("standard", Theme::Standard),
        ("mono", Theme::Mono),
        ("high_contrast", Theme::HighContrast),
        ("reading", Theme::Reading),
    ] {
        assert_eq!(read_appearance(Some(&json!({"theme": s}))), Ok(t), "{s}");
    }
}

#[test]
fn as_str_and_from_vocab_round_trip() {
    for t in Theme::ALL {
        assert_eq!(Theme::from_vocab(t.as_str()), Some(t));
    }
}

#[test]
fn unknown_values_are_rejected() {
    for s in ["dark", "Standard", "high-contrast", ""] {
        assert_eq!(
            read_appearance(Some(&json!({"theme": s}))),
            Err(AppearanceReadError::ThemeUnknown),
            "{s:?}"
        );
    }
}

#[test]
fn non_string_theme_is_rejected() {
    for v in [json!(null), json!(5)] {
        assert_eq!(
            read_appearance(Some(&json!({ "theme": v }))),
            Err(AppearanceReadError::ThemeNotString)
        );
    }
}

#[test]
fn unknown_keys_are_rejected() {
    assert_eq!(
        read_appearance(Some(&json!({"style": "x"}))),
        Err(AppearanceReadError::UnknownKey("style".to_owned()))
    );
    assert_eq!(
        read_appearance(Some(&json!({"theme": "mono", "css": "x"}))),
        Err(AppearanceReadError::UnknownKey("css".to_owned()))
    );
}

#[test]
fn non_mapping_is_rejected() {
    for v in [json!([1]), json!("reading")] {
        assert_eq!(
            read_appearance(Some(&v)),
            Err(AppearanceReadError::NotAMapping)
        );
    }
}
