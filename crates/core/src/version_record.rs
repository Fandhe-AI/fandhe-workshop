//! 履歴の 4 軸の版を書く処理と、版の欄の欠落の検査（TASK-173 2/2、VER-1）。
//!
//! 契約の型（`HistoryVersions` など）は contract の `history_version` が持つ。ここは、その型の値を
//! 開いている教材の IR から組み立てる側（書く側）と、版の欄の無い履歴を型の検査の前に拒否する入口を置く。
//! 呼び出し元は、履歴の包み（TASK-309）、開く可否の判定（TASK-174）、web の保存の画面である。
//!
//! 書く処理は I/O・時刻・乱数を持たない純関数である。IR と履歴は未信頼入力として扱い、曖昧な形
//! （`vocab` が文字列でない、`refs` の欄の欠落や重複、`format_version` の不一致）は既定値に倒さず失敗にする。
//! 書いた版は `HistoryVersions::parse` で自己検査し、読めない版を持つ履歴を作らない。
//! `ir_format` は IR の `format_version` だけから取り、`vocab` には影響されない（加算だけの語彙の追加で
//! `ir_format` を上げない）。
//!
//! 欠落と型の誤りの区別: `versions` が表として存在しないものだけを [`MissingVersion`] にする。
//! 表だが中の欄が欠ける・型が違う・未知の欄を持つものは欠落ではなく、後続の型の段（`malformed`）で落ちる。
//! [`check_versions_present`] は、`load_gate::parse_json_value` の後、型の変換の前に呼ぶ。
//!
//! 境界（ここで扱わないもの）:
//! - `content_hash` の求め方（TASK-180）。呼び出し側が `contract::content_hash::content_hash` で求め、検査済みの `ContentHash` を引数で渡す
//! - 履歴の包み（`format`・`history_format`・`pack_id`・`ops`・`snapshot`）の検査（TASK-309）
//! - 理由コードの文字列と判定の全体の順序（TASK-310）、開く可否の判定（TASK-174）

use core::fmt;
use std::collections::BTreeMap;

use fandhe_workshop_contract::history_version::{
    ContentHash, HistoryVersions, ReasonKind, Version3, VocabVersion,
};
use fandhe_workshop_contract::ir::Ir;
use serde_json::Value;

/// エンジンの版。core の crate の版（workspace の版）を継ぐ。
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 版を書けなかった理由。欄の名前と添字だけを持ち、IR の値は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordError {
    /// エンジンの版が `主.副.修` として読めない。
    Engine,
    /// IR の `format_version` と `pack.ir_format` が一致しない。
    IrFormatMismatch,
    /// `pack.vocab` が `主.副` の文字列でない。
    Vocab,
    /// `pack.version` が `主.副.修` として読めない。
    Content,
    /// `refs` の要素が、文字列の `name`・`commit` を持つ表でない。
    Ref {
        /// `refs` の中の位置。
        index: usize,
    },
    /// `refs` の `name` が重複している。
    DuplicateRef {
        /// 重複した側の要素の位置。
        index: usize,
    },
    /// 書いた版が読み側の検査（`HistoryVersions::parse`）を通らない。
    /// 各欄を検証済みなら到達しないが、contract の検査が増えたときに別の欄の不備を
    /// 誤った欄の名前で報告しないための受け皿である。
    Unreadable,
}

impl fmt::Display for RecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecordError::Engine => write!(f, "invalid engine version"),
            RecordError::IrFormatMismatch => {
                write!(f, "ir format_version differs from pack.ir_format")
            }
            RecordError::Vocab => write!(f, "invalid pack.vocab"),
            RecordError::Content => write!(f, "invalid pack.version"),
            RecordError::Ref { index } => write!(f, "invalid refs[{index}]"),
            RecordError::DuplicateRef { index } => write!(f, "duplicate name in refs[{index}]"),
            RecordError::Unreadable => write!(f, "recorded versions are unreadable"),
        }
    }
}

impl std::error::Error for RecordError {}

/// 現在のエンジンの版で、IR から 4 軸の版を書く。
///
/// # Errors
///
/// [`RecordError`]。エンジンの版が読めない場合と、IR の側の不備（[`record_versions_with`]）。
pub fn record_versions(
    ir: &Ir,
    content_hash: &ContentHash,
) -> Result<HistoryVersions, RecordError> {
    let engine = Version3::parse(ENGINE_VERSION).ok_or(RecordError::Engine)?;
    record_versions_with(ir, engine, content_hash)
}

/// エンジンの版を指定して、IR から 4 軸の版を書く（試験や、別の版を模す用途の入口）。
///
/// # Errors
///
/// `format_version` と `pack.ir_format` の不一致、`pack.vocab`・`pack.version` の不正、
/// `refs` の不正・重複。不備は既定値に倒さず失敗にする。
pub fn record_versions_with(
    ir: &Ir,
    engine: Version3,
    content_hash: &ContentHash,
) -> Result<HistoryVersions, RecordError> {
    if ir.format_version != ir.pack.ir_format {
        return Err(RecordError::IrFormatMismatch);
    }
    let vocab = match &ir.pack.vocab {
        None => VocabVersion::DEFAULT,
        Some(Value::String(s)) => VocabVersion::parse(s).ok_or(RecordError::Vocab)?,
        Some(_) => return Err(RecordError::Vocab),
    };
    let content = Version3::parse(&ir.pack.version).ok_or(RecordError::Content)?;

    let mut refs = BTreeMap::new();
    for (index, r) in ir.refs.iter().enumerate() {
        let field = |key: &str| r.get(key).and_then(Value::as_str);
        let (Some(name), Some(commit)) = (field("name"), field("commit")) else {
            return Err(RecordError::Ref { index });
        };
        if refs.insert(name.to_owned(), commit.to_owned()).is_some() {
            return Err(RecordError::DuplicateRef { index });
        }
    }

    let versions = HistoryVersions {
        engine: engine.to_string(),
        ir_format: ir.format_version,
        vocab: vocab.to_string(),
        content: content.to_string(),
        content_hash: content_hash.as_str().to_owned(),
        refs,
    };
    // 書いた版が読み側の検査を通ることを保証する（読めない履歴を作らない）。
    versions.parse().map_err(|_| RecordError::Unreadable)?;
    Ok(versions)
}

/// 履歴に `versions` の表が無いことを表す（VER-1）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingVersion;

impl MissingVersion {
    /// 理由の分類。コード名への対応づけは TASK-310 が持つ。
    #[must_use]
    pub const fn kind(&self) -> ReasonKind {
        ReasonKind::MissingVersion
    }
}

impl fmt::Display for MissingVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "history has no versions table")
    }
}

impl std::error::Error for MissingVersion {}

/// 履歴の JSON に `versions` の表があることを確かめる。
///
/// 最上位が表でない、`versions` が無い、または表でない（`null`・配列・文字列など）ときは欠落とする。
/// 表の中身の検査は行わない（型の段の役割）。
///
/// # Errors
///
/// [`MissingVersion`]。
pub fn check_versions_present(history: &Value) -> Result<(), MissingVersion> {
    match history.get("versions") {
        Some(Value::Object(_)) => Ok(()),
        _ => Err(MissingVersion),
    }
}
