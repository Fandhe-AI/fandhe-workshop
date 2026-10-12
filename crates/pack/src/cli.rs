//! `fandhe-pack` のサブコマンドの枠と終了コード。
//!
//! 枠だけを置く。各コマンドの中身は後続のタスク（TASK-39・TASK-401〜TASK-408）で作る。
//! 外部仕様（コマンド一覧・引数・usage の文言・終了コード）の正は TASK-399（#470）で、固定されたら直す。
//! `main.rs` から呼ばれ、ファイルシステムには触れない。

use std::ffi::OsString;
use std::io::{self, Write};

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
/// 骨格では全コマンドが未実装で `EXIT_USAGE` を返す。`out` は後続のタスクが診断と最終行を出す場所で、
/// 今は何も書かない。2 つ目以降の引数は読まない。
pub fn run<I, O, E>(args: I, _out: &mut O, err: &mut E) -> io::Result<u8>
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
    fn c1_all_subcommands_not_implemented() {
        for c in Subcommand::ALL {
            let (code, out, err) = run_args(&[c.name(), "x"]);
            assert_eq!(code, EXIT_USAGE);
            assert!(out.is_empty());
            assert!(err.contains("not implemented"));
            assert!(err.contains(c.name()));
        }
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
