//! wasm32 専用の web-sys 層が状態を変える経路を `update` だけに保つことの静的試験
//! （BUILD-6・F-9・FEND-20。TASK-362 1/2）。
//!
//! 前半は `glue.rs` の状態経路（TASK-81・TASK-349）、後半の `layer_violations` は glue 以外の保存・書き出し・
//! 試験用の口を含む層全体の規則（R1〜R4）を固定する。層は `lib.rs` の `cfg(target_arch = "wasm32")` つきの
//! `mod` 宣言から求める（読めない・空なら失敗）。状態経路の検査は `tools/check-deps.sh` ではなく本試験に置く。
//! DOM 読み取りは check-deps.sh、状態経路は本試験という分担を保ち、同じ規則を二か所に持たないため
//! （`make test` を通して CI の `rust-test` ジョブで流れる）。
//!
//! `glue.rs` は wasm32 でしかコンパイルされないため、native の試験はソースを文字列として読んで調べる。
//! 分担: crate 全体の DOM 読み取りの件数は `tools/check-deps.sh`（`make check-dom-reads`）が持ち、
//! 本試験は glue の状態経路（App に触れる口の数と、読んだ値の使い方）を固定する。
//! 解析は、コメントを除き、空白と `r#` を除いた文字列で照合する。曖昧な形は検出側に倒す（見逃さない）。

mod common;

use common::{
    count, fn_body, glue_code, glue_source, host_source, wasm_layer_module_names,
    wasm_layer_sources,
};

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

// ---- 層全体の規則（FEND-20。TASK-362 1/2） ----

/// 層のどこにも置けない形。`MUTATING` から glue 固有の `::dispatch_action(` を除き、`Runtime::hydrate` を足す。
/// `hydrate` は DOM の属性から Component の状態を復元し、`update` を通らない。
const LAYER_FORBIDDEN: &[&str] = &[
    ".update(",
    "::update(",
    "component_mut",
    ".borrow_mut().as_mut()",
    "apply_with_view_transition",
    "::hydrate(",
    ".hydrate(",
];

/// `EngineHost` に持たせない内部可変性の綴り。`with_app` が渡す `&EngineHost` で状態を変えられないことの前提。
const INTERIOR_MUT: &[&str] = &[
    "std::cell",
    "RefCell",
    "Cell<",
    "UnsafeCell",
    "OnceCell",
    "Mutex",
    "RwLock",
    "Atomic",
];

/// 本体の `Runtime` を持つ層のファイル。`RUNTIME` と `Runtime<EngineHost>` はここにだけ現れてよい。
const GLUE_FILE: &str = "glue";

/// `Runtime::mount(` の許可（ファイル, 関数）。件数は 1 件ずつ。
const MOUNT_ALLOWED: &[(&str, &str)] = &[
    ("glue", "mount_host("),
    ("test_hooks", "debug_view_fixture_mount("),
];

/// glue の外に許す `.dispatch_action(`（ファイル, 関数）。代役の部品の Runtime にだけ使う。
const DISPATCH_ALLOWED: (&str, &str) = ("test_hooks", "debug_view_fixture_dispatch(");
/// glue の外に許す `.component()`（ファイル, 関数）。読み取りの口。
const COMPONENT_ALLOWED: (&str, &str) = ("test_hooks", "debug_view_fixture_state(");

