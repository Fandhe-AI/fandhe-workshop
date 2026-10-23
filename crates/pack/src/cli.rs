//! `fandhe-pack` のサブコマンドの引数の解釈と振り分け、終了コード。
//!
//! 引数の形の検査とコマンドへの振り分けは本モジュールが担う（TASK-401）。`compile <pack-dir>` と
//! `compare <pack-dir-a> <pack-dir-b>` は本体まで実装している（TASK-39）。`metrics <pack-dir>` は本体まで実装している（TASK-402）。
//! `compile -o` の書き出しは TASK-407 で実装した。`vocab` は TASK-408 で実装した。`validate` は TASK-403 で実装した。`test` は形を受け付けたうえで
//! 経過措置の未実装（終了コード 2）に振り分け、本体は後続のタスク（TASK-404）が `run_test` の中身を差し替えて作る。
//! 診断と最終行は標準出力、使い方と使い方の誤りは標準エラー出力に出す。
//! 外部仕様（コマンド一覧・引数・usage の文言・終了コード・診断 1 行・最終行・出し先）の正は `docs/cli.md`（版 7）で、
//! コード側の定義は `cli_contract` にある。ここではそれを再公開して、振り分けと出力を担う。
//! `main.rs` から呼ばれる。ディレクトリを読むのは `source::load`（`compile`・`compare` 経由）と `metrics` の走査だけで、引数の解釈は
//! ファイルシステムに触れない。

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::chapter_range::ChapterSelectError;
use crate::cli_contract::{
    compare_differ_line, compare_same_line, compare_unreadable_line, final_line, FinalLine,
    PackSummary,
};
use crate::compare::{compare_dirs, Outcome};
use crate::compile::{compile_dir_with, CompileError, CompileOptions};
use crate::diag::write_diagnostics;
use crate::source;
use crate::validate;
use crate::vocab_report::report_dir;

pub use crate::cli_contract::{usage, Subcommand, EXIT_DIAGNOSTICS, EXIT_OK, EXIT_USAGE};

/// 解釈済みの呼び出し。パスは `OsString` 由来のまま持ち、文字列へ変換しない。
#[derive(Debug, PartialEq, Eq)]
enum Invocation {
    Validate {
        pack_dir: PathBuf,
    },
    Test {
        pack_dir: PathBuf,
    },
    Compile {
        pack_dir: PathBuf,
        chapters: Option<Vec<String>>,
        output: Option<PathBuf>,
    },
    Vocab {
        pack_dir: PathBuf,
    },
    Metrics {
        pack_dir: PathBuf,
    },
    Compare {
        a: PathBuf,
        b: PathBuf,
    },
}

/// 使い方の誤り。`error:` 行の文言を `Display` で持つ。利用者の値は `{:?}` で引用して 1 行に保つ。
#[derive(Debug, PartialEq, Eq)]
enum UsageError {
    /// 必須の位置引数が足りない。
    MissingArgs(Subcommand),
    /// 余分な位置引数（`-` 始まりでないもの）。2 つ目の `-o` は `UnknownOption` になる。
    UnexpectedArg(Subcommand, OsString),
    /// `-` で始まる未知の引数（`-o` の位置違いを含む）。
    UnknownOption(Subcommand, OsString),
    /// `-o` の後ろに値が無い、または値が `-` で始まる。
    MissingOptionValue(Subcommand),
    /// `--chapters` の値が空・空の要素・重複を含む、または `--chapters`・`-o` が 2 回目。
    BadChapters(Subcommand),
}

impl fmt::Display for UsageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingArgs(c) => write!(f, "fandhe-pack {} requires {}", c.name(), c.args()),
            Self::UnexpectedArg(c, a) => write!(
                f,
                "fandhe-pack {} takes {}; unexpected argument: {a:?}",
                c.name(),
                c.args()
            ),
            Self::UnknownOption(c, a) => {
                write!(f, "fandhe-pack {}: unknown option: {a:?}", c.name())
            }
            Self::MissingOptionValue(c) => {
                write!(f, "fandhe-pack {}: -o requires <file>", c.name())
            }
            Self::BadChapters(c) => write!(
                f,
                "fandhe-pack {}: --chapters requires <id>[,<id>...] (once; no empty or duplicate ids)",
                c.name()
            ),
        }
    }
}

fn is_dashed(a: &OsStr) -> bool {
    a.as_encoded_bytes().first() == Some(&b'-')
}

