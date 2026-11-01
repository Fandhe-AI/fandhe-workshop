//! `fandhe-pack preview` の関門（AUTHOR-22・AUTHOR-2、TASK-429 1/3）。
//!
//! 呼び出し元は `cli::run_preview`。教材パックを配る前に、読み込み → 静的検査（`validate`）→ `compile` の順に
//! 通し、どこかの層に診断が 1 件でもあれば `Gate::Unreadable` か `Gate::Diagnosed` で返す。配る側（TASK-429 2/3）は
//! `Gate::Ready` のときだけ待ち受けを開く構造にし、診断のある教材パックを配らない（fail-closed）。
//!
//! 層は空でない最初の層で止める。`validate` と `compile` は一部の検査が重複するため、両方を無条件に流すと
//! 二重に報告される。教材テスト（`tests/playthrough.yaml`）は `test` の持ち分で、関門には入れない。
//! ファイルシステムに触れるのは `source::load` だけで、何も作らない。

use std::path::Path;

use crate::cli_contract::PackSummary;
use crate::compile::{compile_loaded, diag, CompileError, CompileOptions};
use crate::diag::{DiagCode, Diagnostic};
use crate::source::{self, LoadedPack};
use crate::validate;

/// 関門の結果。
#[derive(Debug)]
pub enum Gate {
    /// 読み込めなかった。診断は読み込み層のもの。
    Unreadable(Vec<Diagnostic>),
    /// 読み込めたが、`validate` か `compile` に診断がある。配ってはならない。
    Diagnosed {
        /// 最初に診断が出た層の診断。
        diags: Vec<Diagnostic>,
        /// 最終行に出す教材パックの要約。
        summary: PackSummary,
    },
    /// 診断 0 件。`ir` は `fandhe-pack compile <pack-dir>` の出力とバイト単位で同じ。
    Ready {
        /// 配る IR のバイト列。
        ir: Vec<u8>,
        /// 教材パックの要約。
        summary: PackSummary,
    },
}

/// ディレクトリの教材パックを関門に通す。
pub fn gate(dir: &Path) -> Gate {
    match source::load(dir) {
        Err(diags) => Gate::Unreadable(diags),
        Ok(pack) => gate_loaded(&pack),
    }
}

/// 読み込み済みの教材パックを関門に通す（`validate` → `compile`）。
pub fn gate_loaded(pack: &LoadedPack) -> Gate {
    let summary = PackSummary::of(pack);
    let diags = validate::validate(pack);
    if !diags.is_empty() {
        return Gate::Diagnosed { diags, summary };
    }
    match compile_loaded(pack, &CompileOptions::default()) {
        Ok(ir) => Gate::Ready { ir, summary },
        // 診断が空の失敗は成功にしない。
        Err(CompileError::Diagnostics(diags)) if !diags.is_empty() => {
            Gate::Diagnosed { diags, summary }
        }
        // 章を選ばない呼び出しでは起きない。起きても配らない。
        Err(_) => Gate::Diagnosed {
            diags: vec![diag(
                &pack.workshop_file,
                "pack",
                DiagCode::Structure,
                "unexpected compile failure",
            )],
            summary,
        },
    }
}
