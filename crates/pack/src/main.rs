//! `fandhe-pack` の入口。
//!
//! 現時点は骨格のみで、何も読まずに未実装の旨を返して終了する。実装は後続のタスクで追加する。
#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!("error: fandhe-pack is not implemented yet");
    ExitCode::from(2)
}
