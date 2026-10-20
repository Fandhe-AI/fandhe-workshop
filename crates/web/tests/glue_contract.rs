//! glue の許される操作（`glue_ops`、TASK-343 3/3）の契約適合試験（FEND-20・FEND-21・D-42）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、native の試験はソースを文字列として読んで照合する
//! （走査の道具は `tests/common`）。照合は fail-closed で、表に載っていない呼び出し名が 1 つでもあれば失敗する。
//! 表は試験の側にだけ置く（`make check-dom-reads` の走査対象に DOM 読み取りの綴りを持ち込まないため）。
//! 状態を変える経路が無いことは `state_paths` が持つ。本試験は DOM に対する操作の種類と数を固定する。
//! `glue.rs` は厳密な表（7 種）で、glue 以外の wasm32 専用モジュールは用途ごとの許可表で照合する
//! （層全体への拡張。TASK-362 2/2）。glue の行数と web-sys の feature の記録は `glue_scale`・`frontend_features`。

mod common;

use std::collections::HashSet;

use common::{count, glue_code, glue_source, wasm_layer_sources};
use fandhe_workshop_web::glue_ops::{
    tab_prevents_default, GlueOp, KeyHandlingOwner, MAX_ELEMENT_LOOKUPS, OUTSIDE_ROOT_ESC_OWNER,
    STATEFUL_PREVENT_DEFAULT_OWNER,
};
use fandhe_workshop_web::parts::PARTS_CONTRACT_VERSION;

/// DOM に対する呼び出し名と、対応する許される操作。
const DOM_CALLS: &[(&str, GlueOp)] = &[
    ("query_selector", GlueOp::ElementLookup),
    ("root", GlueOp::ElementLookup),
    ("set_value", GlueOp::TextareaSync),
    ("value", GlueOp::TextareaSync),
    ("is_composing", GlueOp::CompositionMark),
    ("key", GlueOp::KeydownReceive),
    ("key_code", GlueOp::KeydownReceive),
    ("shift_key", GlueOp::KeydownReceive),
    ("ctrl_key", GlueOp::KeydownReceive),
    ("alt_key", GlueOp::KeydownReceive),
    ("meta_key", GlueOp::KeydownReceive),
    ("repeat", GlueOp::KeydownReceive),
    ("target", GlueOp::KeydownReceive),
    // root の外の Esc（TASK-353 1/2。FEND-8・D-42）: keydown を登録する先の `document` を取り、
    // target が root の中かを `contains` で判定する。どちらも keydown の受け取りのための操作で、
    // 要素の取得（`ElementLookup`）には分類しない（root の取得ではなく、上限の対象外）。
    ("document", GlueOp::KeydownReceive),
    ("contains", GlueOp::KeydownReceive),
    ("prevent_default", GlueOp::KeydownReceive),
    ("window", GlueOp::Timer),
    (
        "set_timeout_with_callback_and_timeout_and_arguments_0",
        GlueOp::Timer,
    ),
    ("set_attribute", GlueOp::AttributeWrite),
    ("click", GlueOp::Click),
];

/// リスナーの登録。イベント名で操作の種類が決まる。
/// `_and_bool` は捕捉の段（capture）の登録で、`compositionend` を上流の泡立ちの段の配線より先に受けるために使う
/// （Issue #1510）。どちらの形も、第 1 引数のイベント名を同じ許可の表で照合する。
const LISTENER_CALLS: &[&str] = &[
    "add_event_listener_with_callback",
    "add_event_listener_with_callback_and_bool",
];

/// リスナーに登録してよいイベント名と、対応する許される操作。
const ALLOWED_EVENTS: &[(&str, GlueOp)] = &[
    ("input", GlueOp::TextareaSync),
    ("compositionend", GlueOp::CompositionMark),
    ("compositionstart", GlueOp::CompositionMark),
    ("keydown", GlueOp::KeydownReceive),
];