/// `&mut self` を取るメソッドを mount の前の初期化で呼ぶ許可（ファイル, 関数, メソッド名, 件数, 「これより前」の印）。
/// 名前の衝突で誤検出が出たら、理由つきでここに足す。
const INIT_ALLOWED: &[(&str, &str, &str, usize, &str)] = &[
    ("persist", "boot_and_mount(", "boot", 2, "glue::mount_host("),
    (
        "persist",
        "boot_and_mount(",
        "block_saving",
        1,
        "glue::mount_host(",
    ),
    (
        "persist",
        "boot_and_mount(",
        "set_history_saver",
        1,
        "glue::mount_host(",
    ),
    (
        "persist",
        "boot_and_mount(",
        "set_save_failure_probe",
        1,
        "glue::mount_host(",
    ),
    (
        "persist",
        "boot_and_mount(",
        "set_stale_probe",
        1,
        "glue::mount_host(",
    ),
    (
        "glue",
        "mount_host(",
        "set_tick_request",
        1,
        "Runtime::mount(",
    ),
    (
        "glue",
        "mount_host(",
        "set_tick_reset",
        1,
        "Runtime::mount(",
    ),
    (
        "glue",
        "mount_host(",
        "set_draft_sync_request",
        1,
        "Runtime::mount(",
    ),
    // 変更の後の遅延保存の通知先（TASK-318）。通知先はタイマーを予約するだけで、Runtime・App に触れない。
    (
        "glue",
        "mount_host(",
        "set_save_request",
        1,
        "Runtime::mount(",
    ),
    // 即時の保存が済んだときの予約の無効化（TASK-318）。通知先は畳み込みの状態を触るだけで、Runtime・App に触れない。
    (
        "glue",
        "mount_host(",
        "set_save_cancel",
        1,
        "Runtime::mount(",
    ),
];

/// `host.rs` から `fn <name>(&mut self` / `fn <name><..>(&mut self` の名前を求める（可視性は問わない）。
fn mut_self_methods(host_src: &str) -> Vec<String> {
    let code = common::strip_comments(host_src);
    let mut names: Vec<String> = Vec::new();
    for (i, _) in code.match_indices("fn ") {
        // `xfn ` のように識別子の一部なら飛ばす。
        if code[..i]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            continue;
        }
        let after = &code[i + 3..];
        let name: String = after
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() {
            continue;
        }
        let mut tail = after[name.len()..].trim_start();
        if tail.starts_with('<') {
            let mut depth = 0usize;
            let mut end = None;
            for (k, c) in tail.char_indices() {
                match c {
                    '<' => depth += 1,
                    '>' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(k + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(e) = end else { continue };
            tail = tail[e..].trim_start();
        }
        if let Some(params) = tail.strip_prefix('(') {
            if params.trim_start().starts_with("&mut self") && !names.contains(&name) {
                names.push(name);
            }
        }
    }
    names
}

fn layer_file<'a>(codes: &'a [(String, String)], file: &str) -> Option<&'a str> {
    codes
        .iter()
        .find(|(n, _)| n == file)
        .map(|(_, c)| c.as_str())
}

