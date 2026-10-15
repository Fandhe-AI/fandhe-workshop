//! `fandhe-pack` のサブコマンドの枠と終了コード。
//!
//! `compile <pack-dir>` と `compare <pack-dir-a> <pack-dir-b>` を実装している（TASK-39）。他のコマンドの中身は後続のタスク（TASK-401〜TASK-408）で作る。
//! `compile` の出力先（`-o`）と末尾の扱いは TASK-407、診断の出し先は TASK-399 で固定する。
//! 外部仕様（コマンド一覧・引数・usage の文言・終了コード）の正は TASK-399（#470）で、固定されたら直す。
//! `main.rs` から呼ばれる。ディレクトリを読むのは `source::load`（`compile` 経由）だけである。

use std::ffi::OsString;
use std::io::{self, Write};

use crate::compare::{compare_dirs, Outcome};
use crate::compile::compile_dir;
use crate::diag::write_diagnostics;

/// 成功（診断なし）。TASK-399 で固定する。
pub const EXIT_OK: u8 = 0;
/// 診断が 1 件以上ある。TASK-399 で固定する。
pub const EXIT_DIAGNOSTICS: u8 = 1;
/// 使い方の誤り、または未実装で実行できない。TASK-399 で固定する。
pub const EXIT_USAGE: u8 = 2;

/// 作成者向けのサブコマンド。
///
/// `metrics` は TASK-402（#476）で足す。`defects` は作成者向けではないため置かない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subcommand {
    /// パックを検証する。
    Validate,
    /// パックの通し試験を実行する。
    Test,
    /// パックを配布形式へ変換する。
    Compile,
    /// 2 つのパックを比較する。
    Compare,
    /// 語彙を出力する。
    Vocab,
}

impl Subcommand {
    /// 全サブコマンド。
    pub const ALL: [Subcommand; 5] = [
        Subcommand::Validate,
        Subcommand::Test,
        Subcommand::Compile,
        Subcommand::Compare,
        Subcommand::Vocab,
    ];

    /// コマンド名。
    pub const fn name(self) -> &'static str {
        match self {
            Subcommand::Validate => "validate",
            Subcommand::Test => "test",
            Subcommand::Compile => "compile",
            Subcommand::Compare => "compare",
            Subcommand::Vocab => "vocab",
        }
    }

    /// コマンド名から引く。未知なら `None`。
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// 使い方（英語）。引数の検査は TASK-401 が行う。
pub fn usage() -> String {
    [
        "usage: fandhe-pack <command> [<args>]",
        "",
        "commands:",
        "  validate <pack-dir>",
        "  test <pack-dir>",
        "  compile <pack-dir> [-o <file>]",
        "  compare <pack-dir-a> <pack-dir-b>",
        "  vocab <pack-dir>",
        "",
    ]
    .join("\n")
}

/// 引数（実行ファイル名を除く）を解釈して実行し、終了コードを返す。
///
/// `compile` は `<pack-dir>` を 1 つだけ受け、IR（JSON）を `out` に書く。診断があれば `out` に 1 件 1 行で
/// 書いて `EXIT_DIAGNOSTICS` を返す（診断の出し先は仮で、TASK-399 で固定する）。`-o` や余分な引数は
/// 未実装として `EXIT_USAGE` を返す。他のコマンドは未実装で `EXIT_USAGE` を返し、2 つ目以降の引数は読まない。
pub fn run<I, O, E>(args: I, out: &mut O, err: &mut E) -> io::Result<u8>
where
    I: IntoIterator<Item = OsString>,
    O: Write,
    E: Write,
{
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        write!(err, "{}", usage())?;
        return Ok(EXIT_USAGE);
    };
    match first.to_str().and_then(Subcommand::from_name) {
        Some(Subcommand::Compile) => return run_compile(args, out, err),
        Some(Subcommand::Compare) => return run_compare(args, out, err),
        Some(cmd) => {
            writeln!(
                err,
                "error: fandhe-pack {} is not implemented yet",
                cmd.name()
            )?;
        }
        None => {
            writeln!(err, "error: unknown command: {first:?}")?;
            write!(err, "{}", usage())?;
        }
    }
    Ok(EXIT_USAGE)
}

