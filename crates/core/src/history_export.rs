//! 履歴の包みを書き出すバイト列にする（TASK-330 1/2、SAVE-16・SAVE-17）。
//!
//! 呼び出し元は web のダウンロード（`crates/web` の `history_download`）と、保存先へ積む文字列を作る層である。
//! ブラウザの保存領域はオリジンごとに分かれるため、受講者が履歴を持ち運ぶ手段はファイルだけになる（SAVE-16）。
//! ここは、その書き出しの中身（バイト列）を作る純関数だけを持つ。I/O・時刻・乱数を持たない。
//!
//! 不変条件。
//! 1. 形は契約の [`EXPORT_FORM`]（`ops` と写しの両方。D-52・SAVE-17）。`ops` だけの形は作れない。
//!    値が増えたときは、下の網羅 match がコンパイルで知らせる
//! 2. 直列化は compact な 1 行の JSON である（PoC-6 の測定の基点と同じ形）。契約の試料（整形つき）は試験用の正の形で、
//!    別のものとして扱う
//! 3. 同じ包みからは同じバイト列が出る（`SessionState` の表は `BTreeMap`、欄は宣言順）。
//!    日時を入れないのも、この決定性のためである
//! 4. 失敗したら途中のバイト列を返さず [`ExportError`] で返す（fail-closed）
//!
//! 大きさの上限（推論）。spec に書き出し側の上限の記述は無い。書き出しは受講者の唯一の写しを持ち出す手段のため、
//! 読み込みの上限（[`MAX_HISTORY_BYTES`]・[`MAX_HISTORY_OPS`]）を超えても止めず、[`ExportReport`] の印で知らせる。
//! 案内は画面の責務（TASK-531）である。
//!
//! 未送信の下書きが書き出しに入るか（SAVE-18・D-52・D-111。TASK-329）の判定もここに置く。画面（SHELL-22。TASK-537）が
//! 書き出す前の案内を出すかどうかを決めるための真偽だけを返す。値は返さない。案内の対象は写しの下書きと、`ops` に残る
//! 送信していない下書きである（入力欄の今の値だけではない。D-111）。
//!
//! 含まないもの: ファイル名・ダウンロード（web）、読み込み（[`crate::history_load`]、TASK-330 2/2）、
//! 案内の文面と表示（SHELL-22。TASK-537）。履歴は下書きを含む受講者のデータであり、値を記録・出力しない。

use std::collections::BTreeMap;
use std::fmt;

use fandhe_workshop_contract::history::{
    ExportForm, HistoryOp, EXPORT_FORM, EXPORT_INCLUDES_DRAFT,
};
use fandhe_workshop_contract::limits::{MAX_HISTORY_BYTES, MAX_HISTORY_OPS};

use crate::addr::Root;
use crate::history_envelope::SessionEnvelope;
use crate::ops_record::SessionOp;

/// 書き出しの形を契約と照合する。値が増えたら網羅 match が壊れ、コンパイルで気づく。
const _: () = match EXPORT_FORM {
    ExportForm::OpsAndSnapshot => (),
};

/// 未送信の下書きを書き出しに含める決定（D-52）を契約と照合する。含めない案に変わったら、コンパイルで気づく。
const _: () = assert!(EXPORT_INCLUDES_DRAFT);

/// 書き出せなかった理由。値は持たない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExportError {
    /// JSON に直列化できない。
    Encode,
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Encode => f.write_str("history could not be encoded"),
        }
    }
}

impl std::error::Error for ExportError {}

/// 書き出しの大きさの報告。読み込みの上限を超えるかどうかの印を持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportReport {
    /// 書き出すバイト数。
    pub bytes: usize,
    /// `ops` の件数。
    pub ops: usize,
    /// バイト数が読み込みの上限（[`MAX_HISTORY_BYTES`]）を超える。
    pub exceeds_size_limit: bool,
    /// 件数が読み込みの上限（[`MAX_HISTORY_OPS`]）を超える。
    pub exceeds_ops_limit: bool,
}

/// 包みを compact な JSON のバイト列にする。
///
/// # Errors
///
/// 直列化に失敗したとき [`ExportError::Encode`]。途中までのバイト列は返さない。
pub fn export_bytes(env: &SessionEnvelope) -> Result<Vec<u8>, ExportError> {
    serde_json::to_vec(env).map_err(|_| ExportError::Encode)
}

/// 書き出すバイト列と包みから、大きさの報告を作る。上限を超えても書き出し自体は拒否しない。
#[must_use]
pub fn export_report(bytes: &[u8], env: &SessionEnvelope) -> ExportReport {
    ExportReport {
        bytes: bytes.len(),
        ops: env.ops.len(),
        exceeds_size_limit: u64::try_from(bytes.len()).map_or(true, |n| n > MAX_HISTORY_BYTES),
        exceeds_ops_limit: env.ops.len() > MAX_HISTORY_OPS,
    }
}

