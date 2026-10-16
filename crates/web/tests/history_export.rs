//! 履歴の書き出しファイルの名前と種類（`history_export`、TASK-330 1/2）の試験。
//!
//! `pack_id` は教材由来で信頼できない。どんな入力でも、名前が固定の形に合い、パスの区切りを含まないことを確かめる。

use fandhe_workshop_web::history_export::{file_name, MIME};

fn is_valid(name: &str) -> bool {
    let Some(rest) = name.strip_prefix("fandhe-workshop-history") else {
        return false;
    };
    let Some(mid) = rest.strip_suffix(".json") else {
        return false;
    };
    if mid.is_empty() {
        return true;
    }
    let Some(pack) = mid.strip_prefix('-') else {
        return false;
    };
    (1..=64).contains(&pack.len())
        && pack
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}

#[test]
fn plain_pack_id_is_kept() {
    assert_eq!(
        file_name("bbs-tutorial"),
        "fandhe-workshop-history-bbs-tutorial.json"
    );
}

#[test]
fn hostile_pack_ids_become_safe_names() {
    let long = "a".repeat(200);
    let cases = [
        "",
        "Upper",
        "a/b",
        "../etc/passwd",
        "..",
        "a\\b",
        "line\nbreak",
        "nul\0byte",
        "日本語",
        "a b",
        &long,
    ];
    for case in cases {
        let name = file_name(case);
        assert!(
            is_valid(&name),
            "unsafe name for an input of {} bytes",
            case.len()
        );
        assert!(!name.contains('/') && !name.contains('\\') && !name.contains(".."));
    }
}

#[test]
fn empty_pack_id_gives_the_prefix_only() {
    assert_eq!(file_name(""), "fandhe-workshop-history.json");
}

#[test]
fn long_pack_id_is_cut_at_64() {
    let name = file_name(&"a".repeat(65));
    assert_eq!(
        name,
        format!("fandhe-workshop-history-{}.json", "a".repeat(64))
    );
}

#[test]
fn mime_is_json() {
    assert_eq!(MIME, "application/json");
}