/// 位置引数を 1 つ取る。`-` で始まる引数は位置引数にせず、未知のオプションとして拒否する。
fn positional<I: Iterator<Item = OsString>>(
    cmd: Subcommand,
    args: &mut I,
) -> Result<PathBuf, UsageError> {
    let a = args.next().ok_or(UsageError::MissingArgs(cmd))?;
    if is_dashed(&a) {
        return Err(UsageError::UnknownOption(cmd, a));
    }
    Ok(PathBuf::from(a))
}

/// 残りの引数が無いことを確かめる。
fn expect_end<I: Iterator<Item = OsString>>(
    cmd: Subcommand,
    args: &mut I,
) -> Result<(), UsageError> {
    match args.next() {
        None => Ok(()),
        Some(a) if is_dashed(&a) => Err(UsageError::UnknownOption(cmd, a)),
        Some(a) => Err(UsageError::UnexpectedArg(cmd, a)),
    }
}

/// `compile` の `<pack-dir>` の後ろ。`--chapters <ids>` と `-o <file>` を、順不同で各高々 1 つ受ける。
fn compile_options<I: Iterator<Item = OsString>>(
    args: &mut I,
) -> Result<(Option<Vec<String>>, Option<PathBuf>), UsageError> {
    let cmd = Subcommand::Compile;
    let mut chapters: Option<Vec<String>> = None;
    let mut output: Option<PathBuf> = None;
    while let Some(opt) = args.next() {
        if opt == "-o" {
            if output.is_some() {
                return Err(UsageError::UnknownOption(cmd, opt));
            }
            let value = args
                .next()
                .filter(|v| !is_dashed(v))
                .ok_or(UsageError::MissingOptionValue(cmd))?;
            output = Some(PathBuf::from(value));
        } else if opt == "--chapters" {
            if chapters.is_some() {
                return Err(UsageError::BadChapters(cmd));
            }
            let value = args
                .next()
                .filter(|v| !is_dashed(v))
                .ok_or(UsageError::BadChapters(cmd))?;
            let text = value.to_str().ok_or(UsageError::BadChapters(cmd))?;
            let ids: Vec<String> = text.split(',').map(str::to_string).collect();
            let unique: std::collections::BTreeSet<&String> = ids.iter().collect();
            if ids.iter().any(String::is_empty) || unique.len() != ids.len() {
                return Err(UsageError::BadChapters(cmd));
            }
            chapters = Some(ids);
        } else if is_dashed(&opt) {
            return Err(UsageError::UnknownOption(cmd, opt));
        } else {
            return Err(UsageError::UnexpectedArg(cmd, opt));
        }
    }
    Ok((chapters, output))
}

/// コマンド名の後ろの引数を `docs/cli.md` 3 節の形に従って解釈する。ファイルシステムには触れない。
fn parse<I: Iterator<Item = OsString>>(
    cmd: Subcommand,
    mut args: I,
) -> Result<Invocation, UsageError> {
    let args = &mut args;
    let first = positional(cmd, args)?;
    let inv = match cmd {
        Subcommand::Compare => Invocation::Compare {
            a: first,
            b: positional(cmd, args)?,
        },
        Subcommand::Compile => {
            let (chapters, output) = compile_options(args)?;
            return Ok(Invocation::Compile {
                pack_dir: first,
                chapters,
                output,
            });
        }
        Subcommand::Validate => Invocation::Validate { pack_dir: first },
        Subcommand::Test => Invocation::Test { pack_dir: first },
        Subcommand::Vocab => Invocation::Vocab { pack_dir: first },
        Subcommand::Metrics => Invocation::Metrics { pack_dir: first },
    };
    expect_end(cmd, args)?;
    Ok(inv)
}

