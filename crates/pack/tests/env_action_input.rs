//! 保存・書き出しを Step の入力として宣言する語 `env_action` の試験（TASK-64 1/2、PACK-34・D-69）。
//!
//! 決定（D-69、2026-10-21）は「実行環境の操作（保存・書き出し）を Step の入力として宣言する語を足す」である。
//! 語彙 v1.5 の 4 語（`env_action`・`input.action`・`input.action=save`・`input.action=export`）として、実パック
//! （`tests/data/env-action-pack`）について次を確かめる。
//!
//! - 宣言が `validate`・`compile` を診断 0 件で通り、IR の `input` に宣言のまま出ること
//! - 型で読めない宣言（`action` の欠け・未知の値・未知の欄）は `E-PARSE`、`processing` を持つ Step は `E-STRUCTURE` で、
//!   診断に宣言の値が出ないこと
//! - `pack.vocab` が 1.5 より古いと `E-VOCAB-MISMATCH` が 1 件になること
//! - `input_field: hidden` の教材でも `E-PROFILE-*` が出ないこと（打鍵の入力ではないため）
//! - core の試験データ（`crates/core/tests/data/env_action_1/ir.json`）が、この教材の `compile` とバイト一致すること
//!
//! 確かめないこと: 実行時の受付・達成（core の `tests/env_action_save.rs`）。試験では `unwrap` を使ってよい。

mod common;

use std::path::{Path, PathBuf};

use common::{copy_dir, Tmp};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};
use serde_json::{json, Value};

const CHAPTER: &str = "chapters/c1/chapter.yaml";
const WORKSHOP: &str = "workshop.yaml";
const SAVE: &str = "input: {kind: env_action, action: save}";

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/env-action-pack")
}

/// 写しを作り、`file` の `from` を `to` に置き換える（置換元が 1 回だけあることを確かめる）。
fn edited(tag: &str, file: &str, from: &str, to: &str) -> Tmp {
    let tmp = Tmp::new(tag);
    copy_dir(&pack_dir(), &tmp.0);
    let path = tmp.0.join(file);
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches(from).count(), 1, "置換元がある: {from}");
    std::fs::write(&path, text.replace(from, to)).unwrap();
    tmp
}

fn codes(d: &[Diagnostic]) -> Vec<DiagCode> {
    d.iter().map(Diagnostic::code).collect()
}

#[test]
fn pack34_declaration_passes_validate_and_compile() {
    let diags = validate_dir(&pack_dir());
    assert!(diags.is_empty(), "{diags:?}");
    assert!(compile_dir(&pack_dir()).is_ok());
}

#[test]
fn pack34_ir_keeps_the_declaration_as_written() {
    let ir: Value = serde_json::from_slice(&compile_dir(&pack_dir()).unwrap()).unwrap();
    let input_of = |uid: &str| {
        ir["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["uid"] == uid)
            .unwrap()["step"]["input"]
            .clone()
    };
    assert_eq!(
        input_of("main/st-save"),
        json!({"kind": "env_action", "action": "save"})
    );
    assert_eq!(
        input_of("main/st-export"),
        json!({"kind": "env_action", "action": "export"})
    );
}

#[test]
fn pack34_core_test_ir_matches_the_pack() {
    let want = compile_dir(&pack_dir()).unwrap();
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../core/tests/data/env_action_1/ir.json");
    let have = std::fs::read(path).unwrap();
    assert_eq!(
        String::from_utf8(have).unwrap(),
        String::from_utf8(want).unwrap(),
        "core の試験データを教材から作り直す（fandhe-pack compile -o）"
    );
}

#[test]
fn pack34_unreadable_declarations_are_parse_errors() {
    let secret = "SECRET-ACT-4711";
    for (tag, to) in [
        ("missing", "input: {kind: env_action}".to_string()),
        (
            "unknown-value",
            format!("input: {{kind: env_action, action: {secret}}}"),
        ),
        (
            "unknown-key",
            "input: {kind: env_action, action: save, name: x}".to_string(),
        ),
        ("type", "input: {kind: env_action, action: 1}".to_string()),
    ] {
        let tmp = edited(&format!("env-action-{tag}"), CHAPTER, SAVE, &to);
        let d = validate_dir(&tmp.0);
        assert_eq!(codes(&d), [DiagCode::Parse], "{tag}: {d:?}");
        assert!(
            compile_dir(&tmp.0)
                .expect_err("宣言の誤りは IR を出さない")
                .iter()
                .any(|d| d.code() == DiagCode::Parse),
            "{tag}"
        );
        for diag in &d {
            let text = diag.to_string();
            assert!(!text.contains(secret), "{tag}: {text}");
        }
    }
}

#[test]
fn pack34_processing_is_a_structure_error() {
    let tmp = edited(
        "env-action-processing",
        CHAPTER,
        "    input: {kind: env_action, action: save}\n",
        "    input: {kind: env_action, action: save}\n    processing: [{name: p1}]\n",
    );
    let d = validate_dir(&tmp.0);
    assert!(
        d.iter().any(|d| d.code() == DiagCode::Structure
            && d.to_string().contains("env_action")
            && d.to_string().contains("processing is not allowed")),
        "{d:?}"
    );
}

#[test]
fn pack34_older_vocab_declaration_is_a_mismatch() {
    let old = edited(
        "env-action-vocab-old",
        WORKSHOP,
        "vocab: '1.5'",
        "vocab: '1.4'",
    );
    let d = validate_dir(&old.0);
    assert_eq!(codes(&d), [DiagCode::VocabMismatch], "{d:?}");
    assert!(d[0].to_string().contains("env_action"), "{}", d[0]);
    // 省略は検査しない。
    let none = edited("env-action-vocab-none", WORKSHOP, ", vocab: '1.5'", "");
    assert!(validate_dir(&none.0).is_empty());
}

#[test]
fn pack34_hidden_input_field_profile_is_allowed() {
    let tmp = edited(
        "env-action-hidden",
        WORKSHOP,
        "guidance: A, vocab: '1.5'}",
        "guidance: A, vocab: '1.5', profile: {input_field: hidden}}",
    );
    let d = validate_dir(&tmp.0);
    assert!(d.is_empty(), "{d:?}");
}
