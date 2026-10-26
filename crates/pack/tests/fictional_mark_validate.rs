//! 教材パックの `validate` が練習データの【架空】の印の欠けを `E-FICTIONAL-MARK` で報告する試験（AUTHOR-18・D-81。TASK-420 2/2）。
//!
//! 基点は `docs/examples/minimal-pack`（診断 0 件。練習データ `fixtures/item.txt` は 1 つ）の写しで、練習データを 1 か所ずつ
//! 書き換えて確かめる。実在の名前の検査は D-81 により `validate` の対象外（公開前の確認の受け持ち）なので試験しない。
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "試験の前提が崩れたことを知らせる失敗"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_workshop_pack::{compile, validate, DiagCode, Diagnostic};

mod common;
use common::{copy_dir, Tmp};

const ITEM: &str = "fixtures/item.txt";

fn base() -> Tmp {
    let t = Tmp::new("fictional-mark");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/minimal-pack");
    copy_dir(&src, &t.0);
    t
}

fn mark_diags(dir: &Path) -> Vec<Diagnostic> {
    validate::validate_dir(dir)
        .into_iter()
        .filter(|d| d.code() == DiagCode::FictionalMark)
        .collect()
}

fn put_item(t: &Tmp, text: &str) {
    fs::write(t.0.join(ITEM), text).expect("write");
}

#[test]
fn author18_mark_baseline_is_clean() {
    let t = base();
    assert!(validate::validate_dir(&t.0).is_empty());
}

#[test]
fn author18_mark_missing_is_reported_once() {
    let t = base();
    put_item(&t, "x\n");
    let d = mark_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].file(), d[0].item()), (ITEM, "fixture:fx-item"));
}

#[test]
fn author18_mark_variants_pass() {
    for text in [
        "【架空ログ】x\n",
        "# 【架空】x\n",
        "\n  \n【架空】x\n",
        "\u{feff}【架空】x\n",
        "x 【架空】\n",
    ] {
        let t = base();
        put_item(&t, text);
        assert!(mark_diags(&t.0).is_empty(), "variant should pass");
    }
    for text in ["[架空] x\n", "【架空 x\n", "架空 x\n", ""] {
        let t = base();
        put_item(&t, text);
        assert_eq!(mark_diags(&t.0).len(), 1, "variant should be reported");
    }
}

#[test]
fn author18_mark_json_note_passes() {
    let json_pack = |body: &str| {
        let t = base();
        fs::write(t.0.join("fixtures/item.json"), body).expect("write");
        let p = t.0.join("workshop.yaml");
        let y = fs::read_to_string(&p).expect("read");
        assert!(y.contains(ITEM));
        fs::write(&p, y.replacen(ITEM, "fixtures/item.json", 1)).expect("write");
        t
    };
    let t = json_pack("{\"_note\": \"【架空】の値\", \"a\": 1}\n");
    assert!(mark_diags(&t.0).is_empty());
    // 入れ子の中だけにある印は通さない。
    let t = json_pack("{\"a\": {\"_note\": \"【架空】\"}}\n");
    assert_eq!(mark_diags(&t.0).len(), 1);
    // JSON として読めないときは先頭行の規則に倒す。
    let t = json_pack("【架空】 not json\n");
    assert!(mark_diags(&t.0).is_empty());
    let t = json_pack("not json\n");
    assert_eq!(mark_diags(&t.0).len(), 1);
}

#[test]
fn author18_mark_not_in_heading_is_reported() {
    for (name, text) in [
        ("item.txt", "x\n【架空】\n"),
        ("item.txt", "x\ny\nz\n【架空】\n"),
    ] {
        let t = base();
        put_item(&t, text);
        assert_eq!(mark_diags(&t.0).len(), 1, "{name}");
    }
}

#[test]
fn author18_mark_shared_path_reported_once() {
    let t = base();
    put_item(&t, "x\n");
    let p = t.0.join("workshop.yaml");
    let y = fs::read_to_string(&p).expect("read");
    let line = "  fx-item: { path: fixtures/item.txt, kind: file }\n";
    assert!(y.contains(line));
    let two = format!("{line}  fx-item2: {{ path: fixtures/item.txt, kind: file }}\n");
    fs::write(&p, y.replacen(line, &two, 1)).expect("write");
    let d = mark_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].item(), "fixture:fx-item");
}

#[test]
fn author18_mark_output_has_no_fixture_text() {
    let t = base();
    put_item(&t, "SENTINEL-BODY-TEXT\n");
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("validate")
        .arg(&t.0)
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-FICTIONAL-MARK"));
    assert!(!out.contains("SENTINEL-BODY-TEXT"));
    assert_eq!(out.lines().filter(|l| l.contains("E-")).count(), 1);
}

#[test]
fn author18_mark_test_command_stops_at_the_gate() {
    let t = base();
    put_item(&t, "x\n");
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(&t.0)
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(1));
    let all = format!(
        "{}{}",
        String::from_utf8_lossy(&o.stdout),
        String::from_utf8_lossy(&o.stderr)
    );
    assert!(all.contains("E-FICTIONAL-MARK"));
}

#[test]
fn author18_mark_compile_does_not_report() {
    let t = base();
    put_item(&t, "x\n");
    assert!(compile::compile_dir(&t.0).is_ok());
}

fn pack_dirs(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(root) else { return };
    if root.join("workshop.yaml").is_file() || root.join("workshop.toml").is_file() {
        out.push(root.to_path_buf());
    }
    let mut kids: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    kids.sort();
    for k in kids {
        if fs::symlink_metadata(&k).is_ok_and(|m| m.is_dir()) {
            pack_dirs(&k, out);
        }
    }
}

#[test]
fn author18_mark_no_false_positive_in_repo_packs() {
    let m = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut dirs = Vec::new();
    for root in [
        m.join("../../docs/examples"),
        m.join("../../testdata"),
        m.join("tests/data"),
        m.join("../web/tests/data"),
    ] {
        pack_dirs(&root, &mut dirs);
    }
    assert!(dirs.len() >= 10, "too few packs found: {}", dirs.len());
    for d in dirs {
        assert_eq!(
            mark_diags(&d).len(),
            0,
            "unmarked practice data in {}",
            d.display()
        );
    }
}
