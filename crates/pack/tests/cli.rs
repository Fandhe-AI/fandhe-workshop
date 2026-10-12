//! `fandhe-pack` の実行ファイルを起動する結合試験。

use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

#[test]
fn i1_subcommands_start() {
    for name in ["validate", "test", "compile", "compare", "vocab"] {
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
