//! 秘密情報らしい文字列の検出の試験（SAFE-27・TASK-164 1/3）。
//!
//! 値は `testdata/untrusted-input-safety/test-data-addendum-4/secret-shapes.json` の部品から実行時に組み立てる。
//! repo のどこにも完成した値は置かない。assert のメッセージには形の ID だけを入れ、値を出さない。
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "試験の前提が崩れたことを知らせる失敗"
)]

use fandhe_workshop_core::secret_scan::{
    scan_json_text, scan_json_value, scan_text, shapes_of, SecretShape,
};
use proptest::prelude::*;
use serde_json::{json, Value};

fn fixture() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../testdata/untrusted-input-safety/test-data-addendum-4/secret-shapes.json"
    );
    let text = std::fs::read_to_string(path).expect("secret-shapes.json is readable");
    serde_json::from_str(&text).expect("secret-shapes.json is JSON")
}

/// `{N×c}`・`{seqN}`・`\n` を展開する。知らない記法は失敗にする。
fn expand(part: &str) -> String {
    let mut out = String::new();
    let mut rest = part;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("\\n") {
            out.push('\n');
            rest = r;
        } else if let Some(r) = rest.strip_prefix('{') {
            let (body, tail) = r.split_once('}').expect("closed brace");
            if let Some(n) = body.strip_prefix("seq") {
                let n: usize = n.parse().expect("seq count");
                out.extend("0123456789abcdefghijklmnopqrstuvwxyz".chars().take(n));
            } else if let Some((n, c)) = body.split_once('\u{00D7}') {
                let n: usize = n.parse().expect("repeat count");
                out.extend(std::iter::repeat_n(c.chars().next().expect("char"), n));
            } else {
                panic!("unknown expansion form");
            }
            rest = tail;
        } else {
            let end = rest.find(['{', '\\']).unwrap_or(rest.len()).max(1);
            let (head, tail) = rest.split_at(end);
            out.push_str(head);
            rest = tail;
        }
    }
    out
}

fn shapes() -> Vec<(String, String, String)> {
    let f = fixture();
    f["shapes"]
        .as_array()
        .expect("shapes")
        .iter()
        .map(|s| {
            let parts: String = s["parts"]
                .as_array()
                .expect("parts")
                .iter()
                .map(|p| expand(p.as_str().expect("part")))
                .collect();
            (
                s["id"].as_str().expect("id").to_owned(),
                s["expect"].as_str().expect("expect").to_owned(),
                parts,
            )
        })
        .collect()
}

fn controls() -> Vec<(String, String)> {
    let f = fixture();
    ["controls", "extra_controls"]
        .iter()
        .flat_map(|k| f[*k].as_array().expect("controls").clone())
        .map(|c| {
            (
                c["id"].as_str().expect("id").to_owned(),
                c["text"].as_str().expect("text").to_owned(),
            )
        })
        .collect()
}

fn ids(v: &[fandhe_workshop_core::secret_scan::Finding]) -> Vec<&'static str> {
    shapes_of(v).into_iter().map(SecretShape::id).collect()
}

#[test]
fn all_ten_shapes_are_detected_with_the_expected_id() {
    let all = shapes();
    assert_eq!(all.len(), 10, "ten shapes in the fixture");
    for (id, expect, value) in all {
        let found = ids(&scan_text(&value));
        assert!(
            found == [expect.as_str()],
            "{id}: detected shape differs from {expect}"
        );
    }
}

#[test]
fn shapes_are_detected_inside_json_strings_and_nested_values() {
    for (id, expect, value) in shapes() {
        let doc = json!({"ops": [{"kind": "input", "text": value}], "draft": {"a": [value]}});
        let found = ids(&scan_json_value(&doc));
        assert!(
            found == [expect.as_str()],
            "{id}: nested value not detected as {expect}"
        );
        let text = doc.to_string();
        assert!(
            ids(&scan_json_text(&text)) == [expect.as_str()],
            "{id}: json text not detected"
        );
    }
}

#[test]
fn controls_are_not_detected() {
    let cs = controls();
    assert_eq!(cs.len(), 6, "six controls in the fixture");
    for (id, text) in cs {
        assert!(scan_text(&text).is_empty(), "{id}: false positive");
        assert!(
            scan_json_text(&json!({ "t": text }).to_string()).is_empty(),
            "{id}: false positive in json"
        );
    }
}

