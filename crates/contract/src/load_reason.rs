//! 履歴の読み込みの拒否理由コードの一覧と判定の順序（TASK-310 2/3・3/3、保存先の契約の版 1 の範囲）。
//!
//! 履歴の読み込みで「開かない」「表示だけ」になる理由の名前を、この 1 か所だけに定義する。
//! 呼び出し元は、読み込みの段を実装する core（TASK-331）、版の判定（TASK-146・TASK-155・TASK-169・
//! TASK-174）、理由を受講者へ見せる web（TASK-532）である。契約は contract に置き、core と web は
//! ここの [`LoadReason`] だけを参照して、同じ名前を別の列挙に重ねて定義しない。
//!
//! 版は新しく作らず、[`crate::storage::STORAGE_CONTRACT_VERSION`]（版 1）の範囲で固定する。
//! variant・文字列を足す・変える場合は、先に spec を直し、その版を上げる。
//!
//! 上限に関わる 3 種（[`LimitReason`]）の文字列は [`LimitReason::as_str`] に委ね、再定義しない。
//! [`LoadReason`] は 1 段で網羅 match できるよう、入れ子にせず 19 個を平らに並べる。
//!
//! 出所と決定との対応:
//!
//! | 理由コード | 出所 |
//! |---|---|
//! | `too_large`・`too_deep`・`truncated`・`not_json`・`missing_version`・`malformed`・`other_pack`・`replay_mismatch` | SAVE-19・SAVE-20 |
//! | `content_major`・`vocab_newer`・`format_newer`・`engine_newer`・`format_older`・`engine_major` | VER-6・VER-7（VER-16・VER-18・D-59） |
//! | `replay_diverged` | VER-4・VER-5 |
//! | `too_many_ops` | 上限の契約（[`crate::limits`]） |
//!
//! 【推論】次の 4 種は Issue と spec の列挙に名前が無く、試作の `docs/spec/prototype/engine/crates/core/src/history.rs` の
//! `Code`（判定の 18 段）に合わせて足した。後から足すと版上げが要り、TASK-174 などはここにあるコードしか参照できないため。
//! - `too_many_ops`（上限の契約の履歴の拒否理由）
//! - `history_format_newer`（履歴の包みの `format` が新しい。型の段の後に別に判定する）
//! - `bad_version`（版の文字列が読めない。[`ReasonKind::UnreadableVersion`] に対応）
//! - `start_missing`（履歴の起点が教材に無い）
//!
//! 判定の順序（TASK-310 3/3）:
//!
//! 順序の正は [`HISTORY_LOAD_ORDER`]（上から判定し、最初に当たった理由で結果が決まる）である。
//! 各理由が属する段は [`LoadReason::stage`]、段の並びは [`HistoryLoadStage::ALL`] が表す。
//! 呼び出し元は、読み込みの経路を実装する core（TASK-331）と、版の違う履歴の判定（TASK-174）である。
//! どちらもこの順序に従い、別の並びを作らない。どの段の拒否も副作用を持たない（SAVE-19）。
//!
//! | 段 | 段の名前 | 理由コード |
//! |---|---|---|
//! | 1 | [`HistoryLoadStage::Size`] | `too_large` |
//! | 2 | [`HistoryLoadStage::Depth`] | `too_deep` |
//! | 3 | [`HistoryLoadStage::Json`] | `truncated`・`not_json` |
//! | 4 | [`HistoryLoadStage::VersionField`] | `missing_version` |
//! | 5 | [`HistoryLoadStage::Type`] | `malformed`・`too_many_ops`・`history_format_newer`・`other_pack` |
//! | 6 | [`HistoryLoadStage::VersionJudgment`] | `bad_version`・`format_newer`・`vocab_newer`・`engine_newer`・`format_older`・`engine_major`・`content_major` |
//! | 7 | [`HistoryLoadStage::Replay`] | `start_missing` |
//! | 8 | [`HistoryLoadStage::SnapshotMatch`] | `replay_mismatch`・`replay_diverged` |
//!
//! 【推論】次の位置は SAVE-19 に記録が無く、試作の判定順（`open_history`）に合わせて仮に置いた。
//! - `truncated` は Json 段に置く。SAVE-19 は位置を未確認とし、TASK-331 が実装で確かめて記録する。
//!   結果が違えば [`crate::storage::STORAGE_CONTRACT_VERSION`] を上げて直す
//! - `history_format_newer`・`other_pack` は Type 段に置く。どちらも 4 軸の版の理由ではなく（[`ReasonCode::kind`] が `None`）、
//!   試作では `malformed`・`too_many_ops` の後、`bad_version` の前にある
//! - `too_many_ops` は Type 段に置く。上限の契約（[`LimitReason::stage`]）の「型の後・適用し直しの前」に合わせる
//! - `start_missing` は Replay 段に置く。試作は、起点が教材に無いことを適用し直しの失敗として判定する
//!
//! 同じ段の中で条件が排他な組（`truncated` と `not_json`、`replay_mismatch` と `replay_diverged`）は、
//! 並びの前後に意味が無い。順序を変える場合は、先に spec を直し、保存先の契約の版を上げる。
//!
//! 定義しないもの:
//! - どの理由が表示だけ・開かないになるかの線引き（[`crate::history_version::Disposition`]。TASK-174）
//! - 受講者向けの文面（D-57・SHELL-36 が未決。TASK-532）
//! - `truncated` の意味の食い違い（SAVE-19 は UTF-8 の途中切れ、試作は JSON が入力の終わりで途切れたもの）。
//!   位置は上の【推論】のとおり仮置きで、TASK-331 が確かめる
//! - core の `LoadError`・`MissingVersion` からの対応づけ（contract は core に依存できない。TASK-331・TASK-174）
//!
//! 理由コードは固定の `&'static str` だけで、読み込んだ内容を含めない（SAFE-40）。

