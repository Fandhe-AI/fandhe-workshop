//! `fandhe-pack` の実行ファイルを起動する結合試験。

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn i1_subcommands_start() {
    for name in ["validate", "test", "vocab"] {
        let o = bin().args([name, "/nonexistent/path"]).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{name}");
        let err = String::from_utf8_lossy(&o.stderr);
        assert!(err.contains("not implemented"), "{name}: {err}");
    }
}

#[test]
fn i2_no_args_prints_usage() {
    let o = bin().output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&o.stderr).starts_with("usage: fandhe-pack"));
}

#[test]
fn i3_unknown_subcommand() {
    let o = bin().arg("bogus").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
}

fn ch1_pack() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/input-field-ime/test-data/ch1-pack")
}

#[test]
fn i4_compile_prints_ir() {
    let o = bin().arg("compile").arg(ch1_pack()).output().unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    assert!(o.stdout.ends_with(b"}\n") && !o.stdout.ends_with(b"\n\n"));
    let v: serde_json::Value = serde_json::from_slice(&o.stdout).unwrap();
    // 本文を持つパックは、最小の版が 2 になる（TASK-60）。
    assert_eq!(v["format_version"], 2);
}

#[test]
fn i5_compile_missing_dir_is_diagnostic() {
    let o = bin()
        .args(["compile", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).contains("E-PARSE"));
}

#[test]
fn i6_compile_argument_errors() {
    let o = bin().arg("compile").output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = bin()
        .arg("compile")
        .arg(ch1_pack())
        .args(["-o", "x"])
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
}

fn min_3step_a() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step-pack")
}

fn min_3step_b() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

#[test]
fn i7_compare_same_content_is_ok() {
    let o = bin()
        .arg("compare")
        .arg(min_3step_a())
        .arg(min_3step_b())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(0));
    assert!(o.stderr.is_empty());
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("OK: IR matches ("), "{out}");
}

#[test]
fn i8_compare_different_packs_is_ng() {
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg(min_3step_a())
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.starts_with("NG: IR differs at line "), "{out}");
}

#[test]
fn i9_compare_missing_dir_is_diagnostic_not_verdict() {
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg("/nonexistent/path")
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8_lossy(&o.stdout);
    assert!(out.contains("E-PARSE"), "{out}");
    assert!(!out.contains("OK:") && !out.contains("NG:"), "{out}");
}

#[test]
fn i10_compare_argument_errors() {
    let a = ch1_pack();
    assert_eq!(
        bin().arg("compare").output().unwrap().status.code(),
        Some(2)
    );
    let o = bin().arg("compare").arg(&a).output().unwrap();
    assert_eq!(o.status.code(), Some(2));
    let o = bin()
        .arg("compare")
        .arg(&a)
        .arg(&a)
        .arg(&a)
        .output()
        .unwrap();
    assert_eq!(o.status.code(), Some(2));
    assert!(o.stdout.is_empty());
}

/// PACK-2: 両方ある・どちらも無いは終了コード 1 で `E-PARSE` を出す。
#[test]
fn pack2_cli_exit_code_for_both_and_neither() {
    let base = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("pack2-cli-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let both = base.join("both");
    let none = base.join("none");
    std::fs::create_dir_all(&both).unwrap();
    std::fs::create_dir_all(&none).unwrap();
    std::fs::write(both.join("workshop.yaml"), "pack: {}\n").unwrap();
    std::fs::write(both.join("workshop.toml"), "[pack]\n").unwrap();
    for dir in [&both, &none] {
        let o = bin().arg("compile").arg(dir).output().unwrap();
        assert_eq!(o.status.code(), Some(1), "{}", dir.display());
        assert!(String::from_utf8_lossy(&o.stdout).contains("E-PARSE"));
    }
    let _ = std::fs::remove_dir_all(&base);
}
