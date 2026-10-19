//! CLI の外部仕様の契約の適合試験（TASK-399 1/2、AUTHOR-1・AUTHOR-2）。
//!
//! 正は `docs/cli.md`（版 1）で、本ファイルは実行ファイルが契約の終了コードと出し先で終わることと、
//! 定数・型・使い方の文言を確かめる。文書との一致は `cli_contract_doc.rs` が見る（変異注入は
//! 文書を一時ディレクトリへ写さないため、文書を読む試験を分けている）。
//! 診断 1 行・最終行の形は本版の対象外（TASK-399 2/2）で、`compile -o` の動作は TASK-407 の担当のため見ない。

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use fandhe_workshop_pack::cli_contract::{
    usage, Subcommand, BIN_NAME, EXIT_DIAGNOSTICS, EXIT_OK, EXIT_USAGE,
};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

fn code(o: &Output) -> Option<i32> {
    o.status.code()
}

fn ch1_pack() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../testdata/input-field-ime/test-data/ch1-pack")
}

fn min_3step_a() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step-pack")
}

fn min_3step_b() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/min-3step-pack-b")
}

#[test]
fn exit_codes_are_fixed_values() {
    assert_eq!(EXIT_OK, 0);
    assert_eq!(EXIT_DIAGNOSTICS, 1);
    assert_eq!(EXIT_USAGE, 2);
    assert_eq!(BIN_NAME, "fandhe-pack");
}

#[test]
fn commands_are_the_six_in_contract_order() {
    let names: Vec<&str> = Subcommand::ALL.iter().map(|c| c.name()).collect();
    assert_eq!(
        names,
        ["validate", "test", "compile", "vocab", "metrics", "compare"]
    );
}

#[test]
fn defects_is_not_an_author_command() {
    assert_eq!(Subcommand::from_name("defects"), None);
    assert!(!usage().contains("defects"));
    let o = bin().args(["defects", "x"]).output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains(&usage()));
}

#[test]
fn author1_no_args_prints_usage_and_exits_2() {
    let o = bin().output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert_eq!(String::from_utf8_lossy(&o.stderr), usage());
}

#[test]
fn unknown_command_exits_2_with_usage() {
    let o = bin().arg("bogus").output().unwrap();
    assert_eq!(code(&o), Some(2));
    assert!(o.stdout.is_empty());
    assert!(String::from_utf8_lossy(&o.stderr).contains(&usage()));
}

#[test]
fn argument_shape_errors_exit_2() {
    let a = ch1_pack();
    let cases: Vec<Vec<&std::ffi::OsStr>> = vec![
        vec!["compile".as_ref()],
        vec!["compare".as_ref()],
        vec!["compare".as_ref(), a.as_os_str()],
        vec![
            "compare".as_ref(),
            a.as_os_str(),
            a.as_os_str(),
            a.as_os_str(),
        ],
    ];
    for args in cases {
        let o = bin().args(&args).output().unwrap();
        assert_eq!(code(&o), Some(2), "{args:?}");
        assert!(o.stdout.is_empty(), "{args:?}");
    }
    // 引数の数の検査は TASK-401 の担当。骨格の間は未実装として 2 で終わることだけを見る（経過措置）。
    for c in ["validate", "test", "vocab", "metrics"] {
        let o = bin().args([c, "/nonexistent/path"]).output().unwrap();
        assert_eq!(code(&o), Some(2), "{c}");
        assert!(o.stdout.is_empty(), "{c}");
    }
}

#[test]
fn exit_0_and_1_follow_diagnostics() {
    let o = bin().arg("compile").arg(ch1_pack()).output().unwrap();
    assert_eq!(code(&o), Some(0));
    let o = bin()
        .args(["compile", "/nonexistent/path"])
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(1));
    let o = bin()
        .arg("compare")
        .arg(min_3step_a())
        .arg(min_3step_b())
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(0));
    let o = bin()
        .arg("compare")
        .arg(ch1_pack())
        .arg(min_3step_a())
        .output()
        .unwrap();
    assert_eq!(code(&o), Some(1));
}

#[test]
fn usage_declares_compile_output_option() {
    // `-o` の動作は TASK-407 の担当。ここでは使い方に宣言されていることだけを見る。
    assert!(usage().contains("  compile <pack-dir> [-o <file>]\n"));
    assert!(usage().starts_with("usage: fandhe-pack <command> [<args>]\n\ncommands:\n"));
}