/// DOM ではない呼び出し名（Rust の標準・型変換・App と Runtime への読み取り）。
const NON_DOM_CALLS: &[&str] = &[
    "candidates_open",
    "with",
    "try_borrow",
    "try_borrow_mut",
    "as_ref",
    "map",
    "map_err",
    "ok",
    "unwrap_or",
    "clone",
    "forget",
    "dyn_into",
    "dyn_ref",
    "unchecked_ref",
    "is_some_and",
    "ok_or_else",
    "is_some",
    "chars",
    "count",
    "to_owned",
    "from_str",
    "new",
    "once_into_js",
    "mount",
    // 保存先を待ってから起動の分岐とマウントを行う入口（`persist`。TASK-319）。DOM には触れない。
    "mount_with_history",
    "component",
    "draft",
    "over_limit",
    "keydown_probe",
    "esc_count",
    // 進行・補完の配送（TASK-349）。`dispatch_action` は Runtime の口で、DOM には触れない。
    "dispatch_action",
    "encode",
    "Tick",
    "try_from",
    "complete_count",
    "scheduled_tick",
    "set_tick_request",
    "set_tick_reset",
    "wrapping_add",
    "set_draft_sync_request",
    "engine_snapshot_json",
    // 開いた直後の状態の読み取り（試験の設定だけ。TASK-175 2/2）。DOM には触れない。
    "last_import_state_json",
    "for_mount",
    "rerender",
    "flatten",
    "and_then",
    "is_empty",
];

/// 構造を変える操作。許される操作の一覧に無く、glue に 1 件も書かない。
const STRUCTURAL: &[&str] = &[
    "set_inner_html",
    "set_outer_html",
    "set_text_content",
    "append_child",
    "remove_child",
    "replace_child",
    "insert_before",
    "insert_adjacent_html",
    "create_element",
    "remove_attribute",
    "remove(",
];

fn is_ident(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// 正規化済みコードから、`.name(`・`::name(`・`.name::<`・`::name::<` の `name` を重複なしで集める。
fn call_names(code: &str) -> Vec<String> {
    let chars: Vec<char> = code.chars().collect();
    let mut names: Vec<String> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let start = if chars[i] == '.' {
            i + 1
        } else if chars[i] == ':' && chars.get(i + 1) == Some(&':') {
            i + 2
        } else {
            i += 1;
            continue;
        };
        let mut end = start;
        while end < chars.len() && is_ident(chars[end]) {
            end += 1;
        }
        let tail: String = chars[end..].iter().take(3).collect();
        if end > start && (tail.starts_with('(') || tail.starts_with("::<")) {
            let name: String = chars[start..end].iter().collect();
            if !names.contains(&name) {
                names.push(name);
            }
        }
        i = end.max(i + 1);
    }
    names
}

/// リスナー登録の第 1 引数（文字列リテラル）を集める。リテラルでない第 1 引数は空文字として返す（許可されない）。
fn listener_events(code: &str) -> Vec<String> {
    let mut events = Vec::new();
    for call in LISTENER_CALLS {
        let needle = format!("{call}(");
        for (pos, _) in code.match_indices(&needle) {
            let rest = &code[pos + needle.len()..];
            let event = rest
                .strip_prefix('"')
                .and_then(|r| r.split('"').next())
                .unwrap_or("");
            events.push(event.to_owned());
        }
    }
    events
}

fn lookup_sites(code: &str) -> usize {
    count(code, ".query_selector(") + count(code, ".root(")
}

/// 解析結果の違反を文字列で返す（空なら合格）。
fn violations(src: &str) -> Vec<String> {
    let code = glue_code(src);
    let mut v = Vec::new();
    let known: HashSet<&str> = DOM_CALLS
        .iter()
        .map(|(n, _)| *n)
        .chain(LISTENER_CALLS.iter().copied())
        .chain(NON_DOM_CALLS.iter().copied())
        .collect();
    for name in call_names(&code) {
        if !known.contains(name.as_str()) {
            v.push(format!("表に無い呼び出し: {name}"));
        }
    }
    for event in listener_events(&code) {
        if !ALLOWED_EVENTS.iter().any(|(e, _)| *e == event) {
            v.push(format!("許されないイベントのリスナー: {event:?}"));
        }
    }
    for pat in STRUCTURAL {
        let needle = if pat.ends_with('(') {
            format!(".{pat}")
        } else {
            format!(".{pat}(")
        };
        if count(&code, &needle) != 0 {
            v.push(format!("構造を変える操作: {pat}"));
        }
    }
    let n = lookup_sites(&code);
    if n > MAX_ELEMENT_LOOKUPS {
        v.push(format!(
            "要素の取得が {n} 箇所（{MAX_ELEMENT_LOOKUPS} 以下のはず）"
        ));
    }
    v
}

