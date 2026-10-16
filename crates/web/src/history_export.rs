//! 履歴の書き出しファイルの名前と種類（TASK-330 1/2、SAVE-16）。
//!
//! 呼び出し元は、ダウンロードの URL を作る `history_download`（wasm のみ）と、後続の画面（TASK-531）である。
//! バイト列そのものは core の `history_export::export_bytes` が作る。ここは DOM にも通信にも触れない純関数だけを置き、
//! native の試験で確かめる。
//!
//! ファイル名の `pack_id` は教材由来で信頼できない。`[a-z0-9-]` 以外を `-` に置き換え、64 文字で切るため、
//! パスの区切り・`..`・NUL・改行は残らない。日時は入れない（時刻に依存させず、書き出しを決定的に保つ）。

/// 書き出しファイルの MIME。
pub const MIME: &str = "application/json";

/// ファイル名の接頭辞。
const PREFIX: &str = "fandhe-workshop-history";

/// ファイル名に入れる教材 ID の最大の文字数。
const MAX_PACK_CHARS: usize = 64;

/// 書き出しファイルの名前を返す。`pack_id` が整えた後に空なら接頭辞だけにする。
#[must_use]
pub fn file_name(pack_id: &str) -> String {
    let cleaned: String = pack_id
        .chars()
        .take(MAX_PACK_CHARS)
        .map(|c| {
            if c.is_ascii_lowercase() || c.is_ascii_digit() {
                c
            } else {
                '-'
            }
        })
        .collect();
    if cleaned.is_empty() {
        format!("{PREFIX}.json")
    } else {
        format!("{PREFIX}-{cleaned}.json")
    }
}
