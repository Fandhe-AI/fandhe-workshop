//! `fandhe-pack` の実行ファイルを起動する結合試験。

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn i1_subcommands_start() {
    for name in ["validate", "test", "compare", "vocab"] {
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
    assert_eq!(v["format_version"], 1);
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