/// 引数（実行ファイル名を除く）を解釈して実行し、終了コードを返す。
///
/// 引数なしは使い方だけを標準エラーに出す。コマンド名が未知（`defects` を含む）または引数の形が誤りなら
/// `error:` 行と使い方を標準エラーに出す。いずれも `EXIT_USAGE` で、標準出力には何も書かない。
/// 形が正しければ 6 つのコマンドの `run_*` へ振り分ける。`compile` は IR（JSON）を `out` に書き、診断があれば
/// `out` に 1 件 1 行で書いて `EXIT_DIAGNOSTICS` を返す（診断は標準出力。`docs/cli.md` 版 4 の 9 節）。
/// `-o <file>` があれば IR は標準出力ではなくファイルへ書く（10 節）。`validate` は診断の行と最終行を書く（TASK-403）。`test` は未実装で `EXIT_USAGE` を返す（経過措置）。
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
    let Some(cmd) = first.to_str().and_then(Subcommand::from_name) else {
        writeln!(err, "error: unknown command: {first:?}")?;
        write!(err, "{}", usage())?;
        return Ok(EXIT_USAGE);
    };
    let inv = match parse(cmd, args) {
        Ok(inv) => inv,
        Err(e) => {
            writeln!(err, "error: {e}")?;
            write!(err, "{}", usage())?;
            return Ok(EXIT_USAGE);
        }
    };
    match inv {
        Invocation::Validate { pack_dir } => run_validate(&pack_dir, out),
        Invocation::Test { pack_dir } => run_test(&pack_dir, err),
        Invocation::Compile {
            pack_dir,
            chapters,
            output,
        } => run_compile(&pack_dir, chapters, output.as_deref(), out, err),
        Invocation::Vocab { pack_dir } => run_vocab(&pack_dir, out),
        Invocation::Metrics { pack_dir } => run_metrics(&pack_dir, out, err),
        Invocation::Compare { a, b } => run_compare(&a, &b, out),
    }
}

/// 受け付けた形の呼び出しが、本体の未実装のため終わることを伝える（経過措置。終了コードは 2）。
fn not_implemented<E: Write>(cmd: Subcommand, err: &mut E) -> io::Result<u8> {
    writeln!(
        err,
        "error: fandhe-pack {} is not implemented yet",
        cmd.name()
    )?;
    Ok(EXIT_USAGE)
}

/// `validate` の実行（AUTHOR-2・AUTHOR-3）。診断を 1 件 1 行で標準出力に書き、続けて最終行を 1 行書く。
/// 終了コードは診断 0 件で `EXIT_OK`、1 件以上で `EXIT_DIAGNOSTICS`。読み込み（`source::load`）に失敗したときは
/// 参照などの検査を流さず、読み込みの診断だけを書いて `読み込めません` で終える（fail-closed。診断が空でも成功にしない）。
/// 標準エラー出力には何も書かない。出力の形の正は `docs/cli.md` の 6・8・9 節。
fn run_validate<O: Write>(dir: &Path, out: &mut O) -> io::Result<u8> {
    match source::load(dir) {
        Err(diags) => {
            write_diagnostics(out, &diags)?;
            let line = final_line(&FinalLine::Unreadable {
                diagnostics: diags.len(),
            });
            writeln!(out, "{line}")?;
            Ok(EXIT_DIAGNOSTICS)
        }
        Ok(pack) => {
            let diags = validate::validate(&pack);
            let code = write_diagnostics(out, &diags)?;
            let line = final_line(&FinalLine::Checked {
                diagnostics: diags.len(),
                summary: PackSummary::of(&pack),
            });
            writeln!(out, "{line}")?;
            Ok(code)
        }
    }
}

/// `test` の振り分け先。本体は TASK-404。
fn run_test<E: Write>(_pack_dir: &Path, err: &mut E) -> io::Result<u8> {
    not_implemented(Subcommand::Test, err)
}

/// `vocab` の実行。表と集計を標準出力に書いて `EXIT_OK`。読み込みか `compile` に診断があれば、表は出さず診断を
/// 1 件 1 行で標準出力に書いて `EXIT_DIAGNOSTICS` を返す（失敗を成功とみなさない。標準エラー出力には書かない）。
/// 出力の形の正は `docs/cli.md` 版 6 の 12 節で、数え方は `vocab_report`。
fn run_vocab<O: Write>(dir: &Path, out: &mut O) -> io::Result<u8> {
    match report_dir(dir) {
        Ok(text) => {
            out.write_all(text.as_bytes())?;
            Ok(EXIT_OK)
        }
        Err(diags) => write_diagnostics(out, &diags),
    }
}

/// `metrics` の実行。表も診断も標準出力に出し、標準エラー出力は使わない（診断があれば表は出さない）。
fn run_metrics<O: Write, E: Write>(pack_dir: &Path, out: &mut O, _err: &mut E) -> io::Result<u8> {
    match crate::metrics::report(pack_dir) {
        Ok(table) => {
            out.write_all(table.as_bytes())?;
            Ok(EXIT_OK)
        }
        Err(diags) => write_diagnostics(out, &diags),
    }
}

