//! 作成者向けの診断（エラー）の型と、1 行の出力形。
//!
//! コードの一覧は `docs/diagnostics.md`（TASK-195）で固定した。出力形と終了コードは
//! `docs/cli.md`（版 4）の 6〜9 節で固定した（TASK-399）。`cli` の各コマンドが診断を集め、
//! ここの関数で出力と終了コードに変える。

use std::fmt;
use std::io;

use crate::cli::{EXIT_DIAGNOSTICS, EXIT_OK};

/// 診断コード（`E-` で始まる）。一覧の正は `docs/diagnostics.md` である。
///
/// 一覧と本列挙の一致は `crates/pack/tests/diagnostics_doc.rs`（TASK-400）が検査する。
/// `E-LIMIT-*` の拒否理由の型は contract
/// （TASK-199 の `PackLimitCode`）が持ち、一覧には参照だけを置く。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DiagCode {
    /// 構文として読めない。
    Parse,
    /// `pack.ir_format` が、エンジンの読める版（1 と 2）のどちらでもない。
    FormatVersion,
    /// 構造の誤り（片方だけの指定・排他の違反・正答の無い選択式など）。
    Structure,
    /// 未定義の Step を参照している。
    RefStep,
    /// 未定義の拒否理由を参照している。
    RefRejection,
    /// 未定義の本文を参照している。
    RefBody,
    /// 未定義のフィクスチャを参照している。
    RefFixture,
    /// 未定義のヒントを参照している。
    RefHint,
    /// 存在しない要素への `remove`・`set_attr`（どの `put`・`replace` も置かない経路。PACK-30・D-43）。
    RefElement,
    /// `pack.vocab` の宣言より新しい語彙の版の拡張点を使っている（PACK-31・VER-14・D-43）。
    VocabMismatch,
    /// 未定義のモジュールを参照している。
    RefModule,
    /// 未定義のコマンドを参照している。
    RefCommand,
    /// 未定義のビューを参照している。
    RefView,
    /// ID が重複している。
    DupId,
    /// 到達できない Step がある。
    Unreachable,
    /// 行き止まりの Step がある。
    DeadEnd,
    /// モジュールの呼び出しが巡回している、または入れ子が 8 段を超える。
    ModuleCycle,
    /// 拒否・問い返しの文面で「次の行動」が空になる。
    EmptyNextAction,
    /// 入力を持つ Step が、確認も理由つきの省略の明示も持たない。
    NoConfirm,
    /// 差し込みの構文が不正。
    SlotSyntax,
    /// 未知の差し込みである。
    SlotUnknown,
    /// 差し込みを使えない文脈にある。
    SlotContext,
    /// 差し込みに do が要る。
    SlotNeedsDo,
    /// 差し込みに代替がない。
    SlotNoFallback,
    /// 隠すと宣言した語が、その水準の本文または `do` に含まれている。
    GuidanceLeak,
    /// ヒントを持つ Step が、ある案内レベルでヒントを 1 つも出さない。
    HintHidden,
    /// プロファイルの接頭辞が不正。
    ProfilePrefix,
    /// プロファイルのコマンド名が不正。
    ProfileCommandName,
    /// 接頭辞なしの設定で、自然文の入力・規則がある。
    ProfileText,
    /// 入力欄なしの設定で、コマンド・自然文の入力・規則がある。
    ProfileNoInput,
    /// `test` で、教材テストの入力の種類が画面での分類と異なる。
    PlayInputClass,
    /// 通し試験が完了していない。一覧の版 1 の本表外（04 に出所が無い）。
    PlayIncomplete,
    /// 通し試験の分類が一致しない。一覧の版 1 の本表外（04 に出所が無い）。
    PlayCategoryMismatch,
    /// `view_tabs: hidden` で 2 つ以上のビューを宣言した Step がある。
    LeftMultiView,
    /// 宛先の経路が行にならない（静的な近似）。
    LeftTargetRow,
    /// 宛先のビューが、その Step の画面に出ない。
    LeftTargetView,
    /// 教材パックの値が秘密情報らしい形をしている（AUTHOR-18・D-81。`validate` だけ）。
    SecretShape,
    /// 練習データの見出しに【架空】の印が無い（AUTHOR-18・D-81。`validate` だけ）。
    FictionalMark,
    /// 上限を超えている。正は contract の `PackLimitCode` と engine-safety で、一覧は参照のみ。
    Limit,
    /// 深さの上限を超えている（SAFE-43）。正は contract の `PackLimitCode`。
    LimitDepth,
    /// Markdown の大きさの上限を超えている（SAFE-22）。正は contract の `PackLimitCode`。
    LimitMdSize,
    /// パック全体の大きさの上限を超えている（SAFE-22）。正は contract の `PackLimitCode`。
    LimitPackSize,
    /// シンボリックリンクが含まれている（SAFE-43）。正は contract の `PackLimitCode`。
    LimitSymlink,
}

