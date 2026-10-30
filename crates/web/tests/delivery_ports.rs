//! 配信する設定の画面と書き出しに、試験用の口が 1 件も残らないことの検査（TASK-547 1/3。SHELL-35・BUILD-27）。
//!
//! 切り分けそのものは既にある（`debug_*` の書き出しは `lib.rs` の `mod test_hooks` の背後、`raw-trigger` は `host.rs` の
//! `test-hooks` の背後、移動ボタンと進行の隠し要素は両設定とも置かない）。本ファイルはそれを native の層で固定する。
//! 分担: `no_hidden_triggers.rs` は進行の隠し要素と `data-payload`、`host_contract.rs` は初期画面の `raw`、
//! 本ファイルは (1) `debug_` の書き出しの置き場と gate、移動ボタンの文言の不在を `src/**/*.rs` の静的検査で、
//! (2) CH-0〜CH-3 の 20 Step の各手の後の画面で 0 件（試験の設定では `raw-trigger` だけがちょうど 1 件）を動的に固定する。
//! TASK-547 2/3 の静的な部分も本ファイルが持つ。(3) 章の開始 `Control::Begin` の組み立てが `host.rs` の「次の章へ進む」の腕の 1 件だけであること、
//! (4) PoC 期の試験用クエリ 7 種を `static/app.js` がキーとして読まないこと。動的な部分（画面の要素の復号）は `next_chapter.rs`、
//! ブラウザでの確認は `tools/browser/query-inert.test.mjs`、URL を読まない構造の証明は `tick_interval.rs` が持つ。
//! 範囲外: ブラウザでの CH-0〜CH-3 の通しと部品・配信物の検査（3/3）、配信物の検査（TASK-82）。
//! 試験の設定の対照は `cargo test -p fandhe-workshop-web --features test-hooks --test delivery_ports` で手元で流す。
//! 試験では `unwrap` を使ってよい。

mod common;

#[path = "common/ch0_3_journey.rs"]
mod ch0_3_journey;

use ch0_3_journey::{journey, page, settle};
use common::{count, glue_code, web_src_sources};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_web::host::EngineHost;
use fandhe_workshop_web::parts::Component;

/// `debug_` の書き出しを置いてよい唯一のファイル。
const DEBUG_HOME: &str = "test_hooks.rs";

/// `lib.rs` の `mod test_hooks;` の gate（空白を詰めた形）。
const TEST_HOOKS_MOD: &str =
    "#[cfg(all(target_arch=\"wasm32\",feature=\"test-hooks\"))]modtest_hooks;";

/// 配信する設定で 0 件の隠し要素の名前（先頭 3 件は試験の設定でも 0 件）。
const HIDDEN: [&str; 4] = [
    "tick-trigger",
    "esc-trigger",
    "complete-trigger",
    "raw-trigger",
];

/// 受講者向けの Step 移動のボタンの文言（空白を詰めた形）。
const MOVE_LABEL: &str = "移動:";

/// 章の開始の組み立てを置いてはいけないファイル（試験の設定だけの口）。
const BEGIN_EXEMPT: [&str; 3] = ["test_hooks.rs", "test_fixture.rs", "view_fixture.rs"];

/// PoC 期の試験用クエリ 7 種。配信する設定の `app.js` はどれも読まない（読むのは `start` だけ）。
const TEST_QUERIES: [&str; 7] = [
    "persist", "store", "manual", "tick", "os", "osmode", "flood",
];

/// 違反を 1 件ずつ文にして返す。`src` は（src からの相対パス, 本文）。
fn violations(src: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut home = 0;
    for (rel, body) in src {
        let rel = rel.as_str();
        let code = glue_code(body);
        let defs = count(&code, "fndebug_");
        if rel == DEBUG_HOME {
            home += defs;
        } else if defs != 0 {
            out.push(format!(
                "{rel}: debug_ の関数が {DEBUG_HOME} の外に {defs} 件"
            ));
        }
        if rel != DEBUG_HOME && code.contains("js_name=\"debug_") {
            out.push(format!("{rel}: debug_ で始まる書き出しの名前"));
        }
        if rel == "lib.rs" {
            let gated = count(&code, TEST_HOOKS_MOD);
            let any = count(&code, "modtest_hooks;");
            if gated != 1 || any != 1 {
                out.push(format!(
                    "{rel}: mod test_hooks は gate つきの 1 件だけのはずが gate つき {gated} 件・全体 {any} 件"
                ));
            }
        }
        if code.contains(MOVE_LABEL) {
            out.push(format!("{rel}: 移動ボタンの文言"));
        }
        if code.contains("Control::GoTo") {
            out.push(format!("{rel}: Control::GoTo の組み立て"));
        }
    }
    if home == 0 {
        out.push(format!(
            "{DEBUG_HOME}: debug_ の関数が 0 件（検査の空振り）"
        ));
    }
    out
}