/// `compile` の実行。IR は `-o` が無ければ標準出力へ、あれば `output` のファイルへ、同じバイト列で出す。
///
/// 診断があるときはファイルを作らず既存のファイルにも触れない（失敗した compile の出力を、後で配信物の IR として
/// 取り込む事故を防ぐ fail-closed）。書き出せないときは理由を `error:` 行で標準エラーに出して `EXIT_USAGE` を
/// 返す（`docs/cli.md` 版 4 の 10 節。AUTHOR-8）。書き出しの I/O エラーは `main` へ伝播させず、ここで理由つきの
/// 終了に変える（伝播すると理由なしの 2 になるため）。
fn run_compile<O, E>(
    dir: &Path,
    chapters: Option<Vec<String>>,
    output: Option<&Path>,
    out: &mut O,
    err: &mut E,
) -> io::Result<u8>
where
    O: Write,
    E: Write,
{
    match compile_dir_with(dir, &CompileOptions { chapters }) {
        Ok(bytes) => match output {
            None => {
                out.write_all(&bytes)?;
                Ok(EXIT_OK)
            }
            Some(path) => match write_output(path, &bytes) {
                Ok(()) => Ok(EXIT_OK),
                Err(e) => {
                    writeln!(
                        err,
                        "error: fandhe-pack compile: cannot write output {path:?}: {}",
                        e.kind()
                    )?;
                    Ok(EXIT_USAGE)
                }
            },
        },
        Err(CompileError::Diagnostics(diags)) => write_diagnostics(out, &diags),
        // 章の選択の誤りは使い方の誤り。IR もファイルも出さず、教材の本文は写さない。
        Err(CompileError::Chapter(e)) => {
            let (kind, id) = match e {
                ChapterSelectError::Unknown(id) => ("unknown", id),
                ChapterSelectError::Ambiguous(id) => ("ambiguous", id),
            };
            writeln!(err, "error: fandhe-pack compile: {kind} chapter: {id:?}")?;
            Ok(EXIT_USAGE)
        }
    }
}

