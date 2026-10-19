//! 作成者向け CLI `fandhe-pack` の外部仕様の契約（TASK-399 1/2、AUTHOR-1・AUTHOR-2）。
//!
//! バイナリ名・コマンド一覧と引数の形・使い方の文言・終了コードの 3 値を、版つきの定数と型で固定する。
//! 契約の正は `docs/cli.md`（CLI の外部仕様の版 1）で、本モジュールは同じ内容のコード側の写しである。
//! 両者の一致は `tests/cli_contract.rs` が機械で照合する。
//!
//! 呼び出し元は `cli`（振り分けと出力）と `main.rs`（終了コード）で、`cli` は本モジュールの名前を `pub use`
//! で再公開するため、`cli::EXIT_*`・`cli::Subcommand`・`cli::usage` の既存のパスは変わらない。
//! 後続の TASK-401（振り分け）・TASK-402（`metrics`）・TASK-403〜TASK-408・TASK-410 は、この契約を入力にする。
//! `metrics` は本件で枠と使い方に加えており、TASK-402 は本体の実装だけを足す。
//!
//! 診断 1 行・項目の書式・最終行・診断の出し先は本版の対象外で、TASK-399 2/2（#471）が文書とここに足す。

/// CLI の外部仕様の版。`docs/cli.md` の冒頭の版と一致させる。
///
/// `contract_version`・`ir_format`・診断コードの一覧の版とは別に管理する。
pub const CLI_CONTRACT_VERSION: u32 = 1;

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
