//! 履歴ファイルを読む経路の段 1〜3（大きさ・入れ子の深さ・JSON の段）の入口（TASK-331 1/3、SAVE-19・SAVE-20）。
//!
//! 呼び出し元は、web の復元（TASK-176・TASK-178）・ファイル選択（TASK-531）・後続の型の段（TASK-331 2/3）である。
//! 順序の正は contract の `HISTORY_LOAD_ORDER`。段 1・2 は [`crate::load_gate::check_limits`] を呼び、比較を複製しない。
//! 履歴は未信頼入力なので、解釈の前に大きさと深さで断つ。この関数は I/O・時刻・乱数を持たない純関数で、
//! 断っても保存領域にも画面の状態にも触れない（SAVE-19）。理由は固定のコードだけで、入力の内容を含めない（SAFE-40）。
//!
//! 段 3 の判定（`truncated` の位置を、ここで確かめて固定した）:
//! 大きさ → 深さ → UTF-8 → JSON の解釈の順に判定する。
//! - UTF-8 の検査が失敗し、末尾の列が不完全なら `truncated`、途中に不正なバイトがあれば `not_json`
//! - JSON の解釈が入力の終わりで途切れたなら `truncated`（0 バイト・空白だけの入力を含む）、それ以外の構文の誤りは `not_json`
//!
//! 意図して固定しないもの: 受講者向けの文面、段 4 以降（版の欄・型・版の判定）、IR の経路の UTF-8 の扱い
//! （`load_gate` では不正な UTF-8 は解釈の段の失敗のまま）。
//!
//! spec 側への提案（`docs/spec/**` は本 repo では編集しない）: SAVE-19 の「文字コードの検査は順序列に含めない。位置は未確認」を、
//! 「JSON の段の中で、大きさ・入れ子の後、解釈の前に判定する。末尾の不完全な UTF-8 と、解釈が入力の終わりで途切れたものを
//! `truncated` とし、途中の不正なバイトは `not_json` とする」に直す。

use fandhe_workshop_contract::limits::LoadEntry;
use fandhe_workshop_contract::load_reason::LoadReason;

use crate::load_gate::check_limits;

/// 履歴ファイルのバイト列を段 1〜3 で判定し、JSON の値を返す。
///
/// 保存先から読んだ文字列は `.as_bytes()` で渡す。
///
/// # Errors
///
/// 最初に違反した段の理由。大きさは `too_large`、深さは `too_deep`、
/// 末尾が不完全な UTF-8 または入力の終わりで途切れた JSON は `truncated`、その他は `not_json`。
pub fn parse_history_json(bytes: &[u8]) -> Result<serde_json::Value, LoadReason> {
    check_limits(LoadEntry::History, bytes).map_err(LoadReason::from)?;
    let text = std::str::from_utf8(bytes).map_err(utf8_reason)?;
    serde_json::from_str::<serde_json::Value>(text).map_err(json_reason)
}

/// UTF-8 の失敗の理由。`error_len` が無いのは、入力の終わりで列が途切れたとき。
fn utf8_reason(e: std::str::Utf8Error) -> LoadReason {
    if e.error_len().is_none() {
        LoadReason::Truncated
    } else {
        LoadReason::NotJson
    }
}

/// JSON の解釈の失敗の理由。入力の終わりで途切れたものだけが `truncated`。
fn json_reason(e: serde_json::Error) -> LoadReason {
    if e.is_eof() {
        LoadReason::Truncated
    } else {
        LoadReason::NotJson
    }
}
