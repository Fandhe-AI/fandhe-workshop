//! YAML を読む crate の採用版の固定を守る試験（TASK-73 / PACK-47 / D-58）。
//!
//! 形式 B（YAML）の読み込みは `yaml_serde` に依存する。D-58 は「維持して版を固定する」と決めた。
//! `Cargo.toml` の宣言が範囲に緩むと、`cargo update` で黙って別の版に動く。ここでは宣言・`Cargo.lock` の
//! 解決済みの版・pack crate の参照の仕方を照合する。crates.io の現況の確認は依存の棚卸し（TASK-99）が持つ。
//!
//! 読み込みは `CARGO_MANIFEST_DIR` からの相対パスで行う。変異注入（`scripts/check-mutations.sh`）の
//! 一時コピーでも、コピー側のファイルが読まれる。

use toml::Table;

const ROOT_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.toml"));
const PACK_MANIFEST: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
const LOCK: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../Cargo.lock"));

/// PACK-47 の採用版（D-58）。版を上げるときは、この定数と `docs/testing.md` の 5.126 を同時に更新する。
const ADOPTED_YAML_SERDE: &str = "0.10.7";

fn parse(src: &str) -> Table {
    src.parse::<Table>().expect("TOML として読めること")
}

#[test]
fn pack47_workspace_declares_adopted_yaml_serde_version() {
    let root = parse(ROOT_MANIFEST);
    let dep = &root["workspace"]["dependencies"]["yaml_serde"];
    let declared = dep
        .as_str()
        .or_else(|| dep.get("version").and_then(|v| v.as_str()))
        .unwrap_or_default();
    assert_eq!(
        declared, ADOPTED_YAML_SERDE,
        "PACK-47: yaml_serde は {ADOPTED_YAML_SERDE} に固定する（D-58）: {dep}"
    );
}

#[test]
fn pack47_lock_resolves_adopted_yaml_serde_version() {
    let lock = parse(LOCK);
    let packages = lock["package"].as_array().expect("package の配列");
    let found: Vec<_> = packages
        .iter()
        .filter(|p| p.get("name").and_then(|n| n.as_str()) == Some("yaml_serde"))
        .collect();
    assert_eq!(
        found.len(),
        1,
        "PACK-47: yaml_serde は 1 版だけ解決される: {found:?}"
    );
    let pkg = found[0];
    assert_eq!(
        pkg.get("version").and_then(|v| v.as_str()),
        Some(ADOPTED_YAML_SERDE),
        "PACK-47: Cargo.lock の yaml_serde の版が採用版と違う: {pkg}"
    );
    let source = pkg
        .get("source")
        .and_then(|s| s.as_str())
        .unwrap_or_default();
    assert!(
        source.starts_with("registry+https://github.com/rust-lang/crates.io-index"),
        "PACK-47: yaml_serde は crates.io の registry から取る: {source}"
    );
}

#[test]
fn pack47_pack_uses_workspace_yaml_serde() {
    let pack = parse(PACK_MANIFEST);
    let dep = &pack["dependencies"]["yaml_serde"];
    assert_eq!(
        dep.get("workspace").and_then(|v| v.as_bool()),
        Some(true),
        "PACK-47: pack は版を上書きせず workspace の宣言を使う: {dep}"
    );
    assert_eq!(
        dep.as_table().map(|t| t.len()),
        Some(1),
        "PACK-47: workspace 以外の指定がある: {dep}"
    );
}