impl DiagCode {
    /// 全件の一覧。
    pub const ALL: &'static [DiagCode] = &[
        DiagCode::Parse,
        DiagCode::FormatVersion,
        DiagCode::Structure,
        DiagCode::RefStep,
        DiagCode::RefRejection,
        DiagCode::RefBody,
        DiagCode::RefFixture,
        DiagCode::RefHint,
        DiagCode::RefElement,
        DiagCode::VocabMismatch,
        DiagCode::RefModule,
        DiagCode::RefCommand,
        DiagCode::RefView,
        DiagCode::DupId,
        DiagCode::Unreachable,
        DiagCode::DeadEnd,
        DiagCode::ModuleCycle,
        DiagCode::EmptyNextAction,
        DiagCode::NoConfirm,
        DiagCode::SlotSyntax,
        DiagCode::SlotUnknown,
        DiagCode::SlotContext,
        DiagCode::SlotNeedsDo,
        DiagCode::SlotNoFallback,
        DiagCode::GuidanceLeak,
        DiagCode::HintHidden,
        DiagCode::ProfilePrefix,
        DiagCode::ProfileCommandName,
        DiagCode::ProfileText,
        DiagCode::ProfileNoInput,
        DiagCode::PlayInputClass,
        DiagCode::PlayIncomplete,
        DiagCode::PlayCategoryMismatch,
        DiagCode::LeftMultiView,
        DiagCode::LeftTargetRow,
        DiagCode::LeftTargetView,
        DiagCode::SecretShape,
        DiagCode::FictionalMark,
        DiagCode::Limit,
        DiagCode::LimitDepth,
        DiagCode::LimitMdSize,
        DiagCode::LimitPackSize,
        DiagCode::LimitSymlink,
    ];

    /// `E-...` 形式の文字列を返す。
    pub const fn as_str(self) -> &'static str {
        match self {
            DiagCode::Parse => "E-PARSE",
            DiagCode::FormatVersion => "E-FORMAT-VERSION",
            DiagCode::Structure => "E-STRUCTURE",
            DiagCode::RefStep => "E-REF-STEP",
            DiagCode::RefRejection => "E-REF-REJECTION",
            DiagCode::RefBody => "E-REF-BODY",
            DiagCode::RefFixture => "E-REF-FIXTURE",
            DiagCode::RefHint => "E-REF-HINT",
            DiagCode::RefElement => "E-REF-ELEMENT",
            DiagCode::VocabMismatch => "E-VOCAB-MISMATCH",
            DiagCode::RefModule => "E-REF-MODULE",
            DiagCode::RefCommand => "E-REF-COMMAND",
            DiagCode::RefView => "E-REF-VIEW",
            DiagCode::DupId => "E-DUP-ID",
            DiagCode::Unreachable => "E-UNREACHABLE",
            DiagCode::DeadEnd => "E-DEAD-END",
            DiagCode::ModuleCycle => "E-MODULE-CYCLE",
            DiagCode::EmptyNextAction => "E-EMPTY-NEXT-ACTION",
            DiagCode::NoConfirm => "E-NO-CONFIRM",
            DiagCode::SlotSyntax => "E-SLOT-SYNTAX",
            DiagCode::SlotUnknown => "E-SLOT-UNKNOWN",
            DiagCode::SlotContext => "E-SLOT-CONTEXT",
            DiagCode::SlotNeedsDo => "E-SLOT-NEEDS-DO",
            DiagCode::SlotNoFallback => "E-SLOT-NO-FALLBACK",
            DiagCode::GuidanceLeak => "E-GUIDANCE-LEAK",
            DiagCode::HintHidden => "E-HINT-HIDDEN",
            DiagCode::ProfilePrefix => "E-PROFILE-PREFIX",
            DiagCode::ProfileCommandName => "E-PROFILE-COMMAND-NAME",
            DiagCode::ProfileText => "E-PROFILE-TEXT",
            DiagCode::ProfileNoInput => "E-PROFILE-NO-INPUT",
            DiagCode::PlayInputClass => "E-PLAY-INPUT-CLASS",
            DiagCode::PlayIncomplete => "E-PLAY-INCOMPLETE",
            DiagCode::PlayCategoryMismatch => "E-PLAY-CATEGORY-MISMATCH",
            DiagCode::LeftMultiView => "E-LEFT-MULTI-VIEW",
            DiagCode::LeftTargetRow => "E-LEFT-TARGET-ROW",
            DiagCode::LeftTargetView => "E-LEFT-TARGET-VIEW",
            DiagCode::SecretShape => "E-SECRET-SHAPE",
            DiagCode::FictionalMark => "E-FICTIONAL-MARK",
            DiagCode::Limit => "E-LIMIT",
            DiagCode::LimitDepth => "E-LIMIT-DEPTH",
            DiagCode::LimitMdSize => "E-LIMIT-MD-SIZE",
            DiagCode::LimitPackSize => "E-LIMIT-PACK-SIZE",
            DiagCode::LimitSymlink => "E-LIMIT-SYMLINK",
        }
    }
}