/// `bytes` を `path` へ原子的に置き換えて書く（同じディレクトリの一時ファイルへ書き、`sync_all` してから `rename`）。
///
/// 途中の失敗で途中まで書いた IR が残らず、既存のファイルより短い IR でも末尾が残らない。一時ファイルは
/// `create_new` で作り、既存のファイルやリンクを辿って書き換えない。作れなかった一時パスは消さず、作った後の失敗でだけ消す。
/// 置き換えるので、既存のファイルの権限は引き継がない。
fn write_output(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "output path has no file name")
    })?;
    if fs::metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(io::Error::new(
            io::ErrorKind::IsADirectory,
            "output path is a directory",
        ));
    }
    let mut tmp_name = OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".fandhe-pack-{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    // open が失敗した経路（既存のファイルとの衝突を含む）では、自分が作っていない一時パスを消さない。
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)?;
    let tmp_ref = &tmp;
    let written = (move || {
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(tmp_ref, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// `compare` の実行。引数は `<pack-dir-a> <pack-dir-b>` ちょうど 2 つ（解釈済み）。
///
/// 一致は `cli_contract::compare_same_line` の行で `EXIT_OK`、不一致は `compare_differ_line` の行で
/// `EXIT_DIAGNOSTICS`。どちらかに診断があれば a の診断、続けて b の診断、最後に `compare_unreadable_line` の
/// 行を書いて `EXIT_DIAGNOSTICS` を返し、一致・不一致の行は出さない（失敗を一致とみなさない）。
/// 不一致の行の「バイト目」は 1 始まり（内部のバイト位置に 1 を足す）。
/// 行の形の正は `docs/cli.md` 版 4 の 8 節である。
fn run_compare<O: Write>(a: &Path, b: &Path, out: &mut O) -> io::Result<u8> {
    match compare_dirs(a, b) {
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

    fn parse_args(cmd: Subcommand, args: &[&str]) -> Result<Invocation, UsageError> {
        parse(cmd, args.iter().map(OsString::from))
    }

    #[test]
    fn c1_accepted_shapes_reach_their_handler() {
        for c in Subcommand::ALL {
            if !matches!(c, Subcommand::Test) {
                continue;
            }
            let (code, out, err) = run_args(&[c.name(), "x"]);
            assert_eq!(code, EXIT_USAGE);
            assert!(out.is_empty());
            assert!(err.contains("is not implemented yet"));
            assert!(err.contains(c.name()));
        }
    }

    #[test]
    fn c1_validate_missing_dir_is_unreadable() {
        let (code, out, err) = run_args(&["validate", "/nonexistent/fandhe-validate-unit"]);
        assert_eq!(code, EXIT_DIAGNOSTICS);
        assert!(err.is_empty());
        assert_eq!(out.lines().last(), Some("NG: 診断 1 件（読み込めません）"));
    }

    #[test]
    fn c6_compile_argument_errors() {
        let (code, out, err) = run_args(&["compile"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.starts_with("error:") && err.contains("usage: fandhe-pack"));
        assert!(!err.contains("not implemented"));
        let (code, out, err) = run_args(&["compile", "-o", "y", "x"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.contains("unknown option"));
        assert!(!err.contains("not implemented"));
        // `-o` の値が無い・`-` 始まりの値は形の誤り（書き出しには進まない）。
        for args in [&["compile", "x", "-o"][..], &["compile", "x", "-o", "-y"]] {
            let (code, out, err) = run_args(args);
            assert_eq!(code, EXIT_USAGE, "{args:?}");
            assert!(out.is_empty() && err.starts_with("error:"), "{args:?}");
            assert!(!err.contains("not implemented"), "{args:?}");
        }
    }

    #[test]
    fn c8_compare_argument_errors() {
        for args in [&["compare"][..], &["compare", "x"]] {
            let (code, out, err) = run_args(args);
            assert_eq!(code, EXIT_USAGE);
            assert!(out.is_empty() && err.starts_with("error:") && err.contains("requires"));
        }
        let (code, out, err) = run_args(&["compare", "x", "y", "z"]);
        assert_eq!(code, EXIT_USAGE);
        assert!(out.is_empty() && err.contains("unexpected argument"));
    }

    #[test]
    fn c9_parse_table() {
        use Subcommand::*;
        let pd = |s: &str| PathBuf::from(s);
        assert_eq!(
            parse_args(Validate, &["d"]),
            Ok(Invocation::Validate { pack_dir: pd("d") })
        );
        assert_eq!(
            parse_args(Compile, &["d", "-o", "f"]),
            Ok(Invocation::Compile {
                pack_dir: pd("d"),
                chapters: None,
                output: Some(pd("f"))
            })
        );
        assert_eq!(
            parse_args(Compile, &["d", "-o", "f", "--chapters", "a,b"]),
            Ok(Invocation::Compile {
                pack_dir: pd("d"),
                chapters: Some(vec!["a".to_string(), "b".to_string()]),
                output: Some(pd("f"))
            })
        );
        assert_eq!(
            parse_args(Compare, &["a", "b"]),
            Ok(Invocation::Compare {
                a: pd("a"),
                b: pd("b")
            })
        );
        assert_eq!(parse_args(Test, &[]), Err(UsageError::MissingArgs(Test)));
        assert!(matches!(
            parse_args(Vocab, &["a", "b"]),
            Err(UsageError::UnexpectedArg(Vocab, _))
        ));
        for args in [&["-x"][..], &["-o", "f"], &["--"], &["-o=f"]] {
            assert!(
                matches!(
                    parse_args(Metrics, args),
                    Err(UsageError::UnknownOption(..))
                ),
                "{args:?}"
            );
        }
        assert!(matches!(
            parse_args(Compile, &["-o", "f", "d"]),
            Err(UsageError::UnknownOption(..))
        ));
        assert!(matches!(
            parse_args(Compile, &["d", "-x"]),
            Err(UsageError::UnknownOption(..))
        ));
        assert_eq!(
            parse_args(Compile, &["d", "-o"]),
            Err(UsageError::MissingOptionValue(Compile))
        );
        assert_eq!(
            parse_args(Compile, &["d", "-o", "-x"]),
            Err(UsageError::MissingOptionValue(Compile))
        );
        assert!(matches!(
            parse_args(Compile, &["d", "-o", "f", "-o", "g"]),
            Err(UsageError::UnknownOption(..))
        ));
        assert!(matches!(
            parse_args(Compile, &["d", "-o", "f", "extra"]),
            Err(UsageError::UnexpectedArg(..))
        ));
        assert!(matches!(
            parse_args(Compare, &["-x", "a"]),
            Err(UsageError::UnknownOption(..))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn c10_non_utf8_pack_dir_is_kept() {
        use std::os::unix::ffi::OsStrExt;
        let raw = OsStr::from_bytes(b"d\xff");
        let inv = parse(Subcommand::Validate, std::iter::once(raw.to_os_string()));
        assert_eq!(
            inv,
            Ok(Invocation::Validate {
                pack_dir: PathBuf::from(raw)
            })
        );
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
