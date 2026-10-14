//! 未信頼入力の上限値と拒否理由の契約（TASK-199、版 1）。
//!
//! 教材パック・IR・履歴・入力欄は未信頼入力であり、読み込み（core）・保存と復元・作成者向け CLI・入力欄
//! （TERM-15・JUDGE-23）の 4 か所が、同じ値・同じ理由コード・同じ判定の順序に従う必要がある。
//! そのため定数と型をこの 1 か所に置く。呼び出し元は core・pack・web で、判定の関数は持たない
//! （判定は TASK-154・155・157 が core に実装する）。判定の順序はデータ（[`LOAD_STAGE_ORDER`]）で表す。
//!
//! 値の出所は spec の D-44・D-45・D-25 と、`engine-safety.md` の SAFE-17〜23・25・41〜43・45 である。
//! 値・理由コード・順序・文面を変える場合は、先に spec を直し、[`LIMITS_CONTRACT_VERSION`] を上げる。
//!
//! 整数の型の規約: バイト数は `u64`（pack がメタデータの `u64` で数え、wasm32 の `usize` は 32 bit のため）、
//! 件数・段数・文字数・回数・要素数は `usize`。呼ぶ側は `bytes.len() as u64`（拡大で値を失わない）で比べる。
//!
//! 履歴の拒否理由コード全体の一覧は TASK-310 の範囲で、本契約は上限に関わる 3 種（[`LimitReason`]）だけを
//! 定義する。TASK-310 は自分の列挙に埋め込むか `From` で変換し、`too_large`・`too_deep` を再定義しない。
//!
//! 意図して固定しないもの（決める前に固定しない）:
//! - SAFE-25（要素数・文字列長の超過）の理由コードと文面（TASK-162 で決めて版上げする）
//! - SAFE-34（遷移の上限）の見出し（TASK-160 で扱う）
//! - UTF-8 の検査（`truncated`・`not_utf8`）の判定順の位置（記録が無く未確認）
//! - 素の `E-LIMIT`（`.md` 以外の本文ファイルの超過。AUTHOR-11 が未解決で、診断コードの一覧は TASK-195 の契約）
//!   値は [`MAX_MARKDOWN_BYTES`] を共有して使う

use core::fmt;
use serde::{Deserialize, Serialize};

/// この契約の版。値・理由コード・順序・文面を変えたら上げる。
pub const LIMITS_CONTRACT_VERSION: u32 = 1;

/// IR 1 ファイルの上限（バイト）。ちょうどは通し、1 超過で拒否する（SAFE-17・SAFE-45、D-44）。
pub const MAX_IR_BYTES: u64 = 20_971_520;
/// 履歴ファイルの上限（バイト）。ちょうどは通す（SAFE-18、計画の閾値 D-25）。
pub const MAX_HISTORY_BYTES: u64 = 10_485_760;
/// 履歴の入力（`ops`）の件数の上限。20,000 件ちょうどは通す（SAFE-19・SAFE-41、D-44）。
pub const MAX_HISTORY_OPS: usize = 20_000;
/// JSON の入れ子の上限（段）。64 段ちょうどは通し、65 段以上は拒否する（SAFE-20・SAFE-42、D-44）。
pub const MAX_JSON_DEPTH: usize = 64;
/// 入力欄 1 回の上限。コードポイント（`chars().count()`）で数える。UTF-16 の単位ではない（SAFE-21、D-25）。
pub const MAX_INPUT_CODE_POINTS: usize = 20_000;
/// 教材パック全体の上限（バイト）（SAFE-22、D-25）。
pub const MAX_PACK_BYTES: u64 = 20_971_520;
/// Markdown 1 ファイルの上限（バイト）。素の `E-LIMIT` の本文ファイルも共有する（SAFE-22、D-25）。
pub const MAX_MARKDOWN_BYTES: u64 = 524_288;
/// 教材パックのディレクトリの入れ子の上限（段）。根を 0 段とし、32 段は通し、33 段は拒否する（SAFE-43、D-44）。
pub const MAX_PACK_DIR_DEPTH: usize = 32;
/// 1 回の入力で起こせる遷移の上限（回）。1,000 回ちょうどは通し、1,001 回目で止める（SAFE-23、D-25）。
pub const MAX_TRANSITIONS_PER_INPUT: usize = 1_000;
/// 1 つのコレクションの要素数の上限（SAFE-25、D-45）。
pub const MAX_COLLECTION_ELEMENTS: usize = 100_000;
/// 1 つの文字列の長さの上限。コードポイントで数える（SAFE-25・SAFE-26、D-45）。
///
/// 【推論】D-45 は単位を書いていない。spec が単位を定めている SAFE-21 に合わせた。
pub const MAX_STRING_CHARS: usize = 100_000;