#[test]
fn a_prefix_inside_a_longer_word_is_not_detected() {
    for (id, _, value) in shapes() {
        if id == "S-05" || id == "S-09" || id == "S-10" {
            continue;
        }
        let glued = format!("x{value}");
        assert!(scan_text(&glued).is_empty(), "{id}: glued prefix detected");
    }
}

#[test]
fn keys_are_not_scanned() {
    let (_, _, value) = shapes().remove(0);
    let doc = json!({ value: 1 });
    assert!(scan_json_value(&doc).is_empty(), "key detected");
}

#[test]
fn fingerprint_is_stable_and_distinguishes_values() {
    let all = shapes();
    let a = scan_text(&all[0].2);
    let b = scan_text(&all[0].2);
    assert!(a == b, "fingerprint differs between runs");
    let other = scan_text(&all[5].2);
    assert!(
        a[0].fingerprint != other[0].fingerprint,
        "fingerprints collide"
    );
}

#[test]
fn ids_round_trip() {
    for s in SecretShape::ALL {
        assert!(SecretShape::from_id(s.id()) == Some(s), "{}", s.id());
        assert!(!s.label().is_empty(), "{}", s.id());
    }
    assert!(SecretShape::from_id("github_token ").is_none());
}

proptest! {
    #[test]
    fn arbitrary_text_never_panics(prefix in "\\PC{0,40}", suffix in "\\PC{0,40}", pick in 0usize..10) {
        let all = shapes();
        let text = format!("{prefix}{}{suffix}", all[pick].2);
        let _ = scan_text(&text);
        let _ = scan_json_text(&text);
    }

    #[test]
    fn truncated_values_never_panic(pick in 0usize..10, cut in 0usize..120) {
        let all = shapes();
        let v = &all[pick].2;
        let end = v.char_indices().map(|(i, _)| i).nth(cut).unwrap_or(v.len());
        let _ = scan_text(&v[..end]);
    }
}

#[test]
fn private_key_without_an_end_line_is_still_detected() {
    let text = ["-----BEGIN RSA ", "PRIVATE KEY-----\nAAAA"].concat();
    assert!(ids(&scan_text(&text)) == ["private_key"]);
}

#[test]
fn aws_secret_name_is_matched_ignoring_ascii_case() {
    let (_, _, value) = shapes()
        .into_iter()
        .find(|(id, _, _)| id == "S-05")
        .expect("S-05 in the fixture");
    let name = "aws_secret_access_key";
    assert!(value.starts_with(name), "S-05 starts with the name");
    for variant in ["AWS_SECRET_ACCESS_KEY", "AWS_Secret_Access_Key"] {
        let text = value.replacen(name, variant, 1);
        assert!(
            ids(&scan_text(&text)) == ["aws_secret_key"],
            "{variant}: not detected"
        );
        assert!(
            scan_text(&format!("x{text}")).is_empty(),
            "{variant}: glued name detected"
        );
    }
}

/// 接頭辞の反復で二次時間にならないこと（履歴の保存のたびにメインスレッドで同期実行されるため）。
/// 二次なら数十秒かかる大きさを与え、十分に余裕のある上限で線形を確かめる。
#[test]
fn repeated_prefixes_are_scanned_in_linear_time() {
    let n = 200_000;
    let key_line = ["-----BEGIN PRIVATE", " KEY-----\n"].concat();
    let jwt_head = format!("eyJ{}", "a".repeat(8));
    let cases: Vec<(&str, String)> = vec![
        ("jwt_dash_eyj", "-eyJ".repeat(n)),
        (
            "jwt_second_part",
            format!("{}.eyJ{}", "eyJaaaaaaaa-".repeat(n), "b".repeat(n)),
        ),
        ("slack", "xoxb-".repeat(n)),
        ("aws_access", "AKIA".repeat(n)),
        ("aws_secret", "aws_secret_access_key=".repeat(n)),
        ("private_key_no_tail", "-----BEGIN ".repeat(n)),
        ("private_key_no_end", key_line.repeat(n)),
        (
            "private_key_with_end",
            format!("{}-----END ", key_line.repeat(n)),
        ),
        ("jwt_head_only", jwt_head.repeat(n)),
    ];
    for (name, text) in &cases {
        let start = std::time::Instant::now();
        let found = scan_text(text);
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "case {name} took {elapsed:?}"
        );
        assert!(
            found.len() <= n,
            "case {name} produced {} findings",
            found.len()
        );
    }
}
