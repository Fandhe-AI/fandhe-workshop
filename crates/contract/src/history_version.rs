//! 履歴の 4 軸の版の欄と、判定結果の型の契約（TASK-173 1/2、版 1）。
//!
//! 履歴は `versions` の欄に、エンジン・教材形式・教材内容・参照元の 4 軸の版を記録する（VER-1、C-12）。
//! 呼び出し元は、版を書く側（core、TASK-173 2/2）、開く可否を判定する側（core、TASK-174）、
//! 保存・復元・復習の画面（web、TASK-176・TASK-178）である。履歴の包み（TASK-309）と理由コードの
//! 順序（TASK-310）は、この型を入力にして重複定義を生まない。
//!
//! 「スキーマ」は、serde の型（未知の欄を拒否）と、ここに書く JSON の形と、`tests/data/history_versions_1/`
//! の golden とのバイト一致で固定する。欄の順は `engine`・`ir_format`・`vocab`・`content`・`content_hash`・
//! `refs` である。全欄を必須とし、欠落・型の違い・未知の欄は逆直列化の失敗にする（失敗を拒否に倒す）。
//!
//! 版の文字列は逆直列化では `String` のまま受け、書式の検査は [`HistoryVersions::parse`] で行う。
//! 判定の 18 段の順序では「版の文字列が読めない」は型の検査より後に当たり、逆直列化で検査すると順序が崩れるため。
//! `vocab` の省略時の既定 [`VocabVersion::DEFAULT`] は書くときの値であり、読むときに欄の省略を許さない。
//!
//! 境界（本契約が定義しないもの）:
//! - 理由コードの名前（`missing_version` など）と順序は TASK-310。[`Verdict`] は理由コードの型を
//!   [`ReasonCode`] として外から受け取り、本契約の分類 [`ReasonKind`] との対応は TASK-310 が付ける
//! - 履歴の包み（`format`・`pack_id`・`start`・`ops`・`snapshot`）は TASK-309
//! - `content_hash` の求め方（本文の木を除いた IR の SHA-256、D-51）は [`crate::content_hash`]（TASK-180）。ここは書式だけを検査する
//! - 開く側の上限（`ir_format` の最大・語彙・エンジンの版）は判定（TASK-174）が持つ
//! - どの理由がどの [`Disposition`] になるかの線引き（D-52・D-59）と、理由ごとの文面（D-57 で未決）
//! - [`Verdict`] の直列化（外へ出す JSON の形が決まっていない）
//!
//! 変更する場合は、先に spec（`engine-versioning.md` の VER-1・VER-4〜VER-7）を直し、
//! [`HISTORY_VERSION_CONTRACT_VERSION`] を上げる。

use core::fmt;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// この契約の版。欄・書式・分類を変えたら上げる。
pub const HISTORY_VERSION_CONTRACT_VERSION: u32 = 1;

/// 履歴の `versions` の欄（ワイヤの形）。履歴は未信頼入力であり、値の意味は [`Self::parse`] で確かめる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HistoryVersions {
    /// エンジンの crate の版（`主.副.修`）。
    pub engine: String,
    /// IR の構造の版（IR の `format_version`）。
    pub ir_format: u32,
    /// 教材が宣言した語彙の版（`主.副`）。
    pub vocab: String,
    /// 教材の版（`pack.version`、`主.副.修`）。
    pub content: String,
    /// IR の SHA-256（小文字 16 進 64 文字。本文の木を除く。求め方は [`crate::content_hash`]）。
    pub content_hash: String,
    /// 参照元の名前から commit への表。空でも `{}` を書く。
    pub refs: BTreeMap<String, String>,
}

/// 版の文字列の検査に失敗した欄。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionField {
    /// `versions.engine`
    Engine,
    /// `versions.vocab`
    Vocab,
    /// `versions.content`
    Content,
    /// `versions.content_hash`
    ContentHash,
}

/// 版の文字列が読めないことを表す。利用者の値は埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionParseError {
    /// 失敗した欄。
    pub field: VersionField,
}

impl fmt::Display for VersionParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self.field {
            VersionField::Engine => "engine",
            VersionField::Vocab => "vocab",
            VersionField::Content => "content",
            VersionField::ContentHash => "content_hash",
        };
        write!(f, "invalid version string in versions.{name}")
    }
}

impl std::error::Error for VersionParseError {}

/// 正規の十進表記（ASCII 数字のみ・先頭の 0 なし・`u32` に収まる）だけを受ける。
fn parse_component(s: &str) -> Option<u32> {
    let b = s.as_bytes();
    if b.is_empty() || !b.iter().all(u8::is_ascii_digit) || (b.len() > 1 && b[0] == b'0') {
        return None;
    }
    s.parse().ok()
}