#[test]
fn 契約の版は_1() {
    assert_eq!(PARTS_CONTRACT_VERSION, 1);
}

#[test]
fn 許される操作は_7_件で名前が重複しない() {
    assert_eq!(GlueOp::ALL.len(), 7);
    let names: HashSet<&str> = GlueOp::ALL.iter().map(|o| o.name()).collect();
    assert_eq!(names.len(), 7);
    assert_eq!(MAX_ELEMENT_LOOKUPS, 2);
}

#[test]
fn 表の操作はすべて許される操作の一覧にある() {
    for (_, op) in DOM_CALLS.iter().chain(ALLOWED_EVENTS) {
        assert!(GlueOp::ALL.contains(op));
    }
}

#[test]
fn 実ソースは違反なし() {
    assert_eq!(violations(&glue_source()), Vec::<String>::new());
}

#[test]
fn glue_の呼び出し名はすべて表に載る() {
    let names = call_names(&glue_code(&glue_source()));
    // 走査が空振りしていないことの確認（抜き出せないと表の照合が素通りする）。
    assert!(names.len() >= 20, "{names:?}");
    assert!(names.iter().any(|n| n == "query_selector"));
}

#[test]
fn リスナーのイベントは許可の4種に限られる() {
    let events = listener_events(&glue_code(&glue_source()));
    assert!(!events.is_empty());
    for e in &events {
        assert!(ALLOWED_EVENTS.iter().any(|(n, _)| n == e), "{e:?}");
    }
}

#[test]
fn 構造を変える操作は_0_件() {
    let code = glue_code(&glue_source());
    for pat in STRUCTURAL {
        let needle = if pat.ends_with('(') {
            format!(".{pat}")
        } else {
            format!(".{pat}(")
        };
        assert_eq!(count(&code, &needle), 0, "{pat}");
    }
}

#[test]
fn 要素の取得の箇所は上限以下() {
    let n = lookup_sites(&glue_code(&glue_source()));
    assert!((1..=MAX_ELEMENT_LOOKUPS).contains(&n), "{n}");
}

const GOOD: &str = r#"
fn sync_draft(root: &Element) {
    let Ok(Some(el)) = root.query_selector("[data-testid=draft-input]") else { return; };
    // root.set_inner_html("") はコメントなので数えない
    with_app(|app| { if field.value() != draft { field.set_value(draft); } });
}
fn wire(root: &Element) {
    root.add_event_listener_with_callback("input", f)?;
    root.add_event_listener_with_callback("compositionend", g)?;
}
"#;

#[test]
fn 負例_基準となる合成ソースは違反なし() {
    assert_eq!(violations(GOOD), Vec::<String>::new());
}

#[test]
fn 負例_構造を変える操作を足すと検出する() {
    let src = GOOD.replace("return; };", "return; }; root.set_inner_html(\"\");");
    assert!(violations(&src).iter().any(|m| m.contains("構造")));
}

#[test]
fn 負例_remove_を足すと検出する() {
    let src = GOOD.replace("return; };", "return; }; el . remove ();");
    assert!(violations(&src).iter().any(|m| m.contains("remove")));
}

#[test]
fn 負例_未知の呼び出しを足すと検出する() {
    let src = GOOD.replace("return; };", "return; }; doc.focus_somewhere();");
    assert!(violations(&src).iter().any(|m| m.contains("表に無い")));
}

#[test]
fn 負例_許可外のイベントのリスナーを足すと検出する() {
    let src = GOOD.replace("\"compositionend\"", "\"paste\"");
    assert!(violations(&src).iter().any(|m| m.contains("paste")));
}

#[test]
fn 負例_捕捉の段の登録でも許可外のイベントを検出する() {
    let src = GOOD.replace(
        "add_event_listener_with_callback(\"compositionend\", g)",
        "add_event_listener_with_callback_and_bool(\"paste\", g, true)",
    );
    assert_ne!(src, GOOD);
    assert!(violations(&src).iter().any(|m| m.contains("paste")));
}

