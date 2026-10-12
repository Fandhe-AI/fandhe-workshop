//! `fandhe-pack` の入口。
//!
//! lib の `cli::run` に引数と標準入出力を渡し、戻り値を終了コードに変えるだけにする。
#![forbid(unsafe_code)]

use std::io;
use std::process::ExitCode;

use fandhe_workshop_pack::cli;

fn main() -> ExitCode {
    let mut out = io::stdout().lock();
    let mut err = io::stderr().lock();
    match cli::run(std::env::args_os().skip(1), &mut out, &mut err) {
        Ok(code) => ExitCode::from(code),
        // 出力できない以上、結果を伝えられない。成功(0)と取り違えられないよう 2 で終える。
        Err(_) => ExitCode::from(cli::EXIT_USAGE),
    }
}
