//! glue.rs が状態を変える経路を持たないことの静的試験（BUILD-6・F-9）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、native の試験はソースを文字列として読んで調べる。
//! 分担: crate 全体の DOM 読み取りの件数は `tools/check-deps.sh`（`make check-dom-reads`）が持ち、
//! 本試験は glue の状態経路（App に触れる口の数と、読んだ値の使い方）を固定する。
//! 解析は、コメントを除き、空白と `r#` を除いた文字列で照合する。曖昧な形は検出側に倒す（見逃さない）。

mod common;

use common::{count, fn_body, glue_code, glue_source};

/// 配送の口。`.dispatch_action(` が許される唯一の関数（TASK-349。FEND-4）。
const DELIVER_FN: &str = "deliver";
/// 配送の予約。`deliver(` の呼び出しが許される唯一の関数。本体に `set_timeout` の予約がなければならない。
const SCHEDULE_FN: &str = "schedule_action";

/// 状態を変える呼び出しの形（正規化後）。`try_borrow_mut` 全体は禁止しない（mount_app が RUNTIME の枠に使う）。
/// `.dispatch_action(` は `deliver` の 1 箇所だけ許す（`delivery_violations`）。
const MUTATING: &[&str] = &[
    ".update(",
    "::update(",
    "::dispatch_action(",
    "component_mut",
    ".borrow_mut().as_mut()",
    ".apply_with_view_transition",
    "::apply_with_view_transition",
];

fn mutating_calls(code: &str) -> usize {
    MUTATING.iter().map(|p| count(code, p)).sum()
}

fn component_calls(code: &str) -> usize {
    count(code, ".component()") + count(code, "::component(")
}

/// 配送の口の規則の違反（空なら合格）。
///
/// - `.dispatch_action(` は glue 全体で 1 箇所、`deliver` の本体の中だけ
/// - `deliver(` の呼び出しは `schedule_action` の本体の中だけで、その本体に `set_timeout` の予約がある
///   （`Runtime::dispatch_action` を更新の最中・リスナーの中から呼ぶと `Reentrant` で捨てられるため）
fn delivery_violations(code: &str) -> Vec<String> {
    let mut v = Vec::new();
    let total = count(code, ".dispatch_action(");
    let in_deliver = fn_body(code, DELIVER_FN).map_or(0, |b| count(b, ".dispatch_action("));
    if total != 1 || in_deliver != 1 {
        v.push(format!(
            ".dispatch_action( が {total} 箇所（{DELIVER_FN} の 1 箇所だけのはず）"
        ));
    }
    // 定義 `fndeliver(` を除いた呼び出しの数。
    let calls = count(code, "deliver(") - count(code, "fndeliver(");
    match fn_body(code, SCHEDULE_FN) {
        Some(body) => {
            let inside = count(body, "deliver(");
            if calls != inside || inside == 0 {
                v.push(format!(
                    "deliver( の呼び出しが {SCHEDULE_FN} の外にある（全体 {calls} 箇所・内 {inside} 箇所）"
                ));
            }
            if !body.contains("once_into_js(")
                || !body.contains(".set_timeout_with_callback_and_timeout_and_arguments_0(")
            {
                v.push(format!("{SCHEDULE_FN} が set_timeout を通していない"));
            }
        }
        None => v.push(format!("{SCHEDULE_FN} が無い")),
    }
    v
}

/// 解析結果の違反を文字列で返す（空なら合格）。
fn violations(src: &str) -> Vec<String> {
    let code = glue_code(src);
    let mut v = delivery_violations(&code);
    if mutating_calls(&code) != 0 {
        v.push("状態を変える呼び出しがある".to_owned());
    }
    let n = component_calls(&code);
    if n != 1 {
        v.push(format!("component() が {n} 箇所（1 箇所のはず）"));
    }
    match fn_body(&code, "with_app") {
        Some(body) => {
            if component_calls(body) != 1 {
                v.push("component() が with_app の本体にない".to_owned());
            }
        }
        None => v.push("with_app が無い".to_owned()),
    }
    if !code.contains("f:implFnOnce(&EngineHost)->T") || code.contains("FnOnce(&mut") {
        v.push("with_app の閉包が共有参照を受けていない".to_owned());
    }
    let reads = count(&code, ".value()");
    if reads != 1 {
        v.push(format!(".value() が {reads} 箇所（1 箇所のはず）"));
    }
    match fn_body(&code, "sync_draft") {
        Some(body) => {
            if count(body, ".value()") != 1 || !body.contains("field.value()!=draft") {
                v.push("値の読み取りが sync_draft の比較にない".to_owned());
            }
            if mutating_calls(body) != 0 {
                v.push("sync_draft が状態を変える".to_owned());
            }
        }
        None => v.push("sync_draft が無い".to_owned()),
    }
    v
}

