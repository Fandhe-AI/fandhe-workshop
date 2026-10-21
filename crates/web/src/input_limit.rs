//! 入力欄 1 回分の入力の上限（SAFE-21。TASK-158）。
//!
//! 巨大な貼り付けによるメモリ・処理時間の浪費を防ぐため、状態へ入れる前に値を
//! コードポイントで 20,000 文字に切る。`hello` の画面が `SetDraft` を受けるときに呼ぶ純粋関数で、
//! DOM に触れない。値は contract の `limits`（上限の正）から取り、ここでは再定義しない。
//! HTML の `maxlength` は UTF-16 の単位で数えるため使えない。

use fandhe_workshop_contract::limits::{INPUT_TOO_LONG_MESSAGE, MAX_INPUT_CODE_POINTS};

/// 値を上限（コードポイント）で切る。戻り値は切った後の値と、切り捨てが起きたかどうか。
///
/// 切る位置は値全体の 20,001 コードポイント目以降で、挿入位置は見ない。これは安全網である。
/// 途中への貼り付けで貼った分だけを切るのは [`clip_paste`]（D-60。JUDGE-24）が受け持つ。
/// 文字（`char`）の境界で切るため、サロゲートペアの文字を割らない。走査は上限で止まるので、
/// 200,000 文字の貼り付けでも O(上限) で済む。
pub fn clamp_draft(value: &str) -> (&str, bool) {
    // バイト数が上限以下なら、コードポイント数も上限以下（1 文字は 1 バイト以上）。
    if value.len() <= MAX_INPUT_CODE_POINTS {
        return (value, false);
    }
    match value.char_indices().nth(MAX_INPUT_CODE_POINTS) {
        Some((idx, _)) => (&value[..idx], true),
        None => (value, false),
    }
}

/// 超過の状態に対応する案内文。超過でなければ空文字列（案内を消す）。
pub fn notice_for(over_limit: bool) -> &'static str {
    if over_limit {
        INPUT_TOO_LONG_MESSAGE
    } else {
        ""
    }
}

/// 上限を超える貼り付けを、貼った分だけで切る計画（JUDGE-24・D-60）。
///
/// 呼び出し元は glue の `beforeinput` のリスナーで、`value` を欄へ書き、`caret_utf16` へキャレットを置く。
#[derive(Debug, PartialEq, Eq)]
pub struct PasteClip {
    /// 欄へ書く値。入る分だけの貼り付けと、はみ出した先頭 1 文字（理由は [`clip_paste`]）を含む。
    pub value: String,
    /// 切った後のキャレットの位置（UTF-16 の単位）。残した貼り付け部分の直後。
    pub caret_utf16: u32,
}

/// 範囲 `[start, end)`（UTF-16）を `pasted` で置き換えると上限を超えるとき、貼った分だけを切る計画を返す。
///
/// 次のときは `None` を返し、既定の挿入と [`clamp_draft`]（末尾で切る安全網）に任せる。
/// - 上限に収まる（そのまま入る）
/// - 選択の終わりより後ろが空（末尾で切っても貼った分だけが切られ、結果は同じ）
/// - 位置が範囲外・逆順・サロゲートペアの途中、または選択の外だけで上限を超える（信頼しない入力への fail-closed）
///
/// 値の末尾に、はみ出した先頭 1 文字を置く。状態側の `clamp_draft` がちょうどその 1 文字を切り、
/// 超過の印と案内が立つ。超過の判定を `clamp_draft` の 1 か所に保つための形である。
/// 改行は textarea が挿入のときに LF へ揃えるため、数え方をそれに合わせる。
/// 走査は O(上限 + 貼り付けの長さ) で、はみ出した残りは複製しない。
pub fn clip_paste(
    draft: &str,
    start_utf16: u32,
    end_utf16: u32,
    pasted: &str,
) -> Option<PasteClip> {
    if start_utf16 > end_utf16 {
        return None;
    }
    let start = byte_of_utf16(draft, start_utf16)?;
    let end = byte_of_utf16(draft, end_utf16)?;
    let (before, after) = (&draft[..start], &draft[end..]);
    if after.is_empty() {
        return None;
    }
    let outside = before.chars().count() + after.chars().count();
    let room = MAX_INPUT_CODE_POINTS.checked_sub(outside)?;
    let normalized: String;
    let pasted = if pasted.contains('\r') {
        normalized = pasted.replace("\r\n", "\n").replace('\r', "\n");
        normalized.as_str()
    } else {
        pasted
    };
    // バイト数が room 以下なら、コードポイント数も room 以下。
    if pasted.len() <= room {
        return None;
    }
    let cut = match pasted.char_indices().nth(room) {
        Some((idx, _)) => idx,
        None => return None,
    };
    let (kept, rest) = pasted.split_at(cut);
    let overflow = rest.chars().next()?;
    let mut value = String::with_capacity(draft.len() + kept.len() + 4);
    value.push_str(before);
    value.push_str(kept);
    value.push_str(after);
    value.push(overflow);
    let kept_utf16 = u32::try_from(kept.encode_utf16().count()).ok()?;
    Some(PasteClip {
        value,
        caret_utf16: start_utf16.checked_add(kept_utf16)?,
    })
}

/// UTF-16 の位置を `s` のバイト位置へ直す。範囲外・サロゲートペアの途中は `None`。
fn byte_of_utf16(s: &str, pos: u32) -> Option<usize> {
    let mut units: u32 = 0;
    for (idx, c) in s.char_indices() {
        if units == pos {
            return Some(idx);
        }
        if units > pos {
            return None;
        }
        units += u32::try_from(c.len_utf16()).ok()?;
    }
    (units == pos).then_some(s.len())
}