/// `主.副.修` の版。主→副→修の辞書順で比べる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version3 {
    /// 主版。
    pub major: u32,
    /// 副版。
    pub minor: u32,
    /// 修版。
    pub patch: u32,
}

impl Version3 {
    /// 正規の表記（`主.副.修`）だけを受ける。符号・空白・先頭の 0・成分数の過不足は `None`。
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split('.');
        let major = parse_component(it.next()?)?;
        let minor = parse_component(it.next()?)?;
        let patch = parse_component(it.next()?)?;
        if it.next().is_some() {
            return None;
        }
        Some(Self {
            major,
            minor,
            patch,
        })
    }

    /// 主版が等しいか（VER-6 の `content_major`・VER-7 の `engine_major` の比較の意味）。
    pub fn same_major(&self, other: &Self) -> bool {
        self.major == other.major
    }
}

impl fmt::Display for Version3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// `主.副` の語彙の版。主→副の辞書順で比べる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VocabVersion {
    /// 主版。
    pub major: u32,
    /// 副版。
    pub minor: u32,
}

impl VocabVersion {
    /// 教材が `pack.vocab` を省略したときに書く値（`1.0`）。
    pub const DEFAULT: Self = Self { major: 1, minor: 0 };

    /// 正規の表記（`主.副`）だけを受ける。
    pub fn parse(s: &str) -> Option<Self> {
        let mut it = s.split('.');
        let major = parse_component(it.next()?)?;
        let minor = parse_component(it.next()?)?;
        if it.next().is_some() {
            return None;
        }
        Some(Self { major, minor })
    }
}

impl fmt::Display for VocabVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// 小文字 16 進 64 文字であることを確かめた SHA-256 の表記。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// 長さ 64 かつ `[0-9a-f]` のみを受ける。大文字は拒否する（表記の違う同じ値を作らない）。
    pub fn parse(s: &str) -> Option<Self> {
        let ok = s.len() == 64 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'));
        ok.then(|| Self(s.to_owned()))
    }

    /// SHA-256 の 32 バイトから作る。`content_hash` モジュールだけが使う生成口で、`parse` と同じ不変条件を満たす。
    pub(crate) fn from_digest(digest: &[u8; 32]) -> Self {
        Self(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    /// 正規の表記。
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 書式を確かめた版の欄。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedVersions {
    /// エンジンの版。
    pub engine: Version3,
    /// 語彙の版。
    pub vocab: VocabVersion,
    /// 教材の版。
    pub content: Version3,
    /// IR のハッシュ。
    pub content_hash: ContentHash,
}

impl HistoryVersions {
    /// 版の文字列の書式を確かめる。欄の順（`engine`→`vocab`→`content`→`content_hash`）で、最初の失敗を返す。
    /// `ir_format` と `refs` は型で足りるため検査しない。
    pub fn parse(&self) -> Result<ParsedVersions, VersionParseError> {
        let err = |field| VersionParseError { field };
        Ok(ParsedVersions {
            engine: Version3::parse(&self.engine).ok_or(err(VersionField::Engine))?,
            vocab: VocabVersion::parse(&self.vocab).ok_or(err(VersionField::Vocab))?,
            content: Version3::parse(&self.content).ok_or(err(VersionField::Content))?,
            content_hash: ContentHash::parse(&self.content_hash)
                .ok_or(err(VersionField::ContentHash))?,
        })
    }

    /// 開く側の版との違いを、軸ごとの構造のデータで返す。同一なら空。
    ///
    /// 並びは [`VersionAxis::ALL`] の順で、`refs` は名前の昇順。履歴由来の値を含むため、
    /// 文面には整形しない（表示層が文脈に応じてエスケープする）。
    pub fn diff(&self, opening: &HistoryVersions) -> Vec<AxisDiff> {
        let mut out = Vec::new();
        let mut push = |axis, h: String, o: String| {
            if h != o {
                out.push(AxisDiff {
                    axis,
                    ref_name: None,
                    history: Some(h),
                    opening: Some(o),
                });
            }
        };
        push(
            VersionAxis::Engine,
            self.engine.clone(),
            opening.engine.clone(),
        );
        push(
            VersionAxis::IrFormat,
            self.ir_format.to_string(),
            opening.ir_format.to_string(),
        );
        push(
            VersionAxis::Vocab,
            self.vocab.clone(),
            opening.vocab.clone(),
        );
        push(
            VersionAxis::Content,
            self.content.clone(),
            opening.content.clone(),
        );
        push(
            VersionAxis::ContentHash,
            self.content_hash.clone(),
            opening.content_hash.clone(),
        );
        let mut names: Vec<&String> = self.refs.keys().chain(opening.refs.keys()).collect();
        names.sort();
        names.dedup();
        for name in names {
            let (h, o) = (self.refs.get(name), opening.refs.get(name));
            if h != o {
                out.push(AxisDiff {
                    axis: VersionAxis::Refs,
                    ref_name: Some(name.clone()),
                    history: h.cloned(),
                    opening: o.cloned(),
                });
            }
        }
        out
    }
}

/// 版の軸。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VersionAxis {
    /// エンジンの版。
    Engine,
    /// IR の構造の版。
    IrFormat,
    /// 語彙の版。
    Vocab,
    /// 教材の版。
    Content,
    /// IR のハッシュ。
    ContentHash,
    /// 参照元の commit。
    Refs,
}

impl VersionAxis {
    /// 全軸。[`HistoryVersions::diff`] の並びの順。
    pub const ALL: [VersionAxis; 6] = [
        VersionAxis::Engine,
        VersionAxis::IrFormat,
        VersionAxis::Vocab,
        VersionAxis::Content,
        VersionAxis::ContentHash,
        VersionAxis::Refs,
    ];
}

/// 1 つの軸の版の違い。`ref_name` は `Refs` のときだけ `Some`。参照元の追加・削除は片側が `None`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AxisDiff {
    /// 違いのある軸。
    pub axis: VersionAxis,
    /// 参照元の名前（`Refs` のみ）。
    pub ref_name: Option<String>,
    /// 履歴の側の値。
    pub history: Option<String>,
    /// 開く側の値。
    pub opening: Option<String>,
}

