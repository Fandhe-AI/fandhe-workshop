//! 教材パックの `validate` が秘密情報らしい値を `E-SECRET-SHAPE` で報告する試験（AUTHOR-18・D-81。TASK-420 1/2）。
//!
//! 値は `testdata/` の部品から実行時に組み立て、repo のどこにも完成した値を置かない。assert のメッセージにも値を出さない。
//! 基点は `docs/examples/minimal-pack`（診断 0 件）の写しで、値を 1 か所ずつ入れて検出を確かめる。
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "試験の前提が崩れたことを知らせる失敗"
)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fandhe_workshop_pack::{validate, DiagCode, Diagnostic};
use serde_json::Value;

mod common;
use common::{copy_dir, testdata, Tmp};

const BODY: &str = "chapters/c-mini/st-hello.md";

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

fn load(rel: &str) -> Value {
    let text = fs::read_to_string(testdata(rel)).expect("fixture readable");
    serde_json::from_str(&text).expect("fixture is JSON")
}

/// （試験データの ID, 期待する形の ID, 組み立てた値）を 15 形ぶん。
fn shapes() -> Vec<(String, String, String)> {
    let mut out = Vec::new();
    for rel in [
        "untrusted-input-safety/test-data-addendum-4/secret-shapes.json",
        "secret-shapes-added/added-shapes.json",
    ] {
        for s in load(rel)["shapes"].as_array().expect("shapes") {
            let v: String = s["parts"]
                .as_array()
                .expect("parts")
                .iter()
                .map(|p| expand(p.as_str().expect("part")))
                .collect();
            out.push((
                s["id"].as_str().expect("id").to_owned(),
                s["expect"].as_str().expect("expect").to_owned(),
                v,
            ));
        }
    }
    out
}

/// 検出されない対照の文章。
fn controls() -> Vec<(String, String)> {
    let mut out = Vec::new();
    let f = load("untrusted-input-safety/test-data-addendum-4/secret-shapes.json");
    let added = load("secret-shapes-added/added-shapes.json");
    for arr in [&f["controls"], &f["extra_controls"], &added["controls"]] {
        for c in arr.as_array().expect("controls") {
            out.push((
                c["id"].as_str().expect("id").to_owned(),
                c["text"].as_str().expect("text").to_owned(),
            ));
        }
    }
    out
}

fn base() -> Tmp {
    let t = Tmp::new("secret-shape");
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/examples/minimal-pack");
    copy_dir(&src, &t.0);
    t
}

fn edit(t: &Tmp, rel: &str, from: &str, to: &str) {
    let p = t.0.join(rel);
    let text = fs::read_to_string(&p).expect("read");
    assert!(text.contains(from), "edit target missing in {rel}");
    fs::write(&p, text.replacen(from, to, 1)).expect("write");
}

/// YAML の流れの文字列として安全に埋め込める引用つきの値（JSON の文字列は YAML の二重引用でもある）。
fn quoted(v: &str) -> String {
    serde_json::to_string(v).expect("quote")
}

fn secret_diags(dir: &Path) -> Vec<Diagnostic> {
    validate::validate_dir(dir)
        .into_iter()
        .filter(|d| d.code() == DiagCode::SecretShape)
        .collect()
}

#[test]
fn author18_baseline_is_clean() {
    let t = base();
    assert!(validate::validate_dir(&t.0).is_empty());
}

#[test]
fn author18_each_shape_in_body_is_reported_once() {
    for (id, shape, value) in shapes() {
        let t = base();
        fs::write(t.0.join(BODY), format!("# a\n{value}\n")).expect("write");
        let d = secret_diags(&t.0);
        assert_eq!(d.len(), 1, "{id}: expected one diagnostic, got {}", d.len());
        assert!(
            d[0].message().contains(&shape),
            "{id}: message lacks shape {shape}"
        );
        assert_eq!(d[0].file(), BODY, "{id}");
        assert_eq!(d[0].item(), "file", "{id}");
    }
}

