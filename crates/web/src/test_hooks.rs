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