use crate::history_version::{ReasonCode, ReasonKind, VersionAxis};
use crate::limits::{LimitReason, LoadStage};
use core::fmt;

/// 履歴の読み込みの判定の段（SAVE-19 の 8 段）。小さい順に判定し、どの段の拒否も副作用を持たない。
///
/// IR と履歴に共通の粗い 4 段（[`LoadStage`]）は [`HistoryLoadStage::coarse`] で得る。
/// 呼び出し元は core の読み込みの経路（TASK-331）と版の判定（TASK-174）である。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HistoryLoadStage {
    /// 大きさ。
    Size,
    /// 入れ子の深さ。
    Depth,
    /// JSON の解釈。
    Json,
    /// 版の欄の有無。
    VersionField,
    /// 型・内容の検査。
    Type,
    /// 版の判定（VER-2〜VER-7）。
    VersionJudgment,
    /// 適用し直し。
    Replay,
    /// 写しとの照合。
    SnapshotMatch,
}

impl HistoryLoadStage {
    /// すべての段。判定の順。
    pub const ALL: [HistoryLoadStage; 8] = [
        HistoryLoadStage::Size,
        HistoryLoadStage::Depth,
        HistoryLoadStage::Json,
        HistoryLoadStage::VersionField,
        HistoryLoadStage::Type,
        HistoryLoadStage::VersionJudgment,
        HistoryLoadStage::Replay,
        HistoryLoadStage::SnapshotMatch,
    ];

    /// SAFE-20 の粗い 4 段への対応。大きさ・深さ・JSON の後は、すべて型・内容の段に含める。
    pub const fn coarse(self) -> LoadStage {
        match self {
            HistoryLoadStage::Size => LoadStage::Size,
            HistoryLoadStage::Depth => LoadStage::Depth,
            HistoryLoadStage::Json => LoadStage::JsonParse,
            HistoryLoadStage::VersionField
            | HistoryLoadStage::Type
            | HistoryLoadStage::VersionJudgment
            | HistoryLoadStage::Replay
            | HistoryLoadStage::SnapshotMatch => LoadStage::TypeAndContent,
        }
    }
}