/// 上限の単位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitUnit {
    /// バイト数。
    Bytes,
    /// 入力（`ops`）の件数。
    Ops,
    /// JSON の入れ子の段数。
    JsonLevels,
    /// コードポイント数。
    CodePoints,
    /// ディレクトリの段数。
    DirLevels,
    /// 遷移の回数。
    Transitions,
    /// 要素数。
    Elements,
    /// 文字列の文字数（コードポイント）。
    Chars,
}

/// 上限の種類（11 種）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LimitId {
    /// [`MAX_IR_BYTES`]。
    IrBytes,
    /// [`MAX_HISTORY_BYTES`]。
    HistoryBytes,
    /// [`MAX_HISTORY_OPS`]。
    HistoryOps,
    /// [`MAX_JSON_DEPTH`]。
    JsonDepth,
    /// [`MAX_INPUT_CODE_POINTS`]。
    InputCodePoints,
    /// [`MAX_PACK_BYTES`]。
    PackBytes,
    /// [`MAX_MARKDOWN_BYTES`]。
    MarkdownBytes,
    /// [`MAX_PACK_DIR_DEPTH`]。
    PackDirDepth,
    /// [`MAX_TRANSITIONS_PER_INPUT`]。
    TransitionsPerInput,
    /// [`MAX_COLLECTION_ELEMENTS`]。
    CollectionElements,
    /// [`MAX_STRING_CHARS`]。
    StringChars,
}

impl LimitId {
    /// すべての上限。
    pub const ALL: [LimitId; 11] = [
        LimitId::IrBytes,
        LimitId::HistoryBytes,
        LimitId::HistoryOps,
        LimitId::JsonDepth,
        LimitId::InputCodePoints,
        LimitId::PackBytes,
        LimitId::MarkdownBytes,
        LimitId::PackDirDepth,
        LimitId::TransitionsPerInput,
        LimitId::CollectionElements,
        LimitId::StringChars,
    ];

    /// 上限の値（単位は [`LimitId::unit`]）。
    pub const fn value(self) -> u64 {
        match self {
            LimitId::IrBytes => MAX_IR_BYTES,
            LimitId::HistoryBytes => MAX_HISTORY_BYTES,
            LimitId::HistoryOps => MAX_HISTORY_OPS as u64,
            LimitId::JsonDepth => MAX_JSON_DEPTH as u64,
            LimitId::InputCodePoints => MAX_INPUT_CODE_POINTS as u64,
            LimitId::PackBytes => MAX_PACK_BYTES,
            LimitId::MarkdownBytes => MAX_MARKDOWN_BYTES,
            LimitId::PackDirDepth => MAX_PACK_DIR_DEPTH as u64,
            LimitId::TransitionsPerInput => MAX_TRANSITIONS_PER_INPUT as u64,
            LimitId::CollectionElements => MAX_COLLECTION_ELEMENTS as u64,
            LimitId::StringChars => MAX_STRING_CHARS as u64,
        }
    }

