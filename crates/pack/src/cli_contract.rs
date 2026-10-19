//! 作成者向け CLI `fandhe-pack` の外部仕様の契約（TASK-399 1/2・2/2、AUTHOR-1〜AUTHOR-3）。
//!
//! バイナリ名・コマンド一覧と引数の形・使い方の文言・終了コードの 3 値に加え、診断 1 行の項目の形・最終行
//! （`OK:`・`NG:`）・`compare` の結果行を、版つきの定数と型・関数で固定する。
//! 契約の正は `docs/cli.md`（CLI の外部仕様の版 4）で、本モジュールは同じ内容のコード側の写しである。
//! 両者の一致は `tests/cli_contract.rs` が機械で照合する。
//!
//! 呼び出し元は `cli`（振り分けと出力）と `main.rs`（終了コード）で、`cli` は本モジュールの名前を `pub use`
//! で再公開するため、`cli::EXIT_*`・`cli::Subcommand`・`cli::usage` の既存のパスは変わらない。
//! 後続の TASK-401（振り分け）・TASK-402（`metrics`）・TASK-403〜TASK-408・TASK-410 は、この契約を入力にする。
//! `metrics` の出力の形（ヘッダ・行・`TOTAL`）は版 4 で足した（TASK-402）。
//!
//! 診断 1 行の欄の分け方は `diag::Diagnostic` の Display が持ち、出し先（診断と最終行は標準出力、使い方と
//! 使い方の誤りは標準エラー出力）は `cli` が守る。`validate` への最終行の結線は TASK-403、`test` の最終行と
//! 集計は TASK-404・TASK-405 の担当で、本モジュールは `validate` の最終行の形と `compare` の結果行だけを持つ。

use std::fmt;

use crate::source::{Format, LoadedPack, ScopeKind};

/// CLI の外部仕様の版。`docs/cli.md` の冒頭の版と一致させる。
///
/// `contract_version`・`ir_format`・診断コードの一覧の版とは別に管理する。
pub const CLI_CONTRACT_VERSION: u32 = 4;

/// バイナリ名。
pub const BIN_NAME: &str = "fandhe-pack";

/// 診断 0 件で完了した（`compare` は IR が一致した）。
pub const EXIT_OK: u8 = 0;
/// 診断が 1 件以上ある（`compare` は IR が一致しなかった場合も含む）。
pub const EXIT_DIAGNOSTICS: u8 = 1;
/// 使い方の誤り。結果を書き出せない場合も、成功の 0 と取り違えないようこの値で終える。
pub const EXIT_USAGE: u8 = 2;

/// 作成者向けのサブコマンド。並びは AUTHOR-1 の順である。`defects` は作成者向けではないため置かない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subcommand {
    /// パックを静的に検査する。
    Validate,
    /// 検査の後に、模範解答の完走と誤入力の分類を確かめる。
    Test,
    /// パックを IR（JSON）へ変換する。
    Compile,
    /// 拡張点を使った Step を数え上げる。
    Vocab,
    /// ファイルごとの行数・実効行数・バイト数・入れ子の深さを出す。
    Metrics,
    /// 2 つのパックの IR の完全一致を比べる。
    Compare,
}

impl Subcommand {
    /// 全サブコマンド（AUTHOR-1 の順）。
    pub const ALL: [Subcommand; 6] = [
        Subcommand::Validate,
        Subcommand::Test,
        Subcommand::Compile,
        Subcommand::Vocab,
        Subcommand::Metrics,
        Subcommand::Compare,
    ];

    /// コマンド名。
    pub const fn name(self) -> &'static str {
        match self {
            Subcommand::Validate => "validate",
            Subcommand::Test => "test",
            Subcommand::Compile => "compile",
            Subcommand::Vocab => "vocab",
            Subcommand::Metrics => "metrics",
            Subcommand::Compare => "compare",
        }
    }

    /// 引数の形（使い方と文書の表に出す文字列）。
    pub const fn args(self) -> &'static str {
        match self {
            Subcommand::Compile => "<pack-dir> [-o <file>]",
            Subcommand::Compare => "<pack-dir-a> <pack-dir-b>",
            Subcommand::Validate | Subcommand::Test | Subcommand::Vocab | Subcommand::Metrics => {
                "<pack-dir>"
            }
        }
    }

    /// コマンド名から引く。未知なら `None`。
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

/// 使い方（英語）。標準エラー出力に出す。末尾は改行 1 つで終わる。
///
/// 文言は `Subcommand::ALL` から組み立て、二重に定義しない。
pub fn usage() -> String {
    let mut s = format!("usage: {BIN_NAME} <command> [<args>]\n\ncommands:\n");
    for c in Subcommand::ALL {
        s.push_str(&format!("  {} {}\n", c.name(), c.args()));
    }
    s
}

/// AUTHOR-2 が名指しする項目の種別（`step:<ID>` などの `:` の前）。診断はこの名前で出す。
pub const AUTHOR2_ITEM_KINDS: [&str; 5] =
    ["step", "rejection", "start", "playthrough", "error_input"];

