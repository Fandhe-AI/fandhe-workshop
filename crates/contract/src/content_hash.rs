//! `content_hash` の求め方の仕様と計算（TASK-180、決定 D-51、VER-12）。
//!
//! 履歴の 4 軸の版の 1 つ `content_hash`（[`crate::history_version::ContentHash`]）に入れる値を求める。
//! 求め方をここ 1 か所に置き、pack（`compile` の後）・core・web（実行時の再計算。TASK-174 が履歴のハッシュと
//! 「いま開いている IR」のハッシュを比べる）が同じ関数を呼ぶ。I/O を持たない純関数で、wasm32 でも動く。
//!
//! # 仕様（版 [`CONTENT_HASH_SPEC_VERSION`]）
//!
//! 1. 入力は逆直列化した [`Ir`] の値。ファイルの生のバイト列は使わない（空白・`Value` の中のキーの順に依らない）
//! 2. 本文の木（[`crate::ir::Node::body_doc`]・[`crate::ir::Node::body_levels_doc`]）だけを除く。
//!    本文は `pack.version` で版を示すため、本文だけを直してもハッシュは変わらない
//! 3. 上記以外の欄は、すべて含める。型付きの欄も、語彙の中身を通す [`serde_json::Value`] の領域も同じ。
//!    `format_version`・`pack.ir_format` も含めるため、版 1 と版 2 の同じ教材はハッシュが違ってよい
//!    （`ir_format` の軸は別にある）。他の欄を足せば IR の JSON が変わり、ハッシュも変わる
//! 4. 除いた後の [`Ir`] を、構造体の欄の順のまま compact（空白なし）の JSON にする（`serde_json::to_vec`）。
//!    [`serde_json::Value`] へ変換してから直列化しない（`Map` がキーを並べ替え、欄の順が崩れる）
//! 5. そのバイト列の SHA-256 を、小文字 16 進 64 文字で表す
//!
//! # 未決の点
//!
//! IR 版 2 で載る練習データの内容の木（[`Ir::fixture_docs`]）をハッシュに含めるかは、D-51 で決めていない。
//! 決まった範囲（本文の木だけを除く）を超えて除外を広げないため、暫定で含める。決定が変わったときは
//! [`CONTENT_HASH_SPEC_VERSION`] を上げ、サンプルの期待を直す。
//!
//! # 用途の限界
//!
//! 用途は「同じ教材内容か」の判定材料で、履歴の改ざんの検知（認証）ではない。改ざんは、適用し直しと
//! 写しの照合（TASK-174・TASK-310）が扱う。

use std::fmt;

use crate::history_version::ContentHash;
use crate::ir::Ir;
use crate::sha256::sha256;

/// この仕様の版。除く欄・バイト列の形・ハッシュ関数を変えるときに上げる（spec を先に直し、サンプルの期待を直す）。
pub const CONTENT_HASH_SPEC_VERSION: u32 = 1;

/// `content_hash` を求められなかった理由。IR の中身（本文・練習データ）は持たない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContentHashError {
    /// IR の JSON への直列化に失敗した。空や既定のハッシュには倒さない。
    Serialize,
}

impl fmt::Display for ContentHashError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Serialize => f.write_str("failed to serialize the IR for content_hash"),
        }
    }
}

impl std::error::Error for ContentHashError {}

/// 本文の木を除いた IR の SHA-256 を求める。入力の `ir` は変更しない。
///
/// # Errors
///
/// 直列化に失敗したとき（[`ContentHashError::Serialize`]）。
pub fn content_hash(ir: &Ir) -> Result<ContentHash, ContentHashError> {
    let mut stripped = ir.clone();
    for node in &mut stripped.nodes {
        node.body_doc = None;
        node.body_levels_doc.clear();
    }
    let bytes = serde_json::to_vec(&stripped).map_err(|_| ContentHashError::Serialize)?;
    Ok(ContentHash::from_digest(&sha256(&bytes)))
}