/// 判定の順序の契約の正。上から判定し、最初に当たった理由で結果が決まる。
///
/// 同じ段の排他な組（`truncated`・`not_json`、`replay_mismatch`・`replay_diverged`）は条件で分かれ、
/// 並びの前後に意味が無い。変えるときは保存先の契約の版（[`crate::storage::STORAGE_CONTRACT_VERSION`]）を上げる。
pub const HISTORY_LOAD_ORDER: [LoadReason; 19] = [
    LoadReason::TooLarge,           // 1 大きさ
    LoadReason::TooDeep,            // 2 入れ子の深さ
    LoadReason::Truncated,          // 3 JSON の解釈（仮置き）
    LoadReason::NotJson,            // 3 JSON の解釈
    LoadReason::MissingVersion,     // 4 版の欄
    LoadReason::Malformed,          // 5 型
    LoadReason::TooManyOps,         // 5 型（件数）
    LoadReason::HistoryFormatNewer, // 5 型（包みの形式）
    LoadReason::OtherPack,          // 5 型（別の教材）
    LoadReason::BadVersion,         // 6 版の判定（読めない版）
    LoadReason::FormatNewer,        // 6 版の判定（構造が新しい）
    LoadReason::VocabNewer,         // 6 版の判定（語彙が新しい）
    LoadReason::EngineNewer,        // 6 版の判定（エンジンが新しい）
    LoadReason::FormatOlder,        // 6 版の判定（構造が古い）
    LoadReason::EngineMajor,        // 6 版の判定（エンジンの主版）
    LoadReason::ContentMajor,       // 6 版の判定（教材の主版）
    LoadReason::StartMissing,       // 7 適用し直し
    LoadReason::ReplayMismatch,     // 8 写しとの照合（壊れた履歴）
    LoadReason::ReplayDiverged,     // 8 写しとの照合（版の違い）
];

/// 読み込みの拒否理由コード（19 種）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LoadReason {
    /// 大きさの上限を超えた（[`LimitReason::TooLarge`]）。
    TooLarge,
    /// 入れ子の上限を超えた（[`LimitReason::TooDeep`]）。
    TooDeep,
    /// 履歴の入力の件数の上限を超えた（[`LimitReason::TooManyOps`]）。
    TooManyOps,
    /// 入力が途中で切れている。
    Truncated,
    /// JSON として読めない。
    NotJson,
    /// 版の欄が無い。
    MissingVersion,
    /// 履歴の形が合わない。
    Malformed,
    /// 履歴の包みの `format` が新しい。
    HistoryFormatNewer,
    /// 別の教材の履歴である。
    OtherPack,
    /// 版の文字列が読めない。
    BadVersion,
    /// IR の構造の版が履歴のほうが新しい。
    FormatNewer,
    /// 語彙の版が履歴のほうが新しい。
    VocabNewer,
    /// エンジンの版が履歴のほうが新しい。
    EngineNewer,
    /// IR の構造の版が履歴のほうが古く、変換の規則が無い。
    FormatOlder,
    /// エンジンの主版が違う。
    EngineMajor,
    /// 教材の主版が違う。
    ContentMajor,
    /// 履歴の起点が教材に無い。
    StartMissing,
    /// 適用し直した結果が履歴の写しと食い違う（壊れた履歴）。
    ReplayMismatch,
    /// 適用し直しても写しと一致しない（版の違いによる）。
    ReplayDiverged,
}

impl LoadReason {
    /// すべての理由。並びは試作の `Code` の宣言順で、判定の順序の契約ではない（判定の順序は [`HISTORY_LOAD_ORDER`]）。
    pub const ALL: [LoadReason; 19] = [
        LoadReason::TooLarge,
        LoadReason::TooDeep,
        LoadReason::TooManyOps,
        LoadReason::Truncated,
        LoadReason::NotJson,
        LoadReason::MissingVersion,
        LoadReason::Malformed,
        LoadReason::HistoryFormatNewer,
        LoadReason::OtherPack,
        LoadReason::BadVersion,
        LoadReason::FormatNewer,
        LoadReason::VocabNewer,
        LoadReason::EngineNewer,
        LoadReason::FormatOlder,
        LoadReason::EngineMajor,
        LoadReason::ContentMajor,
        LoadReason::StartMissing,
        LoadReason::ReplayMismatch,
        LoadReason::ReplayDiverged,
    ];