/// 書き出しに未送信の下書きが入るか（SAVE-18・D-52・D-111。TASK-329）。規則の入口で、書き出す包みの中身で判定する。
///
/// 呼び出し元は [`crate::recorded_session::RecordedSession::has_unsent_draft`]（web の
/// `EngineHost::export_includes_unsent_draft` が画面の案内（SHELL-22）のために読む）と、下の包み・バイト列の版である。
/// 次のどちらかを満たせば真。
/// 1. 写し `drafts` に、空でない値がある（空白だけの値も、ファイルに入るため真）
/// 2. `ops` に、送信していない空でない `Draft` が残っている（規則の本体は非公開の `unsent_draft_in`）。
///    入力欄が空でも、打った後に消した・書き換えた下書きが `Engine` を挟んで残っていれば真になる（D-111）
///
/// 記録の時点で消した値を `ops` から畳むことはしない（追記だけで増える列。D-53・EXT-5）。そのため、
/// 入力欄の今の値だけで数えるより真が多めに出る（安全側の誤り。D-111）。値は返さず真偽だけを返す。
#[must_use]
pub fn includes_unsent_draft(ops: &[SessionOp], drafts: &BTreeMap<Root, String>) -> bool {
    if drafts.values().any(|v| !v.is_empty()) {
        return true;
    }
    unsent_draft_in(ops.iter().filter_map(|op| match op {
        HistoryOp::Draft(t) => Some(StringOp::Draft(t.as_str())),
        HistoryOp::Submit(t) => Some(StringOp::Submit(Some(t.as_str()))),
        HistoryOp::Reload | HistoryOp::Engine(_) => None,
    }))
}

/// `ops` の文字列の操作（`Draft`・`Submit`）だけを抜いた 1 件。`Engine`・`Reload` は列に入れない（飛ばす）。
///
/// 型の版とバイト列の版が同じ規則（[`unsent_draft_in`]）を共有するための形である。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StringOp<'a> {
    /// 下書きの置き換え。
    Draft(&'a str),
    /// 送信。本文が文字列として読めないとき（バイト列の版）は `None` で、どの下書きとも同じ値とみなさない。
    Submit(Option<&'a str>),
}

/// 文字列の操作の列に、送信していない空でない `Draft` があるか（SAVE-18 の規則 (2)。D-111）。
///
/// 送信していない `Draft` は、空でない値を持つ `Draft` のうち、直後の文字列の操作が同じ値の `Submit` であるものを
/// 除いたもの（末尾の `Draft` を含む）である（spec の SAVE-18 の【推論】）。値で比べるのは、記録の口が送信の本文を
/// 下書きと別に受け取るため、直前の `Draft` と違う本文の送信で、送っていない値を数え漏らさないためである。
/// 前から 1 回だけ走査し、保留中の空でない下書きを直後の操作で決着させる。
fn unsent_draft_in<'a>(ops: impl IntoIterator<Item = StringOp<'a>>) -> bool {
    let mut pending: Option<&str> = None;
    for op in ops {
        match op {
            StringOp::Draft(t) => {
                if pending.is_some() {
                    return true;
                }
                pending = (!t.is_empty()).then_some(t);
            }
            StringOp::Submit(sent) => {
                if pending.is_some_and(|p| sent != Some(p)) {
                    return true;
                }
                pending = None;
            }
        }
    }
    pending.is_some()
}

/// 包みに [`includes_unsent_draft`] を当てる。
#[must_use]
pub fn envelope_includes_unsent_draft(env: &SessionEnvelope) -> bool {
    includes_unsent_draft(&env.ops, &env.snapshot.drafts)
}

/// 書き出しのバイト列（表示だけで開いた履歴の控え）に [`includes_unsent_draft`] と同じ規則を当てる。
///
/// 版が違うと包みの型に読めないことがあるため、`serde_json::Value` で `snapshot.drafts` と `ops` を見る（D-111 の
/// 「書き出す `ops`」に表示だけの控えも含むと読む。spec の SAVE-18 の【推論】）。
/// JSON として読めない・形が合わないときは真（fail-closed。案内を出す側に倒す）。`draft` の値が文字列でなければ真、
/// `submit` の値が文字列でなければどの下書きとも同じ値とみなさない。
#[must_use]
pub fn bytes_include_unsent_draft(bytes: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(bytes) else {
        return true;
    };
    let Some(drafts) = value
        .get("snapshot")
        .and_then(|s| s.get("drafts"))
        .and_then(serde_json::Value::as_object)
    else {
        return true;
    };
    let mut any_nonempty = false;
    for v in drafts.values() {
        match v.as_str() {
            Some(t) => any_nonempty |= !t.is_empty(),
            None => return true,
        }
    }
    if any_nonempty {
        return true;
    }
    let Some(ops) = value.get("ops").and_then(serde_json::Value::as_array) else {
        return true;
    };
    let mut string_ops = Vec::new();
    for op in ops {
        let Some(obj) = op.as_object() else {
            continue;
        };
        if let Some(d) = obj.get("draft") {
            let Some(t) = d.as_str() else {
                return true;
            };
            string_ops.push(StringOp::Draft(t));
        } else if let Some(sent) = obj.get("submit") {
            string_ops.push(StringOp::Submit(sent.as_str()));
        }
    }
    unsent_draft_in(string_ops)
}