    /// 上限の単位。
    pub const fn unit(self) -> LimitUnit {
        match self {
            LimitId::IrBytes
            | LimitId::HistoryBytes
            | LimitId::PackBytes
            | LimitId::MarkdownBytes => LimitUnit::Bytes,
            LimitId::HistoryOps => LimitUnit::Ops,
            LimitId::JsonDepth => LimitUnit::JsonLevels,
            LimitId::InputCodePoints => LimitUnit::CodePoints,
            LimitId::PackDirDepth => LimitUnit::DirLevels,
            LimitId::TransitionsPerInput => LimitUnit::Transitions,
            LimitId::CollectionElements => LimitUnit::Elements,
            LimitId::StringChars => LimitUnit::Chars,
        }
    }
}

/// IR・履歴を読み込むときの判定の段。SAFE-20 が正で、小さい順に判定する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LoadStage {
    /// 大きさ（バイト数）。
    Size,
    /// 入れ子の深さ。
    Depth,
    /// JSON の解釈。
    JsonParse,
    /// 型・内容の検査。
    TypeAndContent,
}

/// 判定の順序。大きさ、入れ子の深さ、JSON の解釈、型・内容の順。
pub const LOAD_STAGE_ORDER: [LoadStage; 4] = [
    LoadStage::Size,
    LoadStage::Depth,
    LoadStage::JsonParse,
    LoadStage::TypeAndContent,
];

/// 読み込みの入口。文面を入口ごとに分けるために使う。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadEntry {
    /// IR の読み込み。
    Ir,
    /// 履歴の読み込み。
    History,
}

/// 実行時の拒否理由（上限に関わるもの）。直列化した値は理由コードの文字列になる。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitReason {
    /// 大きさの上限を超えた。
    TooLarge,
    /// 入れ子の上限を超えた。
    TooDeep,
    /// 履歴の入力の件数の上限を超えた。
    TooManyOps,
}

impl LimitReason {
    /// すべての理由。
    pub const ALL: [LimitReason; 3] = [
        LimitReason::TooLarge,
        LimitReason::TooDeep,
        LimitReason::TooManyOps,
    ];

    /// 理由コードの文字列。
    pub const fn as_str(self) -> &'static str {
        match self {
            LimitReason::TooLarge => "too_large",
            LimitReason::TooDeep => "too_deep",
            LimitReason::TooManyOps => "too_many_ops",
        }
    }

    /// この理由を判定する段。件数は、型の検査の後・適用し直しの前に判定する。
    pub const fn stage(self) -> LoadStage {
        match self {
            LimitReason::TooLarge => LoadStage::Size,
            LimitReason::TooDeep => LoadStage::Depth,
            LimitReason::TooManyOps => LoadStage::TypeAndContent,
        }
    }
}

impl fmt::Display for LimitReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 作成者向け CLI の拒否理由コード（`E-LIMIT-*`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PackLimitCode {
    /// 教材パック全体が大きすぎる。
    #[serde(rename = "E-LIMIT-PACK-SIZE")]
    PackSize,
    /// Markdown 1 ファイルが大きすぎる。
    #[serde(rename = "E-LIMIT-MD-SIZE")]
    MdSize,
    /// ディレクトリの入れ子が深すぎる。
    #[serde(rename = "E-LIMIT-DEPTH")]
    DirDepth,
    /// シンボリックリンクがある（辿らず、中身を読む前に拒否する）。
    #[serde(rename = "E-LIMIT-SYMLINK")]
    Symlink,
}

impl PackLimitCode {
    /// すべてのコード。
    pub const ALL: [PackLimitCode; 4] = [
        PackLimitCode::PackSize,
        PackLimitCode::MdSize,
        PackLimitCode::DirDepth,
        PackLimitCode::Symlink,
    ];