impl fmt::Display for DiagCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 診断を作れない理由。「診断は必ずファイルと項目を持つ」を守るために返す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticError {
    /// ファイルが空である。
    EmptyFile,
    /// 項目が空である。
    EmptyItem,
}

impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiagnosticError::EmptyFile => f.write_str("diagnostic file must not be empty"),
            DiagnosticError::EmptyItem => f.write_str("diagnostic item must not be empty"),
        }
    }
}

impl std::error::Error for DiagnosticError {}

/// 診断 1 件。出力は `<ファイル>: <項目>: <コード>: <メッセージ>` の 1 行（行・桁は持たない）。
///
/// 欄は非公開で、`new` を通さないと作れない。各コマンドが集めて `write_diagnostics` に渡す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    file: String,
    item: String,
    code: DiagCode,
    message: String,
}

impl Diagnostic {
    /// 診断を作る。`file` か `item` が空（空白のみを含む）なら `Err` を返す。
    pub fn new(
        file: impl Into<String>,
        item: impl Into<String>,
        code: DiagCode,
        message: impl Into<String>,
    ) -> Result<Self, DiagnosticError> {
        let file = file.into();
        let item = item.into();
        if file.trim().is_empty() {
            return Err(DiagnosticError::EmptyFile);
        }
        if item.trim().is_empty() {
            return Err(DiagnosticError::EmptyItem);
        }
        Ok(Self {
            file,
            item,
            code,
            message: message.into(),
        })
    }

    /// 対象のファイル。
    pub fn file(&self) -> &str {
        &self.file
    }

    /// 対象の項目。
    pub fn item(&self) -> &str {
        &self.item
    }

    /// 診断コード。
    pub fn code(&self) -> DiagCode {
        self.code
    }

    /// メッセージ。
    pub fn message(&self) -> &str {
        &self.message
    }
}

/// 制御文字を `\u{XXXX}` に直して書く。教材データ由来の改行で 1 行の形が崩れるのを防ぐ。
///
/// `split_fields` が真のとき（file・item の欄）は、欄の区切りの `: ` のうち空白を `\u{0020}` に直す。ID・パスは
/// 教材データ由来で `: ` を含みうるため、直さないと先頭から 3 つの `: で区切る` 読み方が崩れる
/// （`docs/cli.md` 版 4 の 6 節）。最後の欄のメッセージは区切りを持たないので直さない。
fn write_escaped(f: &mut fmt::Formatter<'_>, s: &str, split_fields: bool) -> fmt::Result {
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c.is_control() {
            write!(f, "\\u{{{:04X}}}", u32::from(c))?;
        } else if split_fields && c == ':' && chars.peek() == Some(&' ') {
            f.write_str(":\\u{0020}")?;
            chars.next();
        } else {
            fmt::Write::write_char(f, c)?;
        }
    }
    Ok(())
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_escaped(f, &self.file, true)?;
        f.write_str(": ")?;
        write_escaped(f, &self.item, true)?;
        write!(f, ": {}: ", self.code)?;
        write_escaped(f, &self.message, false)
    }
}

