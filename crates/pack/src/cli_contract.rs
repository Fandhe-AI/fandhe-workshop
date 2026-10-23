//! 作成者向け CLI `fandhe-pack` の外部仕様の契約（TASK-399 1/2・2/2、AUTHOR-1〜AUTHOR-3）。
//!
//! バイナリ名・コマンド一覧と引数の形・使い方の文言・終了コードの 3 値に加え、診断 1 行の項目の形・最終行
//! （`OK:`・`NG:`）・`compare` の結果行を、版つきの定数と型・関数で固定する。
//! 契約の正は `docs/cli.md`（CLI の外部仕様の版 11）で、本モジュールは同じ内容のコード側の写しである。
//! `vocab` の出力の形（ヘッダ・行・集計行・教材パック全体の行）は版 5 で足した（TASK-408）。
//! 両者の一致は `tests/cli_contract.rs` が機械で照合する。
//!
//! 呼び出し元は `cli`（振り分けと出力）と `main.rs`（終了コード）で、`cli` は本モジュールの名前を `pub use`
//! で再公開するため、`cli::EXIT_*`・`cli::Subcommand`・`cli::usage` の既存のパスは変わらない。
//! 後続の TASK-401（振り分け）・TASK-402（`metrics`）・TASK-403〜TASK-408・TASK-410 は、この契約を入力にする。
//! `metrics` の出力の形（ヘッダ・行・`TOTAL`）は版 4 で足した（TASK-402）。
//!
//! 診断 1 行の欄の分け方は `diag::Diagnostic` の Display が持ち、出し先（診断と最終行は標準出力、使い方と
//! 使い方の誤りは標準エラー出力）は `cli` が守る。`validate` への最終行の結線は `cli::run_validate`（TASK-403）が行い、`test` の模範解答の PASS の行は
//! 版 10 で、誤入力の PASS の行・FAIL の行・集計の 3 行は版 11 で足した（TASK-404 2/3・3/3）。`test` の最終行は TASK-405 の担当である。

use std::fmt;

use crate::source::{Format, LoadedPack, ScopeKind};

/// CLI の外部仕様の版。`docs/cli.md` の冒頭の版と一致させる。
///
/// `contract_version`・`ir_format`・診断コードの一覧の版とは別に管理する。
pub const CLI_CONTRACT_VERSION: u32 = 11;

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
            Subcommand::Compile => "<pack-dir> [--chapters <id>[,<id>...]] [-o <file>]",
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

/// `test` の模範解答 1 本が完走したことを示す PASS の行（AUTHOR-4、TASK-404 2/3）。
///
/// 呼び出し元は `cli::run_test` で、合格した模範解答ごとに 1 行書く。`id` は教材テストの読み取りが
/// `safe_ident` を通したもの（通らなければ位置の `#n`）だけを渡す前提で、本関数は値を写すだけにする。
/// `inputs` は流した入力の数（`tick` の進行を含み、開始点の切り替えは含まない）。
/// ID と全角括弧の間に空白は置かない（04 の書き方。PoC の資料の空白は採らない）。
pub fn test_pass_line(id: &str, inputs: usize) -> String {
    format!("PASS {id}（入力 {inputs} 件）")
}

/// `test` の誤入力 1 件が期待どおりだったことを示す PASS の行（AUTHOR-4、TASK-404 3/3）。
///
/// `observed_word` は観測の固定語（`rejected:bad_arg_shape` など。`play_run::Observed::word`）だけを渡す前提で、
/// 矢印は U+2192、前後は半角空白 1 つ。形の組み立てを本モジュールだけが持つ。
pub fn test_error_pass_line(id: &str, observed_word: &str) -> String {
    format!("PASS {id} → {observed_word}")
}

/// `test` の不合格の 1 件（模範解答または誤入力）の FAIL の行（AUTHOR-4、TASK-404 3/3）。
///
/// `expected`・`observed` は固定語彙だけを渡す前提（教材データの値は写さない）。`note` があれば末尾に全角括弧で
/// 位置の注記（区間と入力の番号など）を足す。呼び出し元は `cli::run_test`。
pub fn test_fail_line(id: &str, expected: &str, observed: &str, note: Option<&str>) -> String {
    match note {
        Some(n) => format!("FAIL {id}: 期待 {expected} に対して結果は {observed}（{n}）"),
        None => format!("FAIL {id}: 期待 {expected} に対して結果は {observed}"),
    }
}

/// `test` の集計の 3 行（模範解答の完走・誤入力の期待どおり・入力の総数。AUTHOR-4、TASK-404 3/3）。
pub fn test_summary_lines(
    pt_passed: usize,
    pt_total: usize,
    ei_passed: usize,
    ei_total: usize,
    inputs: usize,
) -> Vec<String> {
    vec![
        format!("模範解答: {pt_passed}/{pt_total} 完走"),
        format!("誤入力: {ei_passed}/{ei_total} が期待どおりの結果"),
        format!("入力の総数: {inputs}"),
    ]
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

/// `vocab` の表のヘッダ（AUTHOR-9）。
pub const VOCAB_HEADER: &str = "scope,step,kind,extension_points";

/// `vocab` の表の行の種別。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VocabRowKind {
    /// 通常の Step。拡張点を数える。
    Step,
    /// モジュールの呼び出し（`use`）。展開した先の Step で数えるため、拡張点は空で数にも入れない。
    Use,
}