#[test]
fn 負例_捕捉の段の登録でもリテラルでないイベント名を検出する() {
    let src = GOOD.replace(
        "add_event_listener_with_callback(\"compositionend\", g)",
        "add_event_listener_with_callback_and_bool(name, g, true)",
    );
    assert_ne!(src, GOOD);
    assert!(violations(&src).iter().any(|m| m.contains("リスナー")));
}

#[test]
fn 負例_リテラルでないイベント名も検出する() {
    let src = GOOD.replace("\"compositionend\"", "name");
    assert!(violations(&src).iter().any(|m| m.contains("リスナー")));
}

#[test]
fn 負例_要素の取得を3箇所にすると検出する() {
    let src = GOOD.replace(
        "return; };",
        "return; }; root.query_selector(\"a\"); root.query_selector(\"b\");",
    );
    assert!(violations(&src).iter().any(|m| m.contains("要素の取得")));
}

#[test]
fn 負例_コメント内の一致は数えない() {
    let src = format!("{GOOD}\n// x.set_inner_html(\"\"); y.unknown_call();\n");
    assert_eq!(violations(&src), Vec::<String>::new());
}

#[test]
fn キー処理の受け持ちは_glue() {
    assert_eq!(STATEFUL_PREVENT_DEFAULT_OWNER, KeyHandlingOwner::Glue);
    assert_eq!(OUTSIDE_ROOT_ESC_OWNER, KeyHandlingOwner::Glue);
}

#[test]
fn tab_は候補が開いていて文字があるときだけ_prevent_default() {
    assert!(tab_prevents_default(true, true));
    assert!(!tab_prevents_default(true, false));
    assert!(!tab_prevents_default(false, true));
    assert!(!tab_prevents_default(false, false));
}

#[test]
fn 正例_document_の_keydown_のリスナーは違反なし() {
    let src = GOOD.replace(
        "root.add_event_listener_with_callback(\"input\", f)?;",
        "root.add_event_listener_with_callback(\"input\", f)?; document.add_event_listener_with_callback(\"keydown\", h)?;",
    );
    assert_ne!(src, GOOD);
    assert_eq!(violations(&src), Vec::<String>::new());
}

#[test]
fn 負例_document_に_keydown_以外のリスナーを足すと検出する() {
    let src = GOOD.replace(
        "root.add_event_listener_with_callback(\"input\", f)?;",
        "root.add_event_listener_with_callback(\"input\", f)?; document.add_event_listener_with_callback(\"click\", h)?;",
    );
    assert!(violations(&src).iter().any(|m| m.contains("click")));
}

// ---- 層全体（glue 以外の wasm32 専用モジュール）の許可表（FEND-20・TASK-362 2/2） ----
//
// FEND-20 の「DOM 操作は 7 種に限る」は glue だけでなく、web-sys を使う層の全モジュールに及ぶ。
// glue は上の厳密な表で照合する。glue 以外は、モジュールごとに用途と呼び出し名の許可表を持ち、
// 表に無い呼び出し名・許可外のイベント・構造を変える操作を失敗にする（fail-closed）。
// 保存先・書き出し・離脱時の保存・起動時の URL は、7 種の外にある用途で、ここで口を列挙して広がりを止める。

/// glue 以外のモジュールの許可表の 1 行。
struct ModuleRule {
    /// `lib.rs` の `mod` の名前。
    name: &'static str,
    /// 用途（なぜ web-sys を使ってよいか）。
    purpose: &'static str,
    /// このモジュールだけが使ってよい呼び出し名（web-sys の口と、このモジュール固有の名前）。
    calls: &'static [&'static str],
    /// リスナーに登録してよいイベント名。
    events: &'static [&'static str],
}

/// 層で共有する、DOM ではない呼び出し名（Rust 標準・`Option`/`Result`・js-sys・wasm-bindgen の型変換）。
/// glue の照合には使わない（glue は `NON_DOM_CALLS` だけで照合し、緩めない）。
const LAYER_NON_DOM_CALLS: &[&str] = &[
    "to_string",
    "as_deref",
    "ok_or",
    "push",
    "take",
    "borrow",
    "borrow_mut",
    "insert",
    "parse",
    "default",
    "as_str",
    "as_bytes",
    "unwrap_or_default",
    "unwrap_or_else",
    "is_none",
    "is_err",
    "from",
    "of1",
    "from_utf8",
    "as_string",
    "unchecked_into",
    "call0",
    "call1",
    "len",
];

