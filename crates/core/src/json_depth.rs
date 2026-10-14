//! IR・履歴の JSON の入れ子の深さを、解釈の前に数えて断る検査（TASK-157、SAFE-20・SAFE-42）。
//!
//! 読み込み口（TASK-154・155・331）が、大きさの検査の後・`serde_json` の解釈の前に呼ぶ。順序の正は
//! SAFE-20 と contract の `LOAD_STAGE_ORDER` である。走査は再帰せず入力長に線形で、深い入力でも
//! スタックを使い切らない。`serde_json` の再帰の上限（128 段）は、この検査の後ろの保険として残る。
//!
//! 意図して固定しないもの: UTF-8 の検査との前後、読み込み口への組み込み、受講者向けの文面の選択
//! （文面は呼び出し元が contract の `rejection_message` から選ぶ）。

use fandhe_workshop_contract::limits::{LimitReason, MAX_JSON_DEPTH};

/// 入れ子の最大の段数を、解釈せずに数える（文字列の中は読み飛ばす。再帰しない）。
///
/// 最上位の `{}`・`[]` を 1 段とする。閉じていない・不正な入力でも止まり、そこまでの最大を返す。
#[must_use]
pub fn json_depth(bytes: &[u8]) -> usize {
    scan(bytes, usize::MAX)
}

/// 入れ子が上限（contract の `MAX_JSON_DEPTH`）を超えれば `Err(LimitReason::TooDeep)` を返す。
///
/// 上限ちょうどは断らない。エラーに入力の内容・段数の実測値を含めない。
///
/// # Errors
///
/// 入れ子が `MAX_JSON_DEPTH` を超えたとき `LimitReason::TooDeep`。
pub fn check_json_depth(bytes: &[u8]) -> Result<(), LimitReason> {
    if scan(bytes, MAX_JSON_DEPTH) > MAX_JSON_DEPTH {
        Err(LimitReason::TooDeep)
    } else {
        Ok(())
    }
}

/// 括弧の段数の最大を数える。`stop_above` を超えた時点で打ち切る（深い入力ほど早く返る）。
///
/// 走査が見る `"`・`\`・括弧は ASCII で、UTF-8 の複数バイトの文字（0x80 以上）と衝突しない。
fn scan(bytes: &[u8], stop_above: usize) -> usize {
    let mut depth = 0usize;
    let mut max = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for &b in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if b == b'\\' {
                escaped = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => {
                depth += 1;
                if depth > max {
                    max = depth;
                    if max > stop_above {
                        return max;
                    }
                }
            }
            b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    max
}