/// 版 3 時点の実装が使う項目の種別の参考の一覧。種別を足しても版は上げない（形は `is_item` が見る）。
pub const ITEM_KINDS_V2: &[&str] = &[
    "step",
    "rejection",
    "start",
    "playthrough",
    "error_input",
    "fixture",
    "view",
    "command",
    "defaults",
    "pack",
    "file",
    "module",
];

/// 診断の項目の形か。`<種別>` または `<種別>:<名前>` で、種別は `[a-z][a-z_]*`、名前は空でない。
///
/// 診断の各欄の検査を出力の段（適合試験）に置くための判定で、`Diagnostic::new` は呼ばない
/// （データ由来の空の ID で panic させず、空だけを型で拒否する）。
pub fn is_item(s: &str) -> bool {
    let (kind, name) = match s.split_once(':') {
        Some((k, n)) => (k, Some(n)),
        None => (s, None),
    };
    let mut chars = kind.chars();
    let kind_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c == '_');
    kind_ok && name.is_none_or(|n| !n.is_empty())
}

/// 読み込めなかったときの最終行の括弧の中身（AUTHOR-3）。
pub const UNREADABLE: &str = "読み込めません";

/// 最終行の括弧に入れる教材パックの要約。数はコードが数え、教材データの文字列は写さない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackSummary {
    /// 人が書いた形式。
    pub format: Format,
    /// 章の数。
    pub chapters: usize,
    /// モジュールの数。
    pub modules: usize,
    /// 全スコープの Step の定義の数。
    pub step_defs: usize,
}

impl PackSummary {
    /// 読み込んだパックから数える。
    pub fn of(pack: &LoadedPack) -> Self {
        let count = |k: ScopeKind| pack.scopes.iter().filter(|s| s.kind == k).count();
        Self {
            format: pack.format,
            chapters: count(ScopeKind::Chapter),
            modules: count(ScopeKind::Module),
            step_defs: pack.scopes.iter().map(|s| s.src.steps.len()).sum(),
        }
    }
}

/// 形式のラベル（最終行に出す）。
const fn format_label(f: Format) -> &'static str {
    match f {
        Format::Toml => "A(TOML)",
        Format::Yaml => "B(YAML)",
    }
}

impl fmt::Display for PackSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "形式 {}、章 {}、モジュール {}、Step の定義 {}",
            format_label(self.format),
            self.chapters,
            self.modules,
            self.step_defs
        )
    }
}

/// `validate` の最終行の材料。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalLine {
    /// 読み込めて、検査を終えた。
    Checked {
        /// 診断の件数。
        diagnostics: usize,
        /// パックの要約。
        summary: PackSummary,
    },
    /// 読み込めなかった（AUTHOR-3）。
    Unreadable {
        /// 診断の件数。
        diagnostics: usize,
    },
}

/// 最終行を組み立てる。`OK:` になるのは、読み込めて診断が 0 件のときだけである（fail-closed）。
pub fn final_line(line: &FinalLine) -> String {
    match line {
        FinalLine::Checked {
            diagnostics: 0,
            summary,
        } => format!("OK: 診断 0 件（{summary}）"),
        FinalLine::Checked {
            diagnostics,
            summary,
        } => format!("NG: 診断 {diagnostics} 件（{summary}）"),
        FinalLine::Unreadable { diagnostics } => {
            format!("NG: 診断 {diagnostics} 件（{UNREADABLE}）")
        }
    }
}

/// `compare` の一致の行。
pub fn compare_same_line(len: usize) -> String {
    format!("OK: IR が一致しました（{len} バイト）")
}

/// `compare` の不一致の行。`line` は 1 始まりの行、`offset` は 0 始まりのバイト位置で、表示は 1 始まりの序数にする。
pub fn compare_differ_line(line: usize, offset: usize) -> String {
    format!(
        "NG: IR が一致しません（{line} 行目、{} バイト目）",
        offset + 1
    )
}

/// `compare` で、どちらかのパックを読めなかったときの最終行。件数は合計と a・b の別を出す。
pub fn compare_unreadable_line(a: usize, b: usize) -> String {
    format!(
        "NG: 診断 {} 件（比べられません。a {a} 件、b {b} 件）",
        a + b
    )
}

/// `metrics` の出力のヘッダ行。
pub const METRICS_HEADER: &str = "file,lines,effective_lines,bytes,max_depth";

/// `metrics` の合計の行の `file` 欄に置く語。
pub const METRICS_TOTAL_LABEL: &str = "TOTAL";

/// `metrics` の 1 ファイルの行。`file` の制御文字は `\u{XXXX}`、`,` は `\u{002C}`、`"` は `\u{0022}` に直し、1 行 5 欄を保つ。
pub fn metrics_row(file: &str, lines: u64, effective: u64, bytes: u64, depth: u64) -> String {
    let mut f = String::new();
    for c in file.chars() {
        if c.is_control() || c == ',' || c == '"' {
            f.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
        } else {
            f.push(c);
        }
    }
    format!("{f},{lines},{effective},{bytes},{depth}")
}