/// `compile` の実行。引数は `<pack-dir>` ちょうど 1 つ。
fn run_compile<I, O, E>(mut args: I, out: &mut O, err: &mut E) -> io::Result<u8>
where
    I: Iterator<Item = OsString>,
    O: Write,
    E: Write,
{
    let Some(dir) = args.next() else {
        write!(err, "{}", usage())?;
        return Ok(EXIT_USAGE);
    };
    if args.next().is_some() {
        writeln!(
            err,
            "error: fandhe-pack compile accepts only <pack-dir>; -o is not implemented yet"
        )?;
        return Ok(EXIT_USAGE);
    }
    match compile_dir(std::path::Path::new(&dir)) {
        Ok(bytes) => {
            out.write_all(&bytes)?;
            Ok(EXIT_OK)
        }
        Err(diags) => write_diagnostics(out, &diags),
    }
}

/// `compare` の実行。引数は `<pack-dir-a> <pack-dir-b>` ちょうど 2 つ。
///
/// 一致は `OK: IR matches (<len> bytes)` で `EXIT_OK`、不一致は `NG: IR differs at line <line> (byte <offset>)`
/// で `EXIT_DIAGNOSTICS`。どちらかに診断があれば a の診断、続けて b の診断を書いて `EXIT_DIAGNOSTICS` を返し、
/// 判定の行は出さない（失敗を一致とみなさない）。最終行の形と、診断がどちらのパックのものかの表し方は、
/// 仮のもので TASK-399 で固定する。
fn run_compare<I, O, E>(mut args: I, out: &mut O, err: &mut E) -> io::Result<u8>
where
    I: Iterator<Item = OsString>,
    O: Write,
    E: Write,
{
    let (Some(a), Some(b)) = (args.next(), args.next()) else {
        write!(err, "{}", usage())?;
        return Ok(EXIT_USAGE);
    };
    if args.next().is_some() {
        writeln!(
            err,
            "error: fandhe-pack compare accepts exactly <pack-dir-a> <pack-dir-b>"
        )?;
        return Ok(EXIT_USAGE);
    }
    match compare_dirs(std::path::Path::new(&a), std::path::Path::new(&b)) {
        Ok(Outcome::Same { len }) => {
            writeln!(out, "OK: IR matches ({len} bytes)")?;
            Ok(EXIT_OK)
        }
        Ok(Outcome::Differ { offset, line }) => {
            writeln!(out, "NG: IR differs at line {line} (byte {offset})")?;
            Ok(EXIT_DIAGNOSTICS)
        }
        Err(e) => {
            write_diagnostics(out, &e.a)?;
            write_diagnostics(out, &e.b)?;
            Ok(EXIT_DIAGNOSTICS)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_args(args: &[&str]) -> (u8, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let code = run(args.iter().map(OsString::from), &mut out, &mut err).unwrap();
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn c1_unimplemented_subcommands_report_so() {
        for c in Subcommand::ALL {
            if matches!(c, Subcommand::Compile | Subcommand::Compare) {
                continue;
            }
            let (code, out, err) = run_args(&[c.name(), "x"]);
            assert_eq!(code, EXIT_USAGE);
            assert!(out.is_empty());
            assert!(err.contains("not implemented"));
            assert!(err.contains(c.name()));
        }
    }

    #[test]
    fn c6_compile_argument_errors() {
        let (code, out, err) = run_args(&["compile"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.starts_with("usage: fandhe-pack"));
        let (code, out, err) = run_args(&["compile", "x", "-o", "y"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.contains("not implemented yet"));
    }

    #[test]
    fn c8_compare_argument_errors() {
        for args in [&["compare"][..], &["compare", "x"]] {
            let (code, out, err) = run_args(args);
            assert_eq!(code, EXIT_USAGE);
            assert!(out.is_empty() && err.starts_with("usage: fandhe-pack"));
        }
        let (code, out, err) = run_args(&["compare", "x", "y", "z"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.contains("exactly"));
    }

    #[test]
    fn c7_compile_missing_dir_is_diagnostic() {
        let (code, out, err) = run_args(&["compile", "/nonexistent/fandhe-pack-dir"]);
        assert_eq!(code, EXIT_DIAGNOSTICS);
        assert!(out.contains("E-PARSE") && err.is_empty());
    }

    #[test]
    fn c2_no_args_prints_usage() {
        let (code, _, err) = run_args(&[]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.starts_with("usage: fandhe-pack"));
    }

    #[test]
    fn c3_unknown_command() {
        let (code, _, err) = run_args(&["bogus"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(err.contains("unknown command"));
        assert!(err.contains("usage: fandhe-pack"));
    }

    #[test]
    fn c4_usage_lists_commands() {
        let u = usage();
        for c in Subcommand::ALL {
            assert!(u.contains(c.name()));
        }
        assert!(!u.contains("defects"));
    }

    #[test]
    fn c5_name_roundtrip() {
        for c in Subcommand::ALL {
            assert_eq!(Subcommand::from_name(c.name()), Some(c));
        }
    }
}
