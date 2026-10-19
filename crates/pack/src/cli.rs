//! `fandhe-pack` のサブコマンドの枠と終了コード。
//!
//! `compile <pack-dir>` と `compare <pack-dir-a> <pack-dir-b>` を実装している（TASK-39）。他のコマンドの中身は後続のタスク（TASK-401〜TASK-408）で作る。
//! `compile` の出力先（`-o`）と末尾の扱いは TASK-407 で固定する。診断と最終行は標準出力、使い方と使い方の誤りは標準エラー出力に出す。
//! 外部仕様（コマンド一覧・引数・usage の文言・終了コード・診断 1 行・最終行・出し先）の正は `docs/cli.md`（版 2）で、
//! コード側の定義は `cli_contract` にある。ここではそれを再公開して、振り分けと出力を担う。
//! `main.rs` から呼ばれる。ディレクトリを読むのは `source::load`（`compile` 経由）だけである。

use std::ffi::OsString;
use std::io::{self, Write};

use crate::cli_contract::{compare_differ_line, compare_same_line, compare_unreadable_line};
use crate::compare::{compare_dirs, Outcome};
use crate::compile::compile_dir;
use crate::diag::write_diagnostics;

pub use crate::cli_contract::{usage, Subcommand, EXIT_DIAGNOSTICS, EXIT_OK, EXIT_USAGE};

/// 引数（実行ファイル名を除く）を解釈して実行し、終了コードを返す。
///
/// `compile` は `<pack-dir>` を 1 つだけ受け、IR（JSON）を `out` に書く。診断があれば `out` に 1 件 1 行で
/// 書いて `EXIT_DIAGNOSTICS` を返す（診断は標準出力。`docs/cli.md` 版 2 の 9 節）。`-o` や余分な引数は
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
/// 一致は `cli_contract::compare_same_line` の行で `EXIT_OK`、不一致は `compare_differ_line` の行で
/// `EXIT_DIAGNOSTICS`。どちらかに診断があれば a の診断、続けて b の診断、最後に `compare_unreadable_line` の
/// 行を書いて `EXIT_DIAGNOSTICS` を返し、一致・不一致の行は出さない（失敗を一致とみなさない）。
/// 不一致の行の「バイト目」は 1 始まり（内部のバイト位置に 1 を足す）。
/// 行の形の正は `docs/cli.md` 版 2 の 8 節である。
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
            writeln!(out, "{}", compare_same_line(len))?;
            Ok(EXIT_OK)
        }
        Ok(Outcome::Differ { offset, line }) => {
            writeln!(out, "{}", compare_differ_line(line, offset))?;
            Ok(EXIT_DIAGNOSTICS)
        }
        Err(e) => {
            write_diagnostics(out, &e.a)?;
            write_diagnostics(out, &e.b)?;
            writeln!(out, "{}", compare_unreadable_line(e.a.len(), e.b.len()))?;
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