/// glue 以外のモジュールの許可表。新しい wasm32 専用モジュールが増えたら、ここに行を足すまで失敗する。
const MODULE_RULES: &[ModuleRule] = &[
    ModuleRule {
        name: "storage_idb",
        purpose: "保存先（IndexedDB）",
        calls: &[
            "window",
            "indexed_db",
            "open_with_u32",
            "set_onversionchange",
            "set_oncomplete",
            "set_onabort",
            "set_onupgradeneeded",
            "set_onblocked",
            "set_onerror",
            "set_onsuccess",
            "close",
            "error",
            "transaction_with_str_and_mode",
            "transaction_with_str",
            "object_store",
            "put_with_key",
            "get",
            "get_all",
            "get_all_keys",
            "delete",
            "result",
            "create_object_store",
            "length",
            // このモジュール固有の名前（DOM ではない）。
            "set",
            "name",
            "start",
        ],
        events: &[],
    },
    ModuleRule {
        name: "storage_local",
        purpose: "保存先（localStorage）",
        calls: &[
            "window",
            "local_storage",
            "get_item",
            "set_item",
            "remove_item",
            // `Storage::key` と `Storage::length`。glue の `KeyboardEvent::key` とは別物で、ここでだけ許す。
            "key",
            "length",
            // このモジュール固有の名前（DOM ではない）。
            "storage",
            "keys",
        ],
        events: &[],
    },
    ModuleRule {
        name: "history_download",
        purpose: "書き出し（Blob と object URL）",
        calls: &[
            "revoke_object_url",
            "create_object_url_with_blob",
            "new_with_u8_array_sequence_and_options",
            "set_type",
        ],
        events: &[],
    },
    ModuleRule {
        name: "leave_flush",
        purpose: "離脱時の保存（`pagehide` の通知を受けるだけ）",
        calls: &[
            "window",
            "add_event_listener_with_callback",
            // `str::contains`（DOM の `Node::contains` ではない）。glue の keydown の判定とは別。
            "contains",
        ],
        events: &["pagehide"],
    },
    ModuleRule {
        name: "persist",
        purpose: "起動時の URL（`location.pathname`）から保存先を決めてマウントする",
        calls: &[
            "window",
            "location",
            "pathname",
            // このモジュール固有の名前（DOM ではない）。
            "read",
            "stage",
            "flush",
            "resolve",
            "reject",
            "from_pathname",
            "history_key",
            "mount_host",
            "for_mount_with",
            "mount_fixture_from_url",
            "boot",
            "block_saving",
        ],
        events: &[],
    },
    ModuleRule {
        name: "test_hooks",
        purpose: "試験用の口（配信物に入らない。`test-hooks` feature のときだけ）",
        calls: &[
            "window",
            "location",
            "pathname",
            "search",
            // 試験用の口が glue の行の識別を配線するための 1 件（件数は `root` の試験で固定する）。
            "root",
            // このモジュール固有の名前（DOM ではない）。
            "mount_fixture_from_search",
            "last_import_state",
            "keydown_probe_count",
            "draft_snapshot",
            "engine_snapshot",
            "is_mounted",
            "check_json_depth",
            "parse_json_value",
            "stage",
            "Limit",
            "load_json",
            "render",
            "render_doc_node",
            "flush",
            "for_pack",
            "from_pathname",
            "open_history_storage",
            "write",
            "read",
            "last_async_error",
            "kind_for",
            "idb_available",
            "kind",
            "render_tree_sample",
            "request_complete",
            "load",
            "wire_row_keys",
            "selected",
            "variant",
            "phase",
            "last_point",
            "view",
            "row",
            "point_count",
            "pending_len",
            "install_pagehide_flush",
            "two_rounds_envelope",
            "loadable_envelope",
            "export_bytes",
            "object_url",
            "file_name",
        ],
        events: &[],
    },
];