/// 層全体の規則 R1〜R4 の違反（空なら合格）。`files` は（モジュール名, 全文）。
fn layer_violations(files: &[(String, String)], host_src: &str) -> Vec<String> {
    let codes: Vec<(String, String)> = files
        .iter()
        .map(|(n, s)| (n.clone(), glue_code(s)))
        .collect();
    let mut v = Vec::new();

    // R1: 層のどのファイルにも `update` を通らずに状態を変える形が無い。
    for (n, c) in &codes {
        for p in LAYER_FORBIDDEN {
            let k = count(c, p);
            if k != 0 {
                v.push(format!("R1: {n} に {p} が {k} 箇所（0 のはず）"));
            }
        }
    }

    // R2: Runtime に触れる口を限る。
    for (n, c) in &codes {
        if n != GLUE_FILE {
            if c.contains("RUNTIME") || c.contains("Runtime<EngineHost>") {
                v.push(format!(
                    "R2: {n} が本体の Runtime に触れている（glue だけのはず）"
                ));
            }
            let (df, dfn) = DISPATCH_ALLOWED;
            let total = count(c, ".dispatch_action(") + count(c, "::dispatch_action(");
            let in_fn = if n == df {
                fn_body(c, dfn)
                    .filter(|b| b.contains("VIEW_FIXTURE.with"))
                    .map_or(0, |b| count(b, ".dispatch_action("))
            } else {
                0
            };
            let want = usize::from(n == df);
            if total != want || in_fn != want {
                v.push(format!(
                    "R2: {n} の dispatch_action が {total} 箇所（許可の内 {in_fn}。{want} 箇所のはず）"
                ));
            }
            let (cf, cfn) = COMPONENT_ALLOWED;
            let ctotal = component_calls(c);
            let cin = if n == cf {
                fn_body(c, cfn).map_or(0, component_calls)
            } else {
                0
            };
            let cwant = usize::from(n == cf);
            if ctotal != cwant || cin != cwant {
                v.push(format!(
                    "R2: {n} の component() が {ctotal} 箇所（許可の内 {cin}。{cwant} 箇所のはず）"
                ));
            }
        }
        let mounts = count(c, "Runtime::mount(");
        let in_fn: usize = MOUNT_ALLOWED
            .iter()
            .filter(|(f, _)| f == n)
            .map(|(_, func)| fn_body(c, func).map_or(0, |b| count(b, "Runtime::mount(")))
            .sum();
        let want = MOUNT_ALLOWED.iter().filter(|(f, _)| f == n).count();
        if mounts != want || in_fn != want {
            v.push(format!(
                "R2: {n} の Runtime::mount が {mounts} 箇所（許可の内 {in_fn}。{want} 箇所のはず）"
            ));
        }
    }

    // R3: `&mut self` のメソッドは mount の前の初期化（許可表）でだけ呼ぶ。
    let methods = mut_self_methods(host_src);
    if methods.is_empty() {
        v.push("R3: host.rs から &mut self のメソッドを 1 件も列挙できない".to_owned());
    }
    for (n, c) in &codes {
        for m in &methods {
            let pat = format!(".{m}(");
            // `EngineHost::boot(&mut host, ..)` の UFCS 形式も数える（R1・R2 と同じ網羅）。
            let total = count(c, &pat) + count(c, &format!("::{m}("));
            let allowed: usize = INIT_ALLOWED
                .iter()
                .filter(|(f, _, name, _, _)| f == n && name == m)
                .map(|(_, _, _, k, _)| *k)
                .sum();
            if total != allowed {
                v.push(format!(
                    "R3: {n} の {pat} が {total} 箇所（許可表では {allowed} 箇所のはず）"
                ));
            }
        }
    }
    for (f, func, m, k, before) in INIT_ALLOWED {
        let pat = format!(".{m}(");
        let Some(body) = layer_file(&codes, f).and_then(|c| fn_body(c, func)) else {
            v.push(format!("R3: {f} の {func} が無い"));
            continue;
        };
        if count(body, &pat) != *k {
            v.push(format!("R3: {f} の {func} の中の {pat} が {k} 箇所でない"));
        }
        match (body.rfind(&pat), body.find(before)) {
            (Some(last), Some(mount)) if last < mount => {}
            _ => v.push(format!(
                "R3: {f} の {func} で {pat} が {before} より前にない（mount の後に状態を変えている）"
            )),
        }
    }

    // R4: EngineHost に内部可変性が無い（with_app の共有参照で状態を変えられない前提）。
    let host_code = glue_code(host_src);
    for p in INTERIOR_MUT {
        let k = count(&host_code, p);
        if k != 0 {
            v.push(format!("R4: host.rs に {p} が {k} 箇所（0 のはず）"));
        }
    }
    v
}

/// 層に含まれていなければならないモジュール（下限の確認）。
const LAYER_FLOOR: &[&str] = &[
    "glue",
    "persist",
    "leave_flush",
    "storage_idb",
    "storage_local",
    "history_download",
    "test_hooks",
];

#[test]
fn 層は_lib_rs_の_cfg_から求まり下限の7モジュールを含む() {
    let names: Vec<String> = wasm_layer_sources().into_iter().map(|(n, _)| n).collect();
    for m in LAYER_FLOOR {
        assert!(names.iter().any(|n| n == m), "{m} が層に無い: {names:?}");
    }
}