/// 「変換して開く」で記録する版の違い。空にできない（理由の無い変換を作れない。VER-4）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversionNotes(Vec<AxisDiff>);

impl ConversionNotes {
    /// 空なら `None`。
    pub fn new(diffs: Vec<AxisDiff>) -> Option<Self> {
        (!diffs.is_empty()).then_some(Self(diffs))
    }

    /// 違いの一覧（1 件以上）。
    pub fn diffs(&self) -> &[AxisDiff] {
        &self.0
    }
}

/// 判定結果の種類（VER-4〜VER-7）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Disposition {
    /// 変換なしで開く。
    Open,
    /// 変換して開く。
    Converted,
    /// 表示だけ。
    ViewOnly,
    /// 開かない。
    Refused,
}

impl Disposition {
    /// 全種。
    pub const ALL: [Disposition; 4] = [
        Disposition::Open,
        Disposition::Converted,
        Disposition::ViewOnly,
        Disposition::Refused,
    ];

    /// 結果の種類の名前（理由コードではない）。
    pub fn as_str(&self) -> &'static str {
        match self {
            Disposition::Open => "open",
            Disposition::Converted => "converted",
            Disposition::ViewOnly => "view_only",
            Disposition::Refused => "refused",
        }
    }
}

/// 理由の種類（コード名ではない分類）。どの軸で、どの向きかを表す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ReasonKind {
    /// 版の欄が無い（VER-1）。
    MissingVersion,
    /// 版の表記を読めない。
    UnreadableVersion,
    /// 履歴の側が新しい（VER-7）。
    HistoryNewer(VersionAxis),
    /// 履歴の側が古く、変換の規則が無い（VER-7・VER-16）。
    HistoryOlder(VersionAxis),
    /// 主版が違う（VER-6・VER-7）。
    MajorDiffers(VersionAxis),
    /// 適用し直しても写しと一致しない（VER-4・VER-5）。
    ReplayDiverged,
}

/// 理由コードの型が実装する。名前と順序は TASK-310 が持つ。
pub trait ReasonCode {
    /// 理由コードの文字列。空であってはならない（VER-2。検査は実装側の適合試験が行う）。
    fn as_str(&self) -> &'static str;
    /// 版に関わる理由の分類。大きさの超過など版に関わらない理由は `None`。
    fn kind(&self) -> Option<ReasonKind>;
}

/// 開く可否の判定結果。`C` は TASK-310 の理由コードの型。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict<C: ReasonCode> {
    /// 変換なしで開く。
    Open,
    /// 変換して開く。版の違いを記録する。
    Converted {
        /// 版の違い（1 件以上）。
        notes: ConversionNotes,
    },
    /// 表示だけ。
    ViewOnly {
        /// 理由。
        code: C,
    },
    /// 開かない。
    Refused {
        /// 理由。
        code: C,
    },
}

impl<C: ReasonCode> Verdict<C> {
    /// 結果の種類。
    pub fn disposition(&self) -> Disposition {
        match self {
            Verdict::Open => Disposition::Open,
            Verdict::Converted { .. } => Disposition::Converted,
            Verdict::ViewOnly { .. } => Disposition::ViewOnly,
            Verdict::Refused { .. } => Disposition::Refused,
        }
    }

    /// 理由コード。表示だけ・開かないのときに `Some`。
    pub fn code(&self) -> Option<&C> {
        match self {
            Verdict::ViewOnly { code } | Verdict::Refused { code } => Some(code),
            _ => None,
        }
    }
}