impl VocabRowKind {
    const fn as_str(self) -> &'static str {
        match self {
            VocabRowKind::Step => "step",
            VocabRowKind::Use => "use",
        }
    }
}

/// 教材データ由来の ID を表の 1 欄として安全にする。制御文字（改行を含む）・行の区切りの U+2028・U+2029・`,`・`"` を
/// `\u{XXXX}`（16 進 4 桁・大文字）に直し、1 行 4 欄を保つ。`first` が真なら先頭の `#` も直し、表の行が
/// 集計行（`# ` 始まり）に見えないようにする。
fn vocab_field(s: &str, first: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        let escape = c.is_control()
            || matches!(c, ',' | '"' | '\u{2028}' | '\u{2029}')
            || (first && i == 0 && c == '#');
        if escape {
            out.push_str(&format!("\\u{{{:04X}}}", u32::from(c)));
        } else {
            out.push(c);
        }
    }
    out
}

/// `vocab` の表の 1 行。拡張点は ID を半角空白でつないで 1 欄に入れる（空なら空の欄）。
pub fn vocab_row(scope: &str, step: &str, kind: VocabRowKind, eps: &[&str]) -> String {
    format!(
        "{},{},{},{}",
        vocab_field(scope, true),
        vocab_field(step, false),
        kind.as_str(),
        eps.join(" ")
    )
}

/// `vocab` の集計行。（i）（ii）の後ろに、使われた拡張点ごとの Step 数を、渡された順に続ける。
pub fn vocab_summary_lines(i: usize, ii: usize, per_ep: &[(&str, usize)]) -> Vec<String> {
    let mut out = vec![
        format!("# (i) 既存の語彙の組み合わせだけで書けた Step: {i}"),
        format!("# (ii) 名前つき拡張点を使って書けた Step: {ii}"),
    ];
    out.extend(
        per_ep
            .iter()
            .map(|(ep, n)| format!("# {ep} を使った Step: {n}")),
    );
    out
}

/// `vocab` の教材パック全体の行。`field` は `pack.profile` のような固定の欄名で、教材データの値は写さない。
pub fn vocab_pack_line(ep_id: &str, field: &str) -> String {
    format!("# {ep_id} を使った（教材パック全体）: {field}")
}

#[cfg(test)]
mod test_line_tests {
    use super::*;

    #[test]
    fn pass_line_shape() {
        assert_eq!(test_pass_line("pt-main", 2), "PASS pt-main（入力 2 件）");
        assert_eq!(test_pass_line("#1", 0), "PASS #1（入力 0 件）");
    }

    #[test]
    fn error_pass_and_fail_line_shape() {
        assert_eq!(
            test_error_pass_line("e01", "rejected:bad_arg_shape"),
            "PASS e01 \u{2192} rejected:bad_arg_shape"
        );
        assert_eq!(
            test_fail_line(
                "e03",
                "rejected:missing_arg",
                "rejected:bad_arg_shape",
                None
            ),
            "FAIL e03: 期待 rejected:missing_arg に対して結果は rejected:bad_arg_shape"
        );
        assert_eq!(
            test_fail_line("pt-main", "achieved", "progress", Some("区間 1 の入力 2")),
            "FAIL pt-main: 期待 achieved に対して結果は progress（区間 1 の入力 2）"
        );
    }

    #[test]
    fn summary_lines_shape() {
        assert_eq!(
            test_summary_lines(1, 2, 3, 3, 9),
            [
                "模範解答: 1/2 完走",
                "誤入力: 3/3 が期待どおりの結果",
                "入力の総数: 9"
            ]
        );
    }
}

#[cfg(test)]
mod vocab_tests {
    use super::*;

    #[test]
    fn v1_row_shape_and_escape() {
        assert_eq!(
            vocab_row("c1", "s1", VocabRowKind::Step, &["EP-1", "EP-7"]),
            "c1,s1,step,EP-1 EP-7"
        );
        assert_eq!(vocab_row("m1", "u1", VocabRowKind::Use, &[]), "m1,u1,use,");
        let r = vocab_row("#a,b\"c\nd", "e,f", VocabRowKind::Step, &[]);
        assert_eq!(
            r,
            "\\u{0023}a\\u{002C}b\\u{0022}c\\u{000A}d,e\\u{002C}f,step,"
        );
        assert_eq!(r.matches(',').count(), 3);
        assert!(!r.starts_with('#'));
        // 先頭以外の `#` と step 欄の先頭の `#` はそのまま。
        assert_eq!(
            vocab_row("a#", "#b", VocabRowKind::Step, &[]),
            "a#,#b,step,"
        );
    }

    #[test]
    fn v2_summary_and_pack_lines() {
        assert_eq!(
            vocab_summary_lines(2, 1, &[("EP-1", 1)]),
            [
                "# (i) 既存の語彙の組み合わせだけで書けた Step: 2",
                "# (ii) 名前つき拡張点を使って書けた Step: 1",
                "# EP-1 を使った Step: 1"
            ]
        );
        assert_eq!(
            vocab_pack_line("EP-10", "pack.profile"),
            "# EP-10 を使った（教材パック全体）: pack.profile"
        );
    }
}