#[test]
fn 層の求め方_cfg_と宣言の間の属性とコメントを読み飛ばす() {
    let lib = "#[cfg(target_arch = \"wasm32\")]\n/// doc\n#[allow(x)]\npub mod a;\n#[cfg(feature = \"f\")]\nmod b;\n#[cfg(all(target_arch = \"wasm32\", feature = \"t\"))]\nmod c;\nmod d;\n";
    assert_eq!(wasm_layer_module_names(lib), vec!["a", "c"]);
}

#[test]
fn host_の_mut_self_メソッドを列挙できる() {
    let names = mut_self_methods(&host_source());
    for m in ["boot", "block_saving", "open_history", "submit", "update"] {
        assert!(names.iter().any(|n| n == m), "{m} が無い: {names:?}");
    }
    let g = mut_self_methods("fn a<T>(&mut self, x: T) {} fn b(&self) {} // fn c(&mut self)\n");
    assert_eq!(g, vec!["a"]);
}

#[test]
fn 層全体は_update_を通らない状態の変更を持たない() {
    assert_eq!(
        layer_violations(&wasm_layer_sources(), &host_source()),
        Vec::<String>::new()
    );
}

const L_PERSIST: &str = r#"
fn boot_and_mount(root_id: &str, mut host: EngineHost, key: K) -> R {
    match read(&key) {
        Ok(s) => { host.boot(s); }
        Err(_) => { host.boot(None); host.block_saving(); }
    }
    host.set_history_saver(g);
    host.set_save_failure_probe(p);
    host.set_stale_probe(q);
    crate::glue::mount_host(root_id, host)?;
    install_save_on_leave(key, f)
}
"#;
const L_GLUE: &str = r#"
static RUNTIME: X = Y; // Runtime<EngineHost>
fn mount_host(root_id: &str, mut host: EngineHost) -> R {
    host.set_tick_request(a);
    host.set_tick_reset(b);
    host.set_draft_sync_request(c);
    host.set_save_request(d);
    host.set_save_cancel(e);
    let runtime = Runtime::mount(root_id, host)?;
    Ok(())
}
"#;
const L_HOOKS: &str = r#"
fn debug_view_fixture_mount(r: &str) -> String { match Runtime::mount(r, f) { _ => s } }
fn debug_view_fixture_dispatch(n: &str) -> String { VIEW_FIXTURE.with(|c| { rt.dispatch_action(n, p) }) }
fn debug_view_fixture_state() -> String { let f = rt.component(); s }
"#;
const L_LEAVE: &str = "fn flush() { // c.update(a)\n let x = 1; }";
const L_HOST: &str = "impl EngineHost { pub fn boot(&mut self, s: Option<&[u8]>) {} pub(crate) fn block_saving(&mut self) {} pub fn open_history(&mut self, b: &[u8]) {} fn set_history_saver(&mut self, f: F) {} fn set_save_failure_probe(&mut self, f: F) {} fn set_stale_probe(&mut self, f: F) {} fn set_tick_request(&mut self, f: F) {} fn set_tick_reset(&mut self, f: F) {} fn set_draft_sync_request(&mut self, f: F) {} fn set_save_request(&mut self, f: F) {} fn set_save_cancel(&mut self, f: F) {} }";

fn layer(persist: &str, glue: &str, hooks: &str, leave: &str) -> Vec<(String, String)> {
    [
        ("persist", persist),
        ("glue", glue),
        ("test_hooks", hooks),
        ("leave_flush", leave),
    ]
    .iter()
    .map(|(n, s)| ((*n).to_owned(), (*s).to_owned()))
    .collect()
}

fn has(v: &[String], needle: &str) -> bool {
    v.iter().any(|m| m.contains(needle))
}

#[test]
fn 層の負例_基準となる合成ソースは違反なし() {
    let good = layer(L_PERSIST, L_GLUE, L_HOOKS, L_LEAVE);
    assert_eq!(layer_violations(&good, L_HOST), Vec::<String>::new());
}

