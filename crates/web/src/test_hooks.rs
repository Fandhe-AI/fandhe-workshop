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
    let esc = crate::glue::esc_count()
        .map(|n| n.to_string())
        .unwrap_or_else(|| "null".to_string());
    // 本文は JSON に入れない（エスケープを避ける）。照合は `debug_draft` で行う。
    let (cps, over) = crate::glue::draft_snapshot()
        .map(|(n, o, _)| (n.to_string(), o.to_string()))
        .unwrap_or_else(|| ("null".to_string(), "null".to_string()));
    // 3 つの表示の照合用の状態（TASK-347 2/2）。未マウントなら null。中身の組み立ては `EngineHost` が持つ。
    let engine = crate::glue::engine_snapshot().unwrap_or_else(|| "null".to_string());
    format!(
        "{{\"screen\":\"hello\",\"mounted\":{},\"keydownProbe\":{},\"escCount\":{},\"draftCodePoints\":{},\"inputOverLimit\":{},\"engine\":{}}}",
        crate::glue::is_mounted(),
        probe,
        esc,
        cps,
        over,
        engine
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

/// 本文の木の読み込み（core の `load_json::<Doc>`）を呼び、固定形の JSON で結果を返す（読み取りのみ）。
///
/// 手作りの木が関門のどの段で止まるかを、ブラウザ試験（TASK-149）が確かめるための口。
/// 入力の内容は返り値に含めない。
#[wasm_bindgen]
pub fn debug_check_doc(text: &str) -> String {
    use fandhe_workshop_contract::doc::Doc;
    use fandhe_workshop_contract::limits::{LoadEntry, LoadStage};
    match fandhe_workshop_core::load_gate::load_json::<Doc>(LoadEntry::Ir, text.as_bytes()) {
        Ok(_) => "{\"ok\":true}".to_string(),
        Err(err) => {
            let stage = match err.stage() {
                LoadStage::Size => "size",
                LoadStage::Depth => "depth",
                LoadStage::JsonParse => "json_parse",
                LoadStage::TypeAndContent => "type_and_content",
            };
            format!("{{\"ok\":false,\"stage\":\"{stage}\"}}")
        }
    }
}

/// 本文の木を関門に通し、通れば描画した HTML 文字列を返す（読み取りのみ）。拒否なら空文字列。
///
/// 本体のマウントと同じ「`render` の文字列 → innerHTML」の経路を、試験が `#app` の外の容器で再現するための口。
#[wasm_bindgen]
pub fn debug_render_doc(text: &str) -> String {
    use fandhe_workshop_contract::doc::Doc;
    use fandhe_workshop_contract::limits::LoadEntry;
    match fandhe_workshop_core::load_gate::load_json::<Doc>(LoadEntry::Ir, text.as_bytes()) {
        Ok(doc) => fandhe_frontend_core::render(&crate::doc_view::render_doc_node(&doc)),
        Err(_) => String::new(),
    }
}

/// `debug_render_quote_nest` が受け付ける段数の上限。`Doc` の生成と破棄は段数分だけ再帰するため、試験の設定でも
/// 無制限の段数を Rust 側で組み立てない。
const MAX_QUOTE_NEST_FOR_HOOK: u32 = 200;

/// 引用を `n` 段入れ子にした木を Rust で組み立てて描画し、HTML 文字列を返す（読み取りのみ）。
///
/// 関門（JSON の深さ 64）は 50・60 段の木を描画まで通さないため、描画の深さの上限（SAFE-12）を
/// ブラウザで確かめるには関門を通さずに木を作る口が要る。`n` が上限を超えたときは空文字列を返す。
#[wasm_bindgen]
pub fn debug_render_quote_nest(n: u32) -> String {
    use fandhe_workshop_contract::doc::{Block, Doc, TREE_VERSION};
    if n > MAX_QUOTE_NEST_FOR_HOOK {
        return String::new();
    }
    let mut blocks: Vec<Block> = Vec::new();
    for _ in 0..n {
        blocks = vec![Block::Quote { children: blocks }];
    }
    let doc = Doc {
        version: TREE_VERSION,
        blocks,
    };
    fandhe_frontend_core::render(&crate::doc_view::render_doc_node(&doc))
}

/// 進行の間隔（ミリ秒）の定数を返す（読み取りのみ）。
///
/// 配信する設定が `?tick=5` を無視して同じ値のままであることを、ブラウザ試験（TASK-229）が確かめるための口。
/// 値を書き換える口は作らない。
#[wasm_bindgen]
pub fn debug_tick_interval_ms() -> u32 {
    crate::tick_interval::TICK_INTERVAL_MS
}

/// ツリーの試料（ファイルツリー・タスクツリー）を、後の状態で描いた HTML 文字列を返す（読み取りのみ）。
///
/// `kind` は `file_tree`・`task_tree`、`variant` は `a`・`b`。範囲外・描けない場合は空文字列を返し、黙って別の形に
/// 退避しない。ブラウザ試験（TASK-472）が、ツリーの属性と全項目が見えることを `#app` の外の容器で確かめるための口で、
/// 試料は埋め込みの固定データだけである。
#[wasm_bindgen]
pub fn debug_render_tree(kind: &str, variant: &str) -> String {
    crate::test_fixture::render_tree_sample(kind, variant).unwrap_or_default()
}
