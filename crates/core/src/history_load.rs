//! 履歴ファイルを読む経路の段 1〜8（大きさ・深さ・JSON・版の欄・型・版の判定・適用し直し・写しとの照合）の入口（TASK-331 1/3〜3/3、SAVE-19・SAVE-20）。
//!
//! 呼び出し元は、web の復元（TASK-176・TASK-178）・ファイル選択（TASK-531）である。
//! 段 7・8（開始点・適用し直しと写しとの照合）は [`load_history`] が最後に呼ぶ [`open_history`] の中にあり、
//! 条件は複製しない。試験は `tests/history_load.rs`、所要時間の記録は `docs/testing.md` の 5.65 にある。
//! `reload` を含む履歴は、再読み込みの意味の実装（TASK-320）まで適用し直せず `replay_mismatch` になる。
//! 順序の正は contract の `HISTORY_LOAD_ORDER`。段 1・2 は [`crate::load_gate::check_limits`] を呼び、比較を複製しない。
//! 履歴は未信頼入力なので、解釈の前に大きさと深さで断つ。この関数は I/O・時刻・乱数を持たない純関数で、
//! 断っても保存領域にも画面の状態にも触れない（SAVE-19）。理由は固定のコードだけで、入力の内容を含めない（SAFE-40）。
//!
//! 段 3 の判定（`truncated` の位置を、ここで確かめて固定した）:
//! 大きさ → 深さ → UTF-8 → JSON の解釈の順に判定する。
//! - UTF-8 の検査が失敗し、末尾の列が不完全なら `truncated`、途中に不正なバイトがあれば `not_json`
//! - JSON の解釈が入力の終わりで途切れたなら `truncated`（0 バイト・空白だけの入力を含む）、それ以外の構文の誤りは `not_json`
//!
//! 段 4〜6 の分担（TASK-331 2/3）:
//! - 段 4（版の欄）: [`check_history_head`]。`versions` が表であること（[`check_versions_present`]）に加え、`format` が形式名であること・
//!   `history_format` の欄があることを見る。3 条件はどれも `missing_version` で、互いの前後に意味は無い。
//!   【推論】`format` の違いと `history_format` の欠落を含めたのは、`version-expectations.md` の 1 節と試作の `open_history` が
//!   そう期待しているためである。`check_versions_present` は VER-1 の範囲（`versions` の表だけ）のまま変えない。
//! - 段 5（型）: [`parse_history_envelope`]。値から包みへの変換の失敗は `malformed`（`history_format` の型違い・`versions` の中の欄の
//!   欠落や未知の欄を含む）、`ops` が [`MAX_HISTORY_OPS`] を超えれば `too_many_ops`。`history_format_newer`・`other_pack` は
//!   [`open_history`] の冒頭にあり、条件を複製しない。
//! - 段 6（版の判定）: [`open_history`] にある 7 種（`bad_version` から `content_major`）をそのまま使う。
//!
//! 意図して固定しないもの: 受講者向けの文面、IR の経路の UTF-8 の扱い
//! （`load_gate` では不正な UTF-8 は解釈の段の失敗のまま）。
//!
//! spec 側への提案（`docs/spec/**` は本 repo では編集しない）: SAVE-19 の「文字コードの検査は順序列に含めない。位置は未確認」を、
//! 「JSON の段の中で、大きさ・入れ子の後、解釈の前に判定する。末尾の不完全な UTF-8 と、解釈が入力の終わりで途切れたものを
//! `truncated` とし、途中の不正なバイトは `not_json` とする」に直す。
//! 加えて、SAVE-19・VER-1 の `missing_version` が `format` の違いと `history_format` の欠落も含むことを明記する。

use fandhe_workshop_contract::history::HISTORY_FORMAT_NAME;
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::{LoadEntry, MAX_HISTORY_OPS};
use fandhe_workshop_contract::load_reason::LoadReason;
use serde_json::Value;

use crate::history_envelope::SessionEnvelope;
use crate::history_open::{open_history, Opened, OpeningCaps};
use crate::load_gate::check_limits;
use crate::session::SessionState;
use crate::version_record::check_versions_present;

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

/// 段 4（版の欄）。`versions` の表・`format`・`history_format` のどれかが欠ける、または形式名が違うときは `missing_version`。
///
/// 欄の中身（型・値）は見ない。それは段 5 以降の役割である。
///
/// # Errors
///
/// `missing_version`。
pub fn check_history_head(value: &Value) -> Result<(), LoadReason> {
    check_versions_present(value)?;
    if value.get("format").and_then(Value::as_str) != Some(HISTORY_FORMAT_NAME) {
        return Err(LoadReason::MissingVersion);
    }
    if value.get("history_format").is_none() {
        return Err(LoadReason::MissingVersion);
    }
    Ok(())
}

/// 段 4 と段 5 の前半（型と `ops` の件数）。JSON の値から履歴の包みを作る。
///
/// serde のエラー文は捨てる。入力の内容を理由に含めないためである（SAFE-40）。件数は型の変換の後に数える
/// （契約の順が `malformed` → `too_many_ops` のため）。変換の費用は段 1 の大きさの上限で決まる。
///
/// # Errors
///
/// 段 4 は `missing_version`、型の不一致は `malformed`、`ops` が [`MAX_HISTORY_OPS`] 件を超えれば `too_many_ops`。
pub fn parse_history_envelope(value: Value) -> Result<SessionEnvelope, LoadReason> {
    check_history_head(&value)?;
    let env =
        serde_json::from_value::<SessionEnvelope>(value).map_err(|_| LoadReason::Malformed)?;
    if env.ops.len() > MAX_HISTORY_OPS {
        return Err(LoadReason::TooManyOps);
    }
    Ok(env)
}

/// 履歴ファイルのバイト列を段 1〜8 で判定し、最後に [`open_history`] へ渡す入口。
///
/// 段 1〜5 前半で断ったものは `Opened::Refused` にする（`version-expectations.md` の 1〜7 がすべて「開かない」のため）。
/// 戻り値を `Opened` に揃えるのは、呼び出し元が 1 つの型で扱えるようにするためである。
/// 純関数で、保存領域にも画面にも触れない。段 7・8 も同じ呼び出しで通る。
pub fn load_history(
    bytes: &[u8],
    ir: &Ir,
    opening: &HistoryVersions,
    caps: &OpeningCaps,
    initial: SessionState,
) -> Opened {
    let env = parse_history_json(bytes).and_then(parse_history_envelope);
    match env {
        Ok(env) => open_history(ir, opening, caps, initial, &env),
        Err(code) => Opened::Refused { code },
    }
}