    /// 理由コードの文字列。
    pub const fn as_str(self) -> &'static str {
        match self {
            PackLimitCode::PackSize => "E-LIMIT-PACK-SIZE",
            PackLimitCode::MdSize => "E-LIMIT-MD-SIZE",
            PackLimitCode::DirDepth => "E-LIMIT-DEPTH",
            PackLimitCode::Symlink => "E-LIMIT-SYMLINK",
        }
    }

    /// 対応する上限。シンボリックリンクには数値の上限が無い。
    pub const fn limit(self) -> Option<LimitId> {
        match self {
            PackLimitCode::PackSize => Some(LimitId::PackBytes),
            PackLimitCode::MdSize => Some(LimitId::MarkdownBytes),
            PackLimitCode::DirDepth => Some(LimitId::PackDirDepth),
            PackLimitCode::Symlink => None,
        }
    }

    /// 固定の文面。測った値（バイト数）は含まない。
    pub const fn message(self) -> &'static str {
        match self {
            PackLimitCode::PackSize => {
                "教材パック全体が 20MB の上限を超えています。教材を分けてください"
            }
            PackLimitCode::MdSize => {
                "Markdown が 512KB の上限を超えています。ファイルを分けてください"
            }
            PackLimitCode::DirDepth => "ディレクトリの入れ子が 32 段を超えています",
            PackLimitCode::Symlink => {
                "シンボリックリンクは使えません。実体のファイルを置いてください"
            }
        }
    }
}

impl fmt::Display for PackLimitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// 入力欄の上限を超えたときの文面（SAFE-21。TERM-15・JUDGE-23 が従う）。
pub const INPUT_TOO_LONG_MESSAGE: &str =
    "入力は 20,000 文字までです。超えた分は受け付けていません。";

/// 受講者向けの固定の文面。入口に該当しない組（IR の `TooManyOps`）は `None`。
///
/// 文面は表示用で、状態・イベントの決定には使わない。利用者の値は埋め込まない。
pub const fn rejection_message(entry: LoadEntry, reason: LimitReason) -> Option<&'static str> {
    match (entry, reason) {
        (LoadEntry::Ir, LimitReason::TooLarge) => {
            Some("ファイルが上限の大きさを超えています。別のファイルを選んでください。")
        }
        (LoadEntry::Ir, LimitReason::TooDeep) => {
            Some("ファイルの入れ子が深すぎます（64 段まで）。別のファイルを選んでください。")
        }
        (LoadEntry::Ir, LimitReason::TooManyOps) => None,
        (LoadEntry::History, LimitReason::TooLarge) => {
            Some("履歴ファイルが 10MB の上限を超えています。別のファイルを選んでください。")
        }
        (LoadEntry::History, LimitReason::TooDeep) => {
            Some("履歴ファイルの入れ子が深すぎます（64 段まで）。別のファイルを選んでください。")
        }
        (LoadEntry::History, LimitReason::TooManyOps) => {
            Some("履歴の入力の数が上限（20,000 件）を超えています。別のファイルを選んでください。")
        }
    }
}

/// 境界値の試験用サンプル。ちょうど・1 超過・10 倍。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoundarySample {
    /// 対象の上限。
    pub limit: LimitId,
    /// 上限ちょうど（通す）。
    pub exact: u64,
    /// 上限の 1 超過（拒否する）。
    pub over_by_one: u64,
    /// 上限の 10 倍（拒否する）。
    pub tenfold: u64,
}

const fn sample(limit: LimitId) -> BoundarySample {
    let exact = limit.value();
    BoundarySample {
        limit,
        exact,
        over_by_one: exact + 1,
        tenfold: exact * 10,
    }
}

/// 全上限の境界サンプル（[`LimitId::ALL`] と同じ順）。
pub const BOUNDARY_SAMPLES: [BoundarySample; 11] = [
    sample(LimitId::IrBytes),
    sample(LimitId::HistoryBytes),
    sample(LimitId::HistoryOps),
    sample(LimitId::JsonDepth),
    sample(LimitId::InputCodePoints),
    sample(LimitId::PackBytes),
    sample(LimitId::MarkdownBytes),
    sample(LimitId::PackDirDepth),
    sample(LimitId::TransitionsPerInput),
    sample(LimitId::CollectionElements),
    sample(LimitId::StringChars),
];