/// glue の表と同じ綴りだが、別モジュールでは別の意味で使う名前。許可表に書いてよい。
/// `window` は DOM の入口、`root` は試験用の口が 1 件だけ、`contains`・`key` は DOM ではない同名の呼び出し。
const GLUE_SHARED_NAMES: &[&str] = &["window", "root", "contains", "key"];

/// glue 以外のモジュールに持ち込めない glue 固有の DOM 操作（L4）。許可表に書けるのは `GLUE_SHARED_NAMES` だけ。
fn glue_only_names() -> Vec<&'static str> {
    DOM_CALLS
        .iter()
        .map(|(n, _)| *n)
        .filter(|n| !GLUE_SHARED_NAMES.contains(n))
        .collect()
}

/// 層全体の違反を文字列で返す（空なら合格）。glue は `violations` の厳密な照合を使う。
fn layer_violations(sources: &[(String, String)]) -> Vec<String> {
    let mut v = Vec::new();
    if sources.is_empty() {
        v.push("層のモジュールが 0 件".to_owned());
    }
    if !sources.iter().any(|(n, _)| n == "glue") {
        v.push("層に glue が無い".to_owned());
    }
    for (name, src) in sources {
        if name == "glue" {
            v.extend(violations(src).into_iter().map(|m| format!("glue: {m}")));
            continue;
        }
        let Some(rule) = MODULE_RULES.iter().find(|r| r.name == name) else {
            v.push(format!("{name}: 許可表に無いモジュール"));
            continue;
        };
        let code = glue_code(src);
        let known: HashSet<&str> = NON_DOM_CALLS
            .iter()
            .chain(LAYER_NON_DOM_CALLS)
            .chain(rule.calls)
            .copied()
            .collect();
        for n in call_names(&code) {
            if !known.contains(n.as_str()) {
                v.push(format!("{name}: 表に無い呼び出し: {n}"));
            }
        }
        for pat in STRUCTURAL {
            let needle = if pat.ends_with('(') {
                format!(".{pat}")
            } else {
                format!(".{pat}(")
            };
            if count(&code, &needle) != 0 {
                v.push(format!("{name}: 構造を変える操作: {pat}"));
            }
        }
        for event in listener_events(&code) {
            if !rule.events.contains(&event.as_str()) {
                v.push(format!("{name}: 許されないイベントのリスナー: {event:?}"));
            }
        }
        // リスナー登録の呼び出し自体も、イベントを許すモジュールでなければ許さない。
        if rule.events.is_empty() {
            for call in LISTENER_CALLS {
                if count(&code, &format!("{call}(")) != 0 {
                    v.push(format!("{name}: リスナー登録: {call}"));
                }
            }
        }
        if count(&code, ".query_selector(") != 0 {
            v.push(format!("{name}: glue の外で要素を取得している"));
        }
        if name != "test_hooks" && count(&code, ".root(") != 0 {
            v.push(format!("{name}: glue の外で root を取得している"));
        }
    }
    v
}

#[test]
fn 層全体は許可表に無い_dom_操作を持たない() {
    assert_eq!(
        layer_violations(&wasm_layer_sources()),
        Vec::<String>::new()
    );
}

#[test]
fn 層の全モジュールに許可表がある() {
    let sources = wasm_layer_sources();
    assert!(sources.len() >= 7, "{}", sources.len());
    for (n, _) in &sources {
        assert!(
            n == "glue" || MODULE_RULES.iter().any(|r| r.name == n),
            "許可表に無いモジュール: {n}"
        );
    }
    for r in MODULE_RULES {
        assert!(!r.purpose.is_empty(), "{}: 用途が空", r.name);
        assert!(
            sources.iter().any(|(n, _)| n == r.name),
            "{}: 層に無いモジュールの行（古い表）",
            r.name
        );
    }
}

#[test]
fn 許可表は_glue_固有の_dom_操作を持ち込まない() {
    let glue_only = glue_only_names();
    for r in MODULE_RULES {
        for c in r.calls {
            assert!(
                !glue_only.contains(c),
                "{}: glue 固有の操作 {c} を許可表に書いている",
                r.name
            );
        }
    }
}

