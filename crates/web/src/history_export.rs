//! 履歴の書き出しファイルの名前と種類（TASK-330 1/2、SAVE-16）。
//!
//! 呼び出し元は、ダウンロードの URL を作る `history_download`（wasm のみ）と、枠の書き出しの操作を描く `history_ops`（TASK-531 1/2）である。
//! バイト列そのものは core の `history_export::export_bytes` が作る。ここは DOM にも通信にも触れない純関数だけを置き、
//! native の試験で確かめる。
//!
//! ファイル名の `pack_id` は教材由来で信頼できない。`[a-z0-9-]` 以外を `-` に置き換え、64 文字で切るため、
//! パスの区切り・`..`・NUL・改行は残らない。日時は入れない（時刻に依存させず、書き出しを決定的に保つ）。

use fandhe_workshop_core::secret_scan::SecretShape;

/// 受講中の履歴を書き出してよいかの判定（SAFE-27・SAFE-28・D-44。TASK-164 2/3）。
///
/// 返すのは `EngineHost::export_live` で、呼び出し元は後続の書き出しの画面（TASK-531）と試験用の状態 JSON である。
/// 値を持つのは `Ready` だけ。`Held` は形の ID しか持たず、受講者の値を含めない。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExportGate {
    /// 書き出せる。中身は core の `export_bytes` のバイト列（保存と同じ包み）。
    Ready(Vec<u8>),
    /// 秘密情報らしい文字列の警告の間。選ぶまで書き出さない（D-44 の失敗側）。
    Held(Vec<SecretShape>),
    /// 書き出す履歴が無い（未読込・未操作・包みを組み立てられない・書き出せない）。
    Empty,
}

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

/// 書き出しのリンクの文字。受講者がこのリンクを押してファイルを保存する（書き出しは 2 段。TASK-531 1/2）。
pub const LINK_LABEL: &str = "履歴のファイルを保存する";

/// 秘密情報の警告の間に書き出しを押したときの固定の文（仮置き。最終の文面は TASK-534・TASK-537 などが持つ）。
/// 受講者の値・形の名前は含めない（SAFE-27）。
pub const HELD_STATUS: &str = "確認が済むまで書き出せません。";

/// 書き出す履歴が無いときに書き出しを押したときの固定の文（仮置き）。
pub const EMPTY_STATUS: &str = "書き出す履歴がまだありません。";

/// 書き出しを押した結果が履歴のファイルを出せない理由のときの文を返す。`Ready` には文が無く `None`。
#[must_use]
pub fn status_text(gate: &ExportGate) -> Option<&'static str> {
    match gate {
        ExportGate::Ready(_) => None,
        ExportGate::Held(_) => Some(HELD_STATUS),
        ExportGate::Empty => Some(EMPTY_STATUS),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 文は種類ごとに違い空でない() {
        let held = status_text(&ExportGate::Held(Vec::new())).unwrap();
        let empty = status_text(&ExportGate::Empty).unwrap();
        assert!(!held.is_empty() && !empty.is_empty());
        assert_ne!(held, empty);
        assert!(status_text(&ExportGate::Ready(Vec::new())).is_none());
    }
}