/// 診断の件数から終了コードを決める。0 件なら 0、1 件以上なら 1。
pub fn exit_code(diags: &[Diagnostic]) -> u8 {
    if diags.is_empty() {
        EXIT_OK
    } else {
        EXIT_DIAGNOSTICS
    }
}

/// 診断を 1 件 1 行で書き出し、終了コードを返す。
///
/// 最終行（`OK:`・`NG:`）は出さない（`validate` の最終行は `cli_contract::final_line` で、結線は TASK-403）。
pub fn write_diagnostics<W: io::Write>(out: &mut W, diags: &[Diagnostic]) -> io::Result<u8> {
    for d in diags {
        writeln!(out, "{d}")?;
    }
    Ok(exit_code(diags))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diag(file: &str, item: &str, msg: &str) -> Diagnostic {
        Diagnostic::new(file, item, DiagCode::RefStep, msg).unwrap()
    }

    #[test]
    fn d1_fixed_line() {
        let d = diag(
            "chapters/c1/chapter.yaml",
            "step:s1",
            "未定義の Step s9 を参照しています",
        );
        assert_eq!(
            d.to_string(),
            "chapters/c1/chapter.yaml: step:s1: E-REF-STEP: 未定義の Step s9 を参照しています"
        );
        for item in ["rejection:R1", "playthrough:P1"] {
            let d = diag("a.yaml", item, "m");
            assert_eq!(d.to_string(), format!("a.yaml: {item}: E-REF-STEP: m"));
        }
    }

    #[test]
    fn d2_four_fields() {
        let d = diag("a.yaml", "step:s1", "message");
        let s = d.to_string();
        let parts: Vec<&str> = s.splitn(4, ": ").collect();
        assert_eq!(parts, ["a.yaml", "step:s1", "E-REF-STEP", "message"]);
    }

    #[test]
    fn d3_empty_fields() {
        assert_eq!(
            Diagnostic::new("", "i", DiagCode::Parse, "m"),
            Err(DiagnosticError::EmptyFile)
        );
        assert_eq!(
            Diagnostic::new("  ", "i", DiagCode::Parse, "m"),
            Err(DiagnosticError::EmptyFile)
        );
        assert_eq!(
            Diagnostic::new("f", "", DiagCode::Parse, "m"),
            Err(DiagnosticError::EmptyItem)
        );
    }

    #[test]
    fn d4_control_chars_escaped() {
        let d = diag("a\nb.yaml", "step:\r1", "一行目\nE-FAKE: 偽の行");
        let s = d.to_string();
        assert_eq!(s.lines().count(), 1);
        assert!(s.contains("\\u{000A}"));
        assert!(s.contains("\\u{000D}"));
        assert!(s.contains("一行目"));
    }

    /// コードの形式（`E-` の接頭辞・区切りの文字種・重複なし・表示の一致）だけを見る。
    /// 件数は `tests/diagnostics_doc.rs` が `docs/diagnostics.md` の記述と照合する。
    #[test]
    fn d5_codes_are_well_formed() {
        let mut seen = std::collections::HashSet::new();
        for c in DiagCode::ALL {
            let s = c.as_str();
            let rest = s.strip_prefix("E-").expect("E- prefix");
            assert!(!rest.is_empty());
            for seg in rest.split('-') {
                assert!(!seg.is_empty());
                assert!(seg
                    .chars()
                    .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit()));
            }
            assert!(seen.insert(s), "duplicate {s}");
            assert_eq!(c.to_string(), s);
        }
    }

    /// `ALL` が全変種を過不足なく含むこと。`match` にワイルドカードが無いので、変種を足すとここの更新が
    /// コンパイルで強制され、`ALL` への入れ忘れは番号の不足として失敗する（文書との一致は
    /// `tests/diagnostics_doc.rs` が見る）。
    #[test]
    fn d9_all_covers_every_variant() {
        fn index(c: DiagCode) -> usize {
            match c {
                DiagCode::Parse => 0,
                DiagCode::FormatVersion => 1,
                DiagCode::Structure => 2,
                DiagCode::RefStep => 3,
                DiagCode::RefRejection => 4,
                DiagCode::RefBody => 5,
                DiagCode::RefFixture => 6,
                DiagCode::RefHint => 7,
                DiagCode::RefElement => 8,
                DiagCode::VocabMismatch => 9,
                DiagCode::RefModule => 10,
                DiagCode::RefCommand => 11,
                DiagCode::RefView => 12,
                DiagCode::DupId => 13,
                DiagCode::Unreachable => 14,
                DiagCode::DeadEnd => 15,
                DiagCode::ModuleCycle => 16,
                DiagCode::EmptyNextAction => 17,
                DiagCode::NoConfirm => 18,
                DiagCode::SlotSyntax => 19,
                DiagCode::SlotUnknown => 20,
                DiagCode::SlotContext => 21,
                DiagCode::SlotNeedsDo => 22,
                DiagCode::SlotNoFallback => 23,
                DiagCode::GuidanceLeak => 24,
                DiagCode::HintHidden => 25,
                DiagCode::ProfilePrefix => 26,
                DiagCode::ProfileCommandName => 27,
                DiagCode::ProfileText => 28,
                DiagCode::ProfileNoInput => 29,
                DiagCode::PlayInputClass => 30,
                DiagCode::PlayIncomplete => 31,
                DiagCode::PlayCategoryMismatch => 32,
                DiagCode::LeftMultiView => 33,
                DiagCode::LeftTargetRow => 34,
                DiagCode::LeftTargetView => 35,
                DiagCode::SecretShape => 36,
                DiagCode::FictionalMark => 37,
                DiagCode::Limit => 38,
                DiagCode::LimitDepth => 39,
                DiagCode::LimitMdSize => 40,
                DiagCode::LimitPackSize => 41,
                DiagCode::LimitSymlink => 42,
            }
        }
        let mut got: Vec<usize> = DiagCode::ALL.iter().map(|c| index(*c)).collect();
        got.sort_unstable();
        // 期待する変種数は `ALL` から作らない（`ALL` の入れ忘れが want 側にも現れず見逃されるため）。
        // 上の `index` の最大値 + 1 と同じ値を、変種を足すたびに手で更新する。
        const VARIANTS: usize = 43;
        let want: Vec<usize> = (0..VARIANTS).collect();
        assert_eq!(got, want);
    }

    #[test]
    fn d6_exit_code() {
        assert_eq!(exit_code(&[]), 0);
        let d = diag("f", "i", "m");
        assert_eq!(exit_code(std::slice::from_ref(&d)), 1);
        assert_eq!(exit_code(&[d.clone(), d]), 1);
    }

    #[test]
    fn d8_separator_in_file_and_item_keeps_four_fields() {
        let d = diag("a: b.yaml", "step:x: y", "msg: with sep");
        let s = d.to_string();
        let parts: Vec<&str> = s.splitn(4, ": ").collect();
        assert_eq!(
            parts,
            [
                "a:\\u{0020}b.yaml",
                "step:x:\\u{0020}y",
                "E-REF-STEP",
                "msg: with sep"
            ]
        );
        // 末尾が `:` の欄でも、区切りの `: ` を取り違えない。
        let d = diag("a:", "step:", "m");
        let s = d.to_string();
        let parts: Vec<&str> = s.splitn(4, ": ").collect();
        assert_eq!(parts, ["a:", "step:", "E-REF-STEP", "m"]);
    }

    #[test]
    fn d7_write_diagnostics() {
        let mut out = Vec::new();
        assert_eq!(write_diagnostics(&mut out, &[]).unwrap(), 0);
        assert!(out.is_empty());
        let ds = [diag("a", "i", "m1"), diag("b", "j", "m2")];
        let code = write_diagnostics(&mut out, &ds).unwrap();
        assert_eq!(code, exit_code(&ds));
        assert_eq!(String::from_utf8(out).unwrap().lines().count(), 2);
    }
}