#[test]
fn glue_は状態を変える呼び出しを持たない() {
    let code = glue_code(&glue_source());
    assert_eq!(mutating_calls(&code), 0);
    assert_eq!(delivery_violations(&code), Vec::<String>::new());
    // mount_app の RUNTIME の枠への可変借用は、状態を変える経路ではない（禁止の形に当たらない）。
    assert!(code.contains("try_borrow_mut"));
}

#[test]
fn app_に触れるのは_with_app_の_component_の1箇所だけ() {
    let code = glue_code(&glue_source());
    assert_eq!(component_calls(&code), 1);
    let body = fn_body(&code, "with_app").expect("with_app が無い");
    assert_eq!(component_calls(body), 1);
    assert!(code.contains("f:implFnOnce(&EngineHost)->T"));
    assert!(!code.contains("FnOnce(&mut"));
}

#[test]
fn dom_の値の読み取りは_sync_draft_の比較だけ() {
    let code = glue_code(&glue_source());
    assert_eq!(count(&code, ".value()"), 1);
    let body = fn_body(&code, "sync_draft").expect("sync_draft が無い");
    assert!(body.contains("field.value()!=draft"));
    assert_eq!(mutating_calls(body), 0);
}

#[test]
fn 実ソースは違反なし() {
    assert_eq!(violations(&glue_source()), Vec::<String>::new());
}

const GOOD: &str = r#"
fn with_app<T>(f: impl FnOnce(&EngineHost) -> T) -> Option<T> {
    // rt.component() はコメントなので数えない
    let app = rt.component();
    Some(f(&*app))
}
fn sync_draft() {
    with_app(|app| { if field.value() != draft { field.set_value(draft); } });
}
fn deliver(a: &RuntimeAction) -> Option<DispatchEffect> {
    RUNTIME.with(|cell| { let rt = cell.try_borrow().ok()?; Some(rt.dispatch_action(n, &p)) })
}
fn schedule_action(a: RuntimeAction, d: i32) {
    let cb = Closure::once_into_js(move || { let _ = deliver(&a); });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), d);
}
"#;

#[test]
fn 負例_基準となる合成ソースは違反なし() {
    assert_eq!(violations(GOOD), Vec::<String>::new());
}

#[test]
fn 負例_component_を2回呼ぶと検出する() {
    let src = GOOD.replace("Some(f(&*app))", "let _b = rt.component(); Some(f(&*app))");
    assert!(!violations(&src).is_empty());
}

#[test]
fn 負例_dispatch_action_を足すと検出する() {
    let src = GOOD.replace("field.set_value(draft);", "rt.dispatch_action(a);");
    assert!(!violations(&src).is_empty());
}

#[test]
fn 負例_deliver_の外の_dispatch_action_を検出する() {
    let src = GOOD.replace(
        "Some(rt.dispatch_action(n, &p))",
        "Some(rt.dispatch_action(n, &p)) } ; fn other() { rt.dispatch_action(n, &p)",
    );
    assert!(violations(&src)
        .iter()
        .any(|m| m.contains(".dispatch_action(")));
}

#[test]
fn 負例_schedule_action_の外から_deliver_を呼ぶと検出する() {
    let src = GOOD.replace(
        "fn sync_draft() {",
        "fn on_key() { let _ = deliver(&a); }\nfn sync_draft() {",
    );
    assert!(violations(&src).iter().any(|m| m.contains("deliver(")));
}

#[test]
fn 負例_set_timeout_を通さない直接の配送を検出する() {
    let src = GOOD.replace(
        "let cb = Closure::once_into_js(move || { let _ = deliver(&a); });\n    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(cb.unchecked_ref(), d);",
        "let _ = deliver(&a);",
    );
    assert!(violations(&src).iter().any(|m| m.contains("set_timeout")));
}

#[test]
fn 負例_空白を挟んだ_component_も数える() {
    let src = GOOD.replace(
        "Some(f(&*app))",
        "let _b = rt . component ( ); Some(f(&*app))",
    );
    assert!(violations(&src).iter().any(|m| m.contains("component()")));
}

#[test]
fn 負例_raw_identifier_の_update_も検出する() {
    let src = GOOD.replace("field.set_value(draft);", "c.r#update(a);");
    assert!(!violations(&src).is_empty());
}

#[test]
fn 負例_コメント内の_component_は数えない() {
    let code = glue_code("// rt.component()\n/* a.component() */ fn x() {}");
    assert_eq!(component_calls(&code), 0);
}

#[test]
fn 負例_値の読み取りが増えると検出する() {
    let src = GOOD.replace("Some(f(&*app))", "let _v = other.value(); Some(f(&*app))");
    assert!(!violations(&src).is_empty());
}
