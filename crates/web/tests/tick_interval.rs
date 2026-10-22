//! 進行の間隔が 300ms の定数で、配信する経路が URL から変えられないことの試験（TASK-229。EVT-18・D-47）。
//!
//! 分担: 画面上で 300ms ごとに進むことの観察は、タイマー（TASK-349）と処理中の画面ができた後の TASK-453 が持つ。
//! 本試験は「値が 300 であること」と「配信する経路に URL を読む処理が無いこと」を機械的に固定する。
//! 照合はコメントを除き、空白を除いた形で行う。例外は 2 件だけ。(1) `persist.rs` の保存先の鍵を決めるパス名の読み取り
//! （クエリは読まない。TASK-319）。(2) `static/app.js` の直接リンクの読み取り（D-68・TASK-109。クエリの `start` だけを読み、
//! 値は Rust 側が IR の `starts` と完全一致で照合する）。試験の設定でだけ教材を選ぶクエリの読み取り（TASK-319 3/3）は、走査の対象外の `test_hooks.rs` に置く。
//! IR の場所は固定の `./ir/main.json` で、URL の `ir` は読まない（D-68・SAFE-33・TASK-168）。`ir` を読む実装は D-68 に反するため、例外として足さない。

mod common;

use common::glue_code;
use fandhe_workshop_web::tick_interval::TICK_INTERVAL_MS;
use std::path::PathBuf;

/// 配信する経路で禁止する字句（正規化後）。
const FORBIDDEN: &[&str] = &[
    "location",
    "URLSearchParams",
    ".search(",
    "::search(",
    "debug_set_tick",
    "set_tick_interval",
];

/// 例外: 履歴の鍵の区間を決めるパス名の読み取り（ファイル名、`location` の出現数、読み取りの形）。
/// 読むのは `pathname` だけで、クエリ（`search`）は読まない。
const LOCATION_EXCEPTION: (&str, usize, &str) = ("persist.rs", 1, ".location().pathname()");

/// 例外: `app.js` の直接リンクの読み取り（`location` の出現数・`URLSearchParams` の出現数・読み取りの形・読むキー）。
/// 読むのは `start` の 1 つだけで、ほかのキー・ハッシュは読まない。
const APP_JS_EXCEPTION: (usize, usize, &str, &str) = (
    1,
    1,
    "newURLSearchParams(location.search)",
    ".getAll(\"start\")",
);

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn hits(code: &str) -> Vec<&'static str> {
    FORBIDDEN
        .iter()
        .copied()
        .filter(|p| code.contains(p))
        .collect()
}

#[test]
fn 進行の間隔は_300ms_の定数である() {
    assert_eq!(TICK_INTERVAL_MS, 300);
}

#[test]
fn 配信する経路は_url_を読まない() {
    let mut scanned = 0;
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(root().join("src")).expect("src を読めない") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs")
            || path.file_name().is_some_and(|n| n == "test_hooks.rs")
        {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("読めない");
        scanned += 1;
        let code = glue_code(&src);
        let excused = path.file_name().is_some_and(|n| n == LOCATION_EXCEPTION.0)
            && code.matches("location").count() == LOCATION_EXCEPTION.1
            && code.contains(LOCATION_EXCEPTION.2);
        for h in hits(&code) {
            if excused && h == "location" {
                continue;
            }
            bad.push(format!("{}: {h}", path.display()));
        }
    }
    assert!(scanned >= 1, "走査したファイルが 0 件");
    let app = std::fs::read_to_string(root().join("static/app.js")).expect("app.js を読めない");
    let app_code = glue_code(&app);
    let app_excused = app_code.matches("location").count() == APP_JS_EXCEPTION.0
        && app_code.matches("URLSearchParams").count() == APP_JS_EXCEPTION.1
        && app_code.matches(APP_JS_EXCEPTION.2).count() == 1
        && app_code.matches(APP_JS_EXCEPTION.3).count() == 1
        // 読むキーは start の 1 つだけ（get・has・entries など他の読み方・ハッシュを持たない）。
        && !app_code.contains(".get(")
        && !app_code.contains(".has(")
        && !app_code.contains(".entries(")
        && !app_code.contains("location.hash")
        && !app_code.contains("location.href");
    for h in hits(&app_code) {
        if app_excused && matches!(h, "location" | "URLSearchParams") {
            continue;
        }
        bad.push(format!("app.js: {h}"));
    }
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn 試験の口は間隔を書き換えない() {
    let src = std::fs::read_to_string(root().join("src/test_hooks.rs")).expect("読めない");
    let code = glue_code(&src);
    assert!(code.contains("debug_tick_interval_ms"));
    for p in ["debug_set_tick", "set_tick_interval", "TICK_INTERVAL_MS="] {
        assert!(!code.contains(p), "{p}");
    }
}

/// IR の場所を URL の `ir` から作る字句（正規化後）。`app.js` は `ir` のキーを読まず、本番の Rust は `./ir/` を組み立てない。
const IR_QUERY_PATTERNS: &[&str] = &[
    "\"ir\"",
    "'ir'",
    "`ir`",
    "\"./ir/",
    "format!(\"ir/",
    "format!(\"./ir/",
];

fn ir_hits(code: &str) -> Vec<&'static str> {
    IR_QUERY_PATTERNS
        .iter()
        .copied()
        .filter(|p| code.contains(p))
        .collect()
}

/// `app.js` が同梱の IR を取得する固定の呼び出し（正規化後。TASK-112 2/2）。ファイルは `app.js` だけ・件数は 1 件に限り、
/// URL の値を含めない（リテラルの完全一致）。これ以外の `./ir/` の形は従来どおり禁止する。
const BUNDLED_IR_FETCH: &str = "fetch(\"./ir/main.json\")";

#[test]
fn ir_のクエリは読まず場所を組み立てない() {
    let app = std::fs::read_to_string(root().join("static/app.js")).expect("app.js を読めない");
    let app_code = glue_code(&app);
    assert_eq!(
        app_code.matches(BUNDLED_IR_FETCH).count(),
        1,
        "固定の取得がちょうど 1 件でない"
    );
    assert!(
        ir_hits(&app_code.replace(BUNDLED_IR_FETCH, "")).is_empty(),
        "app.js が ir を読む"
    );
    let mut scanned = 0;
    let mut bad = Vec::new();
    for entry in std::fs::read_dir(root().join("src")).expect("src を読めない") {
        let path = entry.expect("entry").path();
        if path.extension().is_none_or(|e| e != "rs")
            || path
                .file_name()
                .is_some_and(|n| n == "test_hooks.rs" || n == "test_fixture.rs")
        {
            continue;
        }
        scanned += 1;
        let src = std::fs::read_to_string(&path).expect("読めない");
        for h in ir_hits(&glue_code(&src)) {
            bad.push(format!("{}: {h}", path.display()));
        }
    }
    assert!(scanned >= 1, "走査したファイルが 0 件");
    assert!(bad.is_empty(), "{bad:?}");
}

#[test]
fn ir_の検査の字句は空振りしない() {
    assert_eq!(
        ir_hits(&glue_code("p.getAll(\"ir\"); fetch(\"./ir/x.json\");")).len(),
        2
    );
}

#[test]
fn 検査の字句は空振りしない() {
    assert_eq!(
        hits(&glue_code("let q = window.location.search();")).len(),
        2
    );
}
