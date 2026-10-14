//! ブラウザ試験用の口。feature `test-hooks` のときだけ組み込まれる。
//!
//! 配信する設定には入れない。`static/test-hooks.js` がここの関数を JS へ橋渡しし、
//! TASK-28 以降のブラウザ試験が状態を読む。`debug_rerender` を除き、状態は変更しない。

use wasm_bindgen::prelude::*;

/// 現在の画面の状態を JSON 文字列で返す（読み取りのみ）。
#[wasm_bindgen]
pub fn debug_state_json() -> String {
    let probe = crate::glue::keydown_probe_count()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string());
    // 本文は JSON に入れない（エスケープを避ける）。照合は `debug_draft` で行う。
    let (cps, over) = crate::glue::draft_snapshot()
        .map(|(n, o, _)| (n.to_string(), o.to_string()))
        .unwrap_or_else(|| ("null".to_string(), "null".to_string()));
    format!(
        "{{\"screen\":\"hello\",\"mounted\":{},\"keydownProbe\":{},\"draftCodePoints\":{},\"inputOverLimit\":{}}}",
        crate::glue::is_mounted(),
        probe,
        cps,
        over
    )
}

/// 状態の下書き本文を返す（読み取りのみ）。未マウントなら空文字列。
#[wasm_bindgen]
pub fn debug_draft() -> String {
    crate::glue::draft_snapshot()
        .map(|(_, _, d)| d)
        .unwrap_or_default()
}

/// 画面を丸ごと再構築する（再描画の時間の測定用）。実行したら `true`、未マウントなどで何もしなかったら `false`。
///
/// 配信する設定には入らない（`debug_` 接頭辞は `scripts/test-build-web.sh` が不在を検査する）。
#[wasm_bindgen]
pub fn debug_rerender() -> bool {
    crate::glue::rerender()
}

/// 入れ子の深さの検査（core の `check_json_depth`）を呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// 10,000 段でもスタックを使い切らず理由が返ることを、ブラウザ試験（TASK-157）が確かめるための口。
/// 入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_json_depth(text: &str) -> String {
    match fandhe_workshop_core::json_depth::check_json_depth(text.as_bytes()) {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(reason) => format!("{{\"ok\":false,\"reason\":\"{}\"}}", reason.as_str()),
    }
}

/// IR・履歴の読み込みの関門（core の `parse_json_value`）の 3 段目までを呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// `entry` は `"ir"` か `"history"`。それ以外は `unknown_entry` を返す。型・内容の段は `serde_json::Value` では
/// 常に通るため、ここでは確かめない（native の `load_order` 試験が持つ）。入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_json_load(entry: &str, text: &str) -> String {
    use fandhe_workshop_contract::limits::{LoadEntry, LoadStage};
    let entry = match entry {
        "ir" => LoadEntry::Ir,
        "history" => LoadEntry::History,
        _ => return "{\"ok\":false,\"error\":\"unknown_entry\"}".to_string(),
    };
    match fandhe_workshop_core::load_gate::parse_json_value(entry, text.as_bytes()) {
        Ok(_) => "{\"ok\":true}".to_string(),
        Err(err) => {
            let stage = match err.stage() {
                LoadStage::Size => "size",
                LoadStage::Depth => "depth",
                LoadStage::JsonParse => "json_parse",
                LoadStage::TypeAndContent => "type_and_content",
            };
            match err {
                fandhe_workshop_core::load_gate::LoadError::Limit(reason) => format!(
                    "{{\"ok\":false,\"stage\":\"{stage}\",\"reason\":\"{}\"}}",
                    reason.as_str()
                ),
                _ => format!("{{\"ok\":false,\"stage\":\"{stage}\"}}"),
            }
        }
    }
}
