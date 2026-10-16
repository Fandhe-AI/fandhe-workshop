//! glue の許される操作（`glue_ops`、TASK-343 3/3）の契約適合試験（FEND-20・FEND-21・D-42）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、native の試験はソースを文字列として読んで照合する
//! （走査の道具は `tests/common`）。照合は fail-closed で、表に載っていない呼び出し名が 1 つでもあれば失敗する。
//! 表は試験の側にだけ置く（`make check-dom-reads` の走査対象に DOM 読み取りの綴りを持ち込まないため）。
//! 状態を変える経路が無いことは `state_paths` が持つ。本試験は DOM に対する操作の種類と数を固定する。

mod common;

use std::collections::HashSet;

use common::{count, glue_code, glue_source};
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
const LISTENER_CALL: &str = "add_event_listener_with_callback";

/// リスナーに登録してよいイベント名と、対応する許される操作。
const ALLOWED_EVENTS: &[(&str, GlueOp)] = &[
    ("input", GlueOp::TextareaSync),
    ("compositionend", GlueOp::CompositionMark),
    ("compositionstart", GlueOp::CompositionMark),
    ("keydown", GlueOp::KeydownReceive),
];

/// DOM ではない呼び出し名（Rust の標準・型変換・App と Runtime への読み取り）。
const NON_DOM_CALLS: &[&str] = &[
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
    "is_some",
    "chars",
    "count",
    "to_owned",
    "from_str",
    "new",
    "once_into_js",
    "mount",
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
    let needle = format!("{LISTENER_CALL}(");
    let mut events = Vec::new();
    for (pos, _) in code.match_indices(&needle) {
        let rest = &code[pos + needle.len()..];
        let event = rest
            .strip_prefix('"')
            .and_then(|r| r.split('"').next())
            .unwrap_or("");
        events.push(event.to_owned());
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
        .chain(std::iter::once(LISTENER_CALL))
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
