//! `testdata/` に写した最小 3 Step の教材が、repo の中だけで読めることの確認。
//!
//! 役割: 兄弟ディレクトリ（`03-poc/`）が無くても試験データが読める（BUILD-14 の前提）ことを、
//! 自動試験で保つ。読み込み器（pack の実装）はまだ無いため、既存の依存（`toml`・`serde_json`）で
//! 構文の水準だけを確かめる。後続の読み込み器の試験は、同じデータを `testdata/` から読む。
//! データの出所と規則は `testdata/README.md`。データは信頼できない入力として扱い、
//! 失敗時の出力にはパスと項目名だけを出す（本文は出さない）。

use std::fs;
use std::path::{Path, PathBuf};

/// 最小 3 Step の教材（形式 A）の `testdata/` 内の位置。
const PACK_DIR: &str = "engine-event-consistency/test-data/min-3step-pack";
/// 同じ教材を `fandhe-pack compile` した IR の `testdata/` 内の位置。
const IR_FILE: &str = "engine-event-consistency/test-data/min-3step.json";
/// 3 Step の id（出現順）。
const STEP_IDS: [&str; 3] = ["st-1", "st-2", "st-3"];

/// `testdata/` の絶対パスを返す。repo の外へ出ていないことも確かめる。
fn testdata_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .join("../..")
        .canonicalize()
        .map_err(|e| format!("repo root not resolvable: {e}"))?;
    let root = manifest_dir
        .join("../../testdata")
        .canonicalize()
        .map_err(|e| format!("testdata/ not found: {e}"))?;
    if !root.starts_with(&repo_root) {
        return Err("testdata/ resolves outside the repository".to_string());
    }
    Ok(root)
}

/// UTF-8 のテキストファイルを読む。失敗時はパスだけを含むエラーにする。
fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))
}

#[test]
fn workshop_toml_declares_the_min_3step_pack() -> Result<(), String> {
    let path = testdata_root()?.join(PACK_DIR).join("workshop.toml");
    let value: toml::Value =
        toml::from_str(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;
    let pack = value
        .get("pack")
        .ok_or_else(|| format!("{}: [pack] missing", path.display()))?;
    assert_eq!(
        pack.get("id").and_then(toml::Value::as_str),
        Some("min-3step")
    );
    assert_eq!(
        pack.get("ir_format").and_then(toml::Value::as_integer),
        Some(1)
    );
    Ok(())
}

#[test]
fn chapter_toml_has_three_steps_with_readable_bodies() -> Result<(), String> {
    let dir = testdata_root()?.join(PACK_DIR).join("chapters/ch-min");
    let path = dir.join("chapter.toml");
    let value: toml::Value =
        toml::from_str(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;

    let steps = value
        .get("steps")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{}: steps missing", path.display()))?;
    let ids: Vec<&str> = steps
        .iter()
        .filter_map(|s| s.get("id").and_then(toml::Value::as_str))
        .collect();
    assert_eq!(ids, STEP_IDS, "step ids in {}", path.display());

    let starts = value
        .get("starts")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{}: starts missing", path.display()))?;
    assert_eq!(starts.len(), 1, "starts in {}", path.display());

    for step in steps {
        let body = step
            .get("body")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| format!("{}: step body missing", path.display()))?;
        // body はデータ由来の値。同じディレクトリの単一のファイル名だけを許し、パスとして辿らない。
        if body.contains('/') || body.contains('\\') || body.contains("..") {
            return Err(format!("{}: unexpected body path", path.display()));
        }
        read_text(&dir.join(body))?;
    }
    Ok(())
}

#[test]
fn compiled_ir_matches_the_min_3step_pack() -> Result<(), String> {
    let path = testdata_root()?.join(IR_FILE);
    let value: serde_json::Value =
        serde_json::from_str(&read_text(&path)?).map_err(|e| format!("{}: {e}", path.display()))?;

    assert_eq!(
        value
            .get("format_version")
            .and_then(serde_json::Value::as_i64),
        Some(1)
    );
    assert_eq!(
        value
            .pointer("/pack/id")
            .and_then(serde_json::Value::as_str),
        Some("min-3step")
    );

    let nodes = value
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| format!("{}: nodes missing", path.display()))?;
    let ids: Vec<&str> = nodes
        .iter()
        .filter_map(|n| n.pointer("/step/id").and_then(serde_json::Value::as_str))
        .collect();
    assert_eq!(ids, STEP_IDS, "node step ids in {}", path.display());
    assert_eq!(
        value
            .pointer("/starts/0/node")
            .and_then(serde_json::Value::as_str),
        Some("main/st-1")
    );
    Ok(())
}