#[test]
fn author18_placements_report_file_and_item() {
    let (_, shape, value) = shapes().remove(0);
    let q = quoted(&value);

    let t = base();
    fs::write(t.0.join("fixtures/item.txt"), format!("{value}\n")).expect("write");
    let d = secret_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].file(), d[0].item()),
        ("fixtures/item.txt", "fixture:fx-item")
    );
    assert!(d[0].message().contains(&shape));

    let t = base();
    edit(
        &t,
        "chapters/c-mini/chapter.yaml",
        "title: あいさつ",
        &format!("title: {q}"),
    );
    let d = secret_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].file(), d[0].item()),
        ("chapters/c-mini/chapter.yaml", "step:st-hello")
    );

    let t = base();
    edit(
        &t,
        "workshop.yaml",
        "message: 一覧にないコマンドです。",
        &format!("message: {q}"),
    );
    let d = secret_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].file(), d[0].item()),
        ("workshop.yaml", "rejection:rj-unknown")
    );

    let t = base();
    edit(
        &t,
        "tests/playthrough.yaml",
        "input: { text: hello,",
        &format!("input: {{ text: {q},"),
    );
    let d = secret_diags(&t.0);
    assert_eq!(d.len(), 1);
    assert_eq!(
        (d[0].file(), d[0].item()),
        ("tests/playthrough.yaml", "error_input:e-text")
    );
}

#[test]
fn author18_diagnostics_never_contain_the_value() {
    for (id, _, value) in shapes() {
        let t = base();
        fs::write(t.0.join(BODY), format!("# a\n{value}\n")).expect("write");
        let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
            .arg("validate")
            .arg(&t.0)
            .output()
            .expect("run");
        let all = format!(
            "{}{}",
            String::from_utf8_lossy(&o.stdout),
            String::from_utf8_lossy(&o.stderr)
        );
        // 値の末尾の 8 文字が現れないこと（形の ID は値の部品と重ならない）。
        let head: String = value.chars().rev().take(8).collect();
        assert!(!all.contains(&head), "{id}: output leaks value head");
        assert!(all.contains("E-SECRET-SHAPE"), "{id}");
        assert_eq!(o.status.code(), Some(1), "{id}");
        let last = String::from_utf8_lossy(&o.stdout)
            .lines()
            .last()
            .expect("last line")
            .to_owned();
        assert!(last.starts_with("NG: 診断 1 件"), "{id}: {last}");
    }
}

#[test]
fn author18_controls_are_not_reported() {
    let t = base();
    let mut text = String::from("# a\n");
    for (_, c) in controls() {
        text.push_str(&c);
        text.push_str("\n\n");
    }
    fs::write(t.0.join(BODY), text).expect("write");
    assert!(secret_diags(&t.0).is_empty());
}

#[test]
fn author18_test_command_stops_at_the_gate() {
    let (_, _, value) = shapes().remove(0);
    let t = base();
    fs::write(t.0.join(BODY), format!("# a\n{value}\n")).expect("write");
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("test")
        .arg(&t.0)
        .output()
        .expect("run");
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-SECRET-SHAPE"));
    let head: String = value.chars().rev().take(8).collect();
    assert!(!out.contains(&head));
}

fn pack_dirs(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = fs::read_dir(root) else { return };
    if root.join("workshop.yaml").is_file() || root.join("workshop.toml").is_file() {
        out.push(root.to_path_buf());
    }
    let mut kids: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
    kids.sort();
    for k in kids {
        let is_real_dir = fs::symlink_metadata(&k).is_ok_and(|m| m.is_dir());
        if is_real_dir {
            pack_dirs(&k, out);
        }
    }
}

#[test]
fn author18_no_false_positive_in_repo_packs() {
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
        let n = secret_diags(&d).len();
        assert_eq!(n, 0, "false positive in {}", d.display());
    }
}
