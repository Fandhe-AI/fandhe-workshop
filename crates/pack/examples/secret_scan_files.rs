//! 公開前の確認（LIC-12・D-40。TASK-139）のための、認証情報の形の照合の入口。
//!
//! 呼び出し元は `tools/check-internal-info.mjs` だけである。認証情報の形の判定の正は
//! `fandhe_workshop_core::secret_scan`（エンジンの実行時の検出と `validate` が使うものと同じ）であり、
//! 規則を JS へ写して正を 2 つにしないよう、ここで core を呼ぶ。公開 CLI（`fandhe-pack`）の契約は変えないため、
//! サブコマンドではなく example にしている。
//!
//! 入力は標準入力の NUL 区切りのパス（シェルの文字列を組み立てない）。各ファイルを行ごとに照合し、
//! 当たった行を `<path>:<line>: <shape_id>` で標準出力へ出す。値・値の一部は出さない。
//!
//! 終了コード: 0 = 当たりなし / 1 = 当たりあり / 2 = 入力を読めない（検査を完了できない。合格にしない）。
#![forbid(unsafe_code)]

use std::io::{self, Read, Write};
use std::process::ExitCode;

use fandhe_workshop_core::secret_scan::{scan_text, shapes_of};

fn run() -> Result<bool, String> {
    let mut raw = Vec::new();
    io::stdin()
        .read_to_end(&mut raw)
        .map_err(|e| format!("cannot read stdin: {e}"))?;
    let mut out = io::stdout().lock();
    let mut found = false;
    for chunk in raw.split(|b| *b == 0).filter(|c| !c.is_empty()) {
        let path = std::str::from_utf8(chunk).map_err(|_| "path is not UTF-8".to_string())?;
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read file: {e}"))?;
        // 本物の秘密情報は ASCII の連なりなので、UTF-8 でない部分は置換して読んでも見逃さない。
        let text = String::from_utf8_lossy(&bytes);
        for (i, line) in text.lines().enumerate() {
            for shape in shapes_of(&scan_text(line)) {
                found = true;
                writeln!(out, "{path}:{}: {}", i + 1, shape.id())
                    .map_err(|e| format!("cannot write stdout: {e}"))?;
            }
        }
    }
    Ok(found)
}

fn main() -> ExitCode {
    match run() {
        Ok(false) => ExitCode::SUCCESS,
        Ok(true) => ExitCode::from(1),
        Err(msg) => {
            eprintln!("error: {msg}");
            ExitCode::from(2)
        }
    }
}