/// `Control::Begin(` の組み立てが、`host.rs` の `HostAction::NextChapter(` の腕の 1 件だけであることの違反。
fn begin_violations(src: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    let mut total = 0;
    for (rel, body) in src {
        if BEGIN_EXEMPT.contains(&rel.as_str()) {
            continue;
        }
        let code = glue_code(body);
        let n = count(&code, "Control::Begin(");
        total += n;
        if n == 0 {
            continue;
        }
        if rel != "host.rs" {
            out.push(format!("{rel}: Control::Begin の組み立てが {n} 件"));
            continue;
        }
        let mut from = 0;
        while let Some(i) = code[from..].find("Control::Begin(") {
            let at = from + i;
            let arm = code[..at].rfind("HostAction::").map(|j| &code[j..at]);
            if !arm.is_some_and(|a| a.starts_with("HostAction::NextChapter(")) {
                out.push(format!("{rel}: 次の章へ進む以外の腕に Control::Begin"));
            }
            from = at + 1;
        }
    }
    if total != 1 {
        out.push(format!(
            "Control::Begin の組み立ては全体で 1 件のはずが {total} 件"
        ));
    }
    out
}

/// `app.js` が 7 種の名前をキーとして読む形（引用符つき・`名前=`）の違反。
fn query_violations(js: &str) -> Vec<String> {
    let mut out = Vec::new();
    for n in TEST_QUERIES {
        for pat in [
            format!("\"{n}\""),
            format!("'{n}'"),
            format!("`{n}`"),
            format!("{n}="),
        ] {
            if js.contains(&pat) {
                out.push(format!("app.js: 試験用クエリ {n} を読む形"));
                break;
            }
        }
    }
    out
}

fn any(v: &[String], needle: &str) -> bool {
    v.iter().any(|m| m.contains(needle))
}

fn good_src() -> Vec<(String, String)> {
    let f = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    vec![
        f(
            "lib.rs",
            "#[cfg(all(target_arch = \"wasm32\", feature = \"test-hooks\"))]\nmod test_hooks;\npub mod host;",
        ),
        f(
            DEBUG_HOME,
            "#[wasm_bindgen]\npub fn debug_state_json() -> String { String::new() }",
        ),
        f("glue.rs", "#[wasm_bindgen]\npub fn mount_app() {}"),
    ]
}

fn with(file: &str, extra: &str) -> Vec<(String, String)> {
    let mut v = good_src();
    match v.iter_mut().find(|(r, _)| r == file) {
        Some((_, b)) => b.push_str(extra),
        None => v.push((file.to_owned(), extra.to_owned())),
    }
    v
}

#[test]
fn 実ソースは違反なし() {
    let v = violations(&web_src_sources());
    assert!(v.is_empty(), "違反: {v:#?}");
}

#[test]
fn 正当な形は違反にしない() {
    assert!(violations(&good_src()).is_empty());
}

#[test]
fn debug_の関数が置き場の外にあるのを検出する() {
    let v = violations(&with("glue.rs", "\npub fn debug_mount() {}"));
    assert!(any(&v, "glue.rs: debug_ の関数"), "{v:?}");
    let v = violations(&with(
        "glue.rs",
        "\n#[wasm_bindgen(js_name = \"debug_x\")]\nfn x() {}",
    ));
    assert!(any(&v, "debug_ で始まる書き出しの名前"), "{v:?}");
}

#[test]
fn test_hooks_の_gate_の欠けと重複を検出する() {
    let mut ungated = good_src();
    ungated[0].1 = "#[cfg(target_arch = \"wasm32\")]\nmod test_hooks;".to_owned();
    assert!(any(&violations(&ungated), "gate つき 0 件"));
    let v = violations(&with("lib.rs", "\npub mod test_hooks;"));
    assert!(any(&v, "全体 2 件"), "{v:?}");
}

#[test]
fn 検査が空振りしたのを検出する() {
    let v = violations(&[("lib.rs".to_owned(), good_src()[0].1.clone())]);
    assert!(any(&v, "空振り"), "{v:?}");
}

#[test]
fn 移動ボタンの文言と_goto_の組み立てを検出する() {
    let v = violations(&with("view.rs", "fn v() { let t = \"移動: st-1\"; }"));
    assert!(any(&v, "移動ボタンの文言"), "{v:?}");
    let v = violations(&with("view.rs", "fn v() { let c = Control::GoTo(x); }"));
    assert!(any(&v, "Control::GoTo"), "{v:?}");
}