#[test]
fn 層の負例_mount_の後の_boot_を検出する() {
    let p = L_PERSIST.replace(
        "install_save_on_leave(key, f)",
        "host.boot(None); install_save_on_leave(key, f)",
    );
    let v = layer_violations(&layer(&p, L_GLUE, L_HOOKS, L_LEAVE), L_HOST);
    assert!(has(&v, "R3"), "{v:?}");
}

#[test]
fn 層の負例_許可表に無い_mut_self_の呼び出しを検出する() {
    let p = L_PERSIST.replace(
        "install_save_on_leave(key, f)",
        "host.open_history(b); install_save_on_leave(key, f)",
    );
    let v = layer_violations(&layer(&p, L_GLUE, L_HOOKS, L_LEAVE), L_HOST);
    assert!(has(&v, ".open_history("), "{v:?}");
}

#[test]
fn 層の負例_leave_flush_の_update_を検出する() {
    let l = "fn flush() { c.update(a); }";
    let v = layer_violations(&layer(L_PERSIST, L_GLUE, L_HOOKS, l), L_HOST);
    assert!(has(&v, "R1") && has(&v, ".update("), "{v:?}");
}

#[test]
fn 層の負例_hydrate_を検出する() {
    for call in ["Runtime::hydrate(id, c)", "rt.hydrate(id)"] {
        let l = format!("fn flush() {{ {call}; }}");
        let v = layer_violations(&layer(L_PERSIST, L_GLUE, L_HOOKS, &l), L_HOST);
        assert!(has(&v, "hydrate("), "{call}: {v:?}");
    }
}

#[test]
fn 層の負例_試験用の口の2件目の_dispatch_action_を検出する() {
    let h = format!("{L_HOOKS}\nfn extra() {{ rt.dispatch_action(n, p) }}");
    let v = layer_violations(&layer(L_PERSIST, L_GLUE, &h, L_LEAVE), L_HOST);
    assert!(has(&v, "R2") && has(&v, "dispatch_action"), "{v:?}");
}

#[test]
fn 層の負例_読み取りの口に紛れた_dispatch_action_を検出する() {
    let h = L_HOOKS.replace(
        "let f = rt.component();",
        "let f = rt.component(); rt.dispatch_action(n, p);",
    );
    let v = layer_violations(&layer(L_PERSIST, L_GLUE, &h, L_LEAVE), L_HOST);
    assert!(has(&v, "R2"), "{v:?}");
}

#[test]
fn 層の負例_glue_の外の_runtime_を検出する() {
    let l = "fn flush() { RUNTIME.with(|c| c); }";
    let v = layer_violations(&layer(L_PERSIST, L_GLUE, L_HOOKS, l), L_HOST);
    assert!(has(&v, "R2") && has(&v, "Runtime"), "{v:?}");
}

#[test]
fn 層の負例_mount_の増加を検出する() {
    let l = "fn flush() { Runtime::mount(r, h); }";
    let v = layer_violations(&layer(L_PERSIST, L_GLUE, L_HOOKS, l), L_HOST);
    assert!(has(&v, "Runtime::mount"), "{v:?}");
}

#[test]
fn 層の負例_host_の内部可変性を検出する() {
    let h = format!("{L_HOST} struct S {{ x: RefCell<u8> }}");
    let good = layer(L_PERSIST, L_GLUE, L_HOOKS, L_LEAVE);
    assert!(has(&layer_violations(&good, &h), "R4"));
}

#[test]
fn 層の負例_mut_self_が列挙できないと失敗する() {
    let good = layer(L_PERSIST, L_GLUE, L_HOOKS, L_LEAVE);
    assert!(has(&layer_violations(&good, "impl EngineHost {}"), "R3"));
}

#[test]
fn 層の負例_コメント内の一致は数えない() {
    let l = "fn flush() { /* c.update(a); Runtime::hydrate(x) */ // rt.dispatch_action(a)\n }";
    assert_eq!(
        layer_violations(&layer(L_PERSIST, L_GLUE, L_HOOKS, l), L_HOST),
        Vec::<String>::new()
    );
}