#[test]
fn リスナーを登録してよいモジュールは_glue_と_leave_flush_だけ() {
    let with_events: Vec<&str> = MODULE_RULES
        .iter()
        .filter(|r| !r.events.is_empty())
        .map(|r| r.name)
        .collect();
    assert_eq!(with_events, vec!["leave_flush"]);
    let leave = MODULE_RULES.iter().find(|r| r.name == "leave_flush");
    assert_eq!(leave.map(|r| r.events), Some(&["pagehide"][..]));
}

#[test]
fn 試験用の口の_root_の取得は_1_件() {
    let src = wasm_layer_sources()
        .into_iter()
        .find(|(n, _)| n == "test_hooks")
        .map(|(_, s)| s)
        .expect("test_hooks");
    assert_eq!(count(&glue_code(&src), ".root("), 1);
}

fn layer(name: &str, body: &str) -> Vec<(String, String)> {
    vec![
        ("glue".to_owned(), GOOD.to_owned()),
        (name.to_owned(), body.to_owned()),
    ]
}

const PERSIST_OK: &str = "fn f() { let l = window().location(); l.pathname(); }";

#[test]
fn 負例_層の基準となる合成ソースは違反なし() {
    assert_eq!(
        layer_violations(&layer("persist", PERSIST_OK)),
        Vec::<String>::new()
    );
}

#[test]
fn 負例_glue_以外に構造を変える操作を足すと検出する() {
    let src = PERSIST_OK.replace("l.pathname();", "l.pathname(); el.set_inner_html(\"\");");
    assert!(layer_violations(&layer("persist", &src))
        .iter()
        .any(|m| m.contains("構造")));
}

#[test]
fn 負例_保存先に未知の呼び出しを足すと検出する() {
    let src = "fn f() { s.get_item(\"k\"); s.focus_somewhere(); }";
    assert!(layer_violations(&layer("storage_local", src))
        .iter()
        .any(|m| m.contains("focus_somewhere")));
}

#[test]
fn 負例_leave_flush_に_click_のリスナーを足すと検出する() {
    let src = "fn f() { window().add_event_listener_with_callback(\"click\", f); }";
    assert!(layer_violations(&layer("leave_flush", src))
        .iter()
        .any(|m| m.contains("click")));
}

#[test]
fn 正例_leave_flush_の_pagehide_は違反なし() {
    let src = "fn f() { window().add_event_listener_with_callback(\"pagehide\", f); }";
    assert_eq!(
        layer_violations(&layer("leave_flush", src)),
        Vec::<String>::new()
    );
}

#[test]
fn 負例_イベントを許さないモジュールにリスナーを足すと検出する() {
    let src = "fn f() { window().add_event_listener_with_callback(\"pagehide\", f); }";
    assert!(layer_violations(&layer("persist", src))
        .iter()
        .any(|m| m.contains("リスナー")));
}

#[test]
fn 負例_glue_以外で要素を取得すると検出する() {
    let src = "fn f() { doc.query_selector(\"a\"); }";
    assert!(layer_violations(&layer("persist", src))
        .iter()
        .any(|m| m.contains("要素を取得")));
}

#[test]
fn 負例_glue_固有の操作を別モジュールに足すと検出する() {
    let src = "fn f() { el.set_attribute(\"a\", \"b\"); el.click(); }";
    let v = layer_violations(&layer("storage_local", src));
    assert!(v.iter().any(|m| m.contains("set_attribute")), "{v:?}");
    assert!(v.iter().any(|m| m.contains("click")), "{v:?}");
}

#[test]
fn 負例_表に無いモジュールを検出する() {
    assert!(layer_violations(&layer("new_module", PERSIST_OK))
        .iter()
        .any(|m| m.contains("許可表に無いモジュール")));
}

#[test]
fn 負例_層の列挙が空なら失敗にする() {
    assert!(!layer_violations(&[]).is_empty());
}

#[test]
fn 負例_層の_glue_の違反も検出する() {
    let src = GOOD.replace("return; };", "return; }; root.set_inner_html(\"\");");
    let sources = vec![("glue".to_owned(), src)];
    assert!(layer_violations(&sources)
        .iter()
        .any(|m| m.starts_with("glue:")));
}