#[test]
fn 章の開始の組み立ては_次の章へ進む_の腕の一件だけ() {
    let v = begin_violations(&web_src_sources());
    assert!(v.is_empty(), "違反: {v:#?}");
}

#[test]
fn 章の開始の組み立ての増加と腕の違いを検出する() {
    let host_ok =
        "fn u() { match a { HostAction::NextChapter(id) => { x(Control::Begin(id)); } _ => {} } }";
    let f = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    assert!(begin_violations(&[f("host.rs", host_ok)]).is_empty());
    let v = begin_violations(&[
        f("host.rs", host_ok),
        f("view.rs", "fn v() { Control::Begin(x); }"),
    ]);
    assert!(any(&v, "view.rs: Control::Begin"), "{v:?}");
    let bad = "fn u() { match a { HostAction::NextChapter(id) => {} HostAction::Other(id) => { x(Control::Begin(id)); } } }";
    assert!(any(&begin_violations(&[f("host.rs", bad)]), "以外の腕"));
    assert!(any(&begin_violations(&[f("host.rs", "fn u() {}")]), "0 件"));
    let two = format!("{host_ok} fn w() {{ Control::Begin(z); }}");
    assert!(any(&begin_violations(&[f("host.rs", &two)]), "全体で 1 件"));
    // 試験の設定だけの口は数えない。
    let v = begin_violations(&[
        f("host.rs", host_ok),
        f("test_hooks.rs", "Control::Begin(x)"),
    ]);
    assert!(v.is_empty(), "{v:?}");
}

#[test]
fn 配信する画面の_app_js_は試験用クエリ_7_種を読まない() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static/app.js");
    let js = std::fs::read_to_string(path).unwrap();
    assert!(
        js.contains(".getAll(\"start\")"),
        "検査の空振り: start を読む行が無い"
    );
    let v = query_violations(&js);
    assert!(v.is_empty(), "違反: {v:#?}");
}

#[test]
fn 試験用クエリを読む形を検出する() {
    for n in TEST_QUERIES {
        for js in [
            format!("p.get(\"{n}\")"),
            format!("p.getAll('{n}')"),
            format!("q.has(`{n}`)"),
            format!("u + \"?{n}=1\""),
        ] {
            assert_eq!(query_violations(&js).len(), 1, "{js}");
        }
    }
    assert!(query_violations("p.getAll(\"start\"); // persistent").is_empty());
}

/// 開始直後と各手（settle の後）の画面の HTML。
fn screens() -> Vec<String> {
    let mut h = page();
    let mut out = vec![render_component_html(&h)];
    for (mv, _, _, _) in journey() {
        mv.apply(&mut h);
        settle(&mut h);
        out.push(render_component_html(&h));
    }
    out
}

/// 廃止済みの 4 種（移動ボタン・3 種の trigger）は、どちらの設定でも 0 件。
fn assert_retired_absent(screens: &[String]) {
    for (i, s) in screens.iter().enumerate() {
        assert!(!s.contains("移動"), "画面 {i}: 移動ボタン");
        for name in &HIDDEN[..3] {
            assert!(!s.contains(name), "画面 {i}: {name}");
        }
    }
}

#[cfg(not(feature = "test-hooks"))]
#[test]
fn 配信する設定では二十_step_の全画面に試験用の口が無い() {
    let screens = screens();
    assert!(screens.len() > 20, "通しの画面の数: {}", screens.len());
    assert_retired_absent(&screens);
    for (i, s) in screens.iter().enumerate() {
        assert!(!s.contains("raw-trigger"), "画面 {i}: raw-trigger");
        assert!(
            !s.contains("data-action=\"raw\""),
            "画面 {i}: raw の action"
        );
    }
    assert!(EngineHost::decode_action("raw", "{\"control\":{\"begin\":\"s-1\"}}").is_none());
}

#[cfg(feature = "test-hooks")]
#[test]
fn 試験の設定の対照では_raw_trigger_だけがちょうど_1_件ある() {
    let screens = screens();
    assert!(screens.len() > 20, "通しの画面の数: {}", screens.len());
    assert_retired_absent(&screens);
    for (i, s) in screens.iter().enumerate() {
        assert_eq!(count(s, "raw-trigger"), 1, "画面 {i}: raw-trigger の件数");
        assert!(s.contains("data-action=\"raw\""), "画面 {i}: raw の action");
        assert!(s.contains("hidden"), "画面 {i}: hidden");
    }
    assert!(EngineHost::decode_action("raw", "{\"control\":{\"begin\":\"s-1\"}}").is_some());
}
