//! 接頭辞の規則・入力の分け・`pack.profile` の読み取りの契約試験（TASK-52 1/2）。

use fandhe_workshop_contract::profile::{
    check_command_prefix, classify_input, is_typable_without_prefix, read_profile,
    InputClass::{self, Command, Text},
    InputFieldMode, PrefixError, ProfileReadError,
};
use serde_json::json;

type Cases = &'static [(&'static str, InputClass)];

#[test]
fn input_split_table() {
    let table: &[(&str, Cases)] = &[
        (
            "/",
            &[
                ("/ls", Command),
                ("ls", Text),
                ("  /ls ", Command),
                ("\u{3000}/ls", Command),
                ("$ls", Text),
            ],
        ),
        (
            "$",
            &[("$hello", Command), ("/hello", Text), ("hello", Text)],
        ),
        ("::", &[("::x", Command), (":x", Text)]),
        ("!!!!", &[("!!!!a", Command), ("!!!a", Text)]),
        (
            "",
            &[
                ("/ls", Command),
                ("ls", Command),
                ("hello world", Command),
                ("$x", Command),
            ],
        ),
    ];
    for (prefix, cases) in table {
        for (raw, want) in *cases {
            assert_eq!(classify_input(prefix, raw), *want, "{prefix:?} {raw:?}");
        }
    }
}

#[test]
fn prefix_rules() {
    for ok in ["", "/", "$", ">", "::", "＄", "!!!!", "あいうえ"] {
        assert_eq!(check_command_prefix(ok), Ok(()), "{ok:?}");
    }
    let bad: &[(&str, PrefixError)] = &[
        ("abcde", PrefixError::TooLong),
        ("!!!!!", PrefixError::TooLong),
        ("\n", PrefixError::Control),
        ("\t", PrefixError::Control),
        ("/ ", PrefixError::Whitespace),
        ("\u{3000}", PrefixError::Whitespace),
        ("a", PrefixError::AsciiAlphanumeric),
        ("1", PrefixError::AsciiAlphanumeric),
        ("<b>", PrefixError::AsciiAlphanumeric),
        ("-", PrefixError::LeadingDash),
        ("-/", PrefixError::LeadingDash),
    ];
    for (s, e) in bad {
        assert_eq!(check_command_prefix(s), Err(*e), "{s:?}");
    }
}

#[test]
fn read_profile_cases() {
    let d = read_profile(None).unwrap();
    assert_eq!(d.command_prefix, "/");
    assert_eq!(d.input_field, InputFieldMode::Shown);
    assert_eq!(read_profile(Some(&json!({}))).unwrap(), d);
    let p = read_profile(Some(&json!({"command_prefix": "$"}))).unwrap();
    assert_eq!(p.command_prefix, "$");
    let p = read_profile(Some(&json!({"input_field": "hidden"}))).unwrap();
    assert_eq!(p.input_field, InputFieldMode::Hidden);
    assert_eq!(
        read_profile(Some(&json!({"x": 1}))),
        Err(ProfileReadError::UnknownKey("x".into()))
    );
    assert_eq!(
        read_profile(Some(&json!("s"))),
        Err(ProfileReadError::NotAMapping)
    );
    assert_eq!(
        read_profile(Some(&json!({"command_prefix": 1}))),
        Err(ProfileReadError::PrefixNotString)
    );
    assert_eq!(
        read_profile(Some(&json!({"command_prefix": null}))),
        Err(ProfileReadError::PrefixNotString)
    );
    assert_eq!(
        read_profile(Some(&json!({"input_field": "maybe"}))),
        Err(ProfileReadError::InputFieldInvalid)
    );
    assert_eq!(
        read_profile(Some(&json!({"command_prefix": "abcde"}))),
        Err(ProfileReadError::Prefix(PrefixError::TooLong))
    );
}

#[test]
fn typable_names() {
    for n in ["", "ls a", "ls\u{3000}a", "a\u{7}"] {
        assert!(!is_typable_without_prefix(n), "{n:?}");
    }
    for n in ["ls", "日本語"] {
        assert!(is_typable_without_prefix(n), "{n:?}");
    }
}