    /// 理由コードの文字列。上限の 3 種は [`LimitReason`] に委ねる。
    pub const fn as_str(self) -> &'static str {
        match self {
            LoadReason::TooLarge => LimitReason::TooLarge.as_str(),
            LoadReason::TooDeep => LimitReason::TooDeep.as_str(),
            LoadReason::TooManyOps => LimitReason::TooManyOps.as_str(),
            LoadReason::Truncated => "truncated",
            LoadReason::NotJson => "not_json",
            LoadReason::MissingVersion => "missing_version",
            LoadReason::Malformed => "malformed",
            LoadReason::HistoryFormatNewer => "history_format_newer",
            LoadReason::OtherPack => "other_pack",
            LoadReason::BadVersion => "bad_version",
            LoadReason::FormatNewer => "format_newer",
            LoadReason::VocabNewer => "vocab_newer",
            LoadReason::EngineNewer => "engine_newer",
            LoadReason::FormatOlder => "format_older",
            LoadReason::EngineMajor => "engine_major",
            LoadReason::ContentMajor => "content_major",
            LoadReason::StartMissing => "start_missing",
            LoadReason::ReplayMismatch => "replay_mismatch",
            LoadReason::ReplayDiverged => "replay_diverged",
        }
    }

    /// 正規の文字列だけを受ける。大文字・空白つき・未知の文字列は `None`（既定値へ倒さない）。
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str() == s)
    }

    /// この理由を判定する段。判定の順序は [`HISTORY_LOAD_ORDER`] が正で、段は常にその順に沿って減らない。
    pub const fn stage(self) -> HistoryLoadStage {
        match self {
            LoadReason::TooLarge => HistoryLoadStage::Size,
            LoadReason::TooDeep => HistoryLoadStage::Depth,
            LoadReason::Truncated => HistoryLoadStage::Json,
            LoadReason::NotJson => HistoryLoadStage::Json,
            LoadReason::MissingVersion => HistoryLoadStage::VersionField,
            LoadReason::Malformed => HistoryLoadStage::Type,
            LoadReason::TooManyOps => HistoryLoadStage::Type,
            LoadReason::HistoryFormatNewer => HistoryLoadStage::Type,
            LoadReason::OtherPack => HistoryLoadStage::Type,
            LoadReason::BadVersion => HistoryLoadStage::VersionJudgment,
            LoadReason::FormatNewer => HistoryLoadStage::VersionJudgment,
            LoadReason::VocabNewer => HistoryLoadStage::VersionJudgment,
            LoadReason::EngineNewer => HistoryLoadStage::VersionJudgment,
            LoadReason::FormatOlder => HistoryLoadStage::VersionJudgment,
            LoadReason::EngineMajor => HistoryLoadStage::VersionJudgment,
            LoadReason::ContentMajor => HistoryLoadStage::VersionJudgment,
            LoadReason::StartMissing => HistoryLoadStage::Replay,
            LoadReason::ReplayMismatch => HistoryLoadStage::SnapshotMatch,
            LoadReason::ReplayDiverged => HistoryLoadStage::SnapshotMatch,
        }
    }

    /// 上限に関わる理由なら、対応する [`LimitReason`]。
    pub const fn limit(self) -> Option<LimitReason> {
        match self {
            LoadReason::TooLarge => Some(LimitReason::TooLarge),
            LoadReason::TooDeep => Some(LimitReason::TooDeep),
            LoadReason::TooManyOps => Some(LimitReason::TooManyOps),
            _ => None,
        }
    }
}

impl From<LimitReason> for LoadReason {
    fn from(r: LimitReason) -> Self {
        match r {
            LimitReason::TooLarge => LoadReason::TooLarge,
            LimitReason::TooDeep => LoadReason::TooDeep,
            LimitReason::TooManyOps => LoadReason::TooManyOps,
        }
    }
}

impl fmt::Display for LoadReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl ReasonCode for LoadReason {
    fn as_str(&self) -> &'static str {
        LoadReason::as_str(*self)
    }

    /// 版に関わる 9 種だけが `Some`。`history_format_newer` は [`VersionAxis`] に履歴の形式の軸が無いため `None`。
    fn kind(&self) -> Option<ReasonKind> {
        match self {
            LoadReason::MissingVersion => Some(ReasonKind::MissingVersion),
            LoadReason::BadVersion => Some(ReasonKind::UnreadableVersion),
            LoadReason::FormatNewer => Some(ReasonKind::HistoryNewer(VersionAxis::IrFormat)),
            LoadReason::VocabNewer => Some(ReasonKind::HistoryNewer(VersionAxis::Vocab)),
            LoadReason::EngineNewer => Some(ReasonKind::HistoryNewer(VersionAxis::Engine)),
            LoadReason::FormatOlder => Some(ReasonKind::HistoryOlder(VersionAxis::IrFormat)),
            LoadReason::EngineMajor => Some(ReasonKind::MajorDiffers(VersionAxis::Engine)),
            LoadReason::ContentMajor => Some(ReasonKind::MajorDiffers(VersionAxis::Content)),
            LoadReason::ReplayDiverged => Some(ReasonKind::ReplayDiverged),
            _ => None,
        }
    }
}
