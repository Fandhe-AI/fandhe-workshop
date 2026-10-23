//! 配信する設定の画面に、進行・補完を届ける隠し要素と、進行の宛先を `data-payload` で届ける経路が無いことの
//! 静的な検査（TASK-349 2/2。FEND-4）。
//!
//! 進行（`tick`）と補完（`complete`）は `Runtime::dispatch_action` の口（`set_timeout` → `deliver`）だけから届く。
//! 隠し要素 `tick-trigger`・`esc-trigger`・`complete-trigger` を置いて `data-payload` を書き換えて click する経路は、
//! 拡張機能や注入スクリプトが進行の宛先を偽造できる経路になるため、ソースに作らせない。
//!
//! 分担: 配信用 WASM・JS・HTML の文字列検索は `tools/check-delivery.sh`（項目 9・10）が持ち、ここは `src/**/*.rs` と
//! 配信する `static/{app.js,index.html}` の静的検査を持つ。WASM の `data-payload` を数えないのは、左の成果物の
//! 指せる行（`data-action="ui"`。`artifact_view`）が正当に出力する属性名であり、同じ文字列リテラルは 1 つに
//! まとめられるため、件数では新しい経路を検出できないからである。そこで「経路」を、進行の宛先
//! （`tick`・`complete`・`esc`）を `data-action` に書く形、許可の外の `data-payload`、DOM への属性の書き込み、
//! 送信ボタンと左の行（`row_key_effect` の結果）以外への `.click()`、`static/app.js` の `blob:` の検査つきの 1 件（`STATIC_HREF_WRITE`）以外の `href` への代入と定義し、許可した箇所の件数を固定する（増えても減っても失敗する）。
//! `static/test-hooks.js` は試験の設定でだけ配信されるため対象外である。
//! 解析は、コメントを除き、空白と `r#` を除いた文字列で照合する（`common::glue_code` と同じ）。

mod common;

use common::{count, glue_code, web_src_sources};
use std::path::PathBuf;

/// 進行・補完・Esc を届ける隠し要素の名前。どのファイルにも置かない。
const HIDDEN_NAMES: [&str; 3] = ["tick-trigger", "esc-trigger", "complete-trigger"];

/// 進行の宛先を表す `data-action` の値（リテラルと定数名）。
const ROUTED_ACTIONS: [&str; 6] = [
    "\"tick\"",
    "\"complete\"",
    "\"esc\"",
    "ACTION_TICK",
    "ACTION_COMPLETE",
    "ACTION_ESC",
];

/// `-trigger"` で終わる文字列リテラルとして許す唯一の名前と、その置き場（試験の設定の背後）。
const RAW_TRIGGER: (&str, &str) = ("raw-trigger", "host.rs");

/// `data-payload` の出現の許可（相対パス, `"data-payload"` の件数, `PAYLOAD_ATTR` の件数）。表に無いファイルは 0 件。
const PAYLOAD_ALLOWED: [(&str, usize, usize); 9] = [
    // 枠の適合の検査器（TASK-511 2/2）が `data-payload` を読んで照合する。use の行 1 と読み取り 3。要素には書かない。
    ("shell.rs", 0, 4),
    // 属性名の定義 1 と、指せる行（`data-action="ui"`）での使用 1。
    ("artifact_view.rs", 1, 2),
    // 試験の設定でだけ出る `raw-trigger` の 1 件。
    ("host.rs", 1, 0),
    // 受講者が押す見える選択肢のボタン（TASK-445 2/2）と承認のボタン 2 種（TASK-447 1/2）。
    // use の行 1 と使用 3。進行の宛先は置かない。
    ("offers.rs", 0, 4),
    // 受講者が押す見えるビューの選択のボタン（TASK-470）。use の行 1 と使用 1。進行の宛先は置かない。
    ("engine_parts.rs", 0, 2),
    // 受講者が押す見えるタブ（TASK-517 2/2）。use の行 1 と使用 1。進行の宛先は置かない。
    ("shell_frame.rs", 0, 2),
    // 受講者が押す見える通知の閉じるボタン（TASK-535 1/2）。use の行 1 と使用 1。進行の宛先は置かない。
    ("shell_notice.rs", 0, 2),
    // 受講者が押す見える「次の章へ進む」のボタン（TASK-526 1/2）。use の行 1 と使用 1。payload は開始点の ID で、進行の宛先ではない。
    ("next_chapter.rs", 0, 2),
    // 受講者が押す見える保存・復帰のボタン（TASK-529 1/2）。use の行 1 と使用 2。payload は ID で、進行の宛先ではない。
    // ページ送りのボタン（FEND-22）は共通の関数 1 か所で使用が 1 増える。payload はページ番号で、進行の宛先ではない。
    ("checkpoint_ops.rs", 0, 4),
];

/// 配信する静的ファイルで禁止する文字列（コメントも含めて生の本文で照合する）。
const STATIC_FORBIDDEN: [&str; 3] = ["data-payload", ".click(", "setAttribute("];

/// 配信する `static/app.js` で許す唯一の属性の書き込み（TASK-531 1/2）。履歴の書き出しのリンクの `href` へ、
/// 自分のオリジンの `blob:` URL を 1 回写す `a.href = url` である。契約は次の 3 点。
/// 対象は書き出しのリンク（`data-testid="export-download"`）の `href` だけ。値は `blob:${window.origin}/` で始まる
/// 文字列だけ（上流の属性の検査が `blob:` の href を落とすため、URL は `data-export-url` に載って届く）。
/// リンクは履歴が変わるたびに view が作り直すので、写した値は次の描画で消える。件数は 1 件に固定する（増減で失敗する）。
const STATIC_HREF_WRITE: &str = ".href=";
const STATIC_HREF_WRITE_COUNT: usize = 1;
/// 上の書き込みが必ず通る前置きの検査（空白を除いた本文で照合する）。
const STATIC_HREF_GUARD: &str = "url.startsWith(blobPrefix)";

/// 違反を 1 件ずつ文にして返す。`src` は（src からの相対パス, 本文）、`statics` は（static からの相対パス, 本文）。
fn violations(src: &[(String, String)], statics: &[(String, String)]) -> Vec<String> {
    let mut out = Vec::new();
    for (rel, body) in src {
        let rel = rel.as_str();
        let code = glue_code(body);
        for name in HIDDEN_NAMES {
            if code.contains(name) {
                out.push(format!("{rel}: 隠し要素の名前 {name}"));
            }
        }
        // `-trigger"` で終わるリテラルは raw-trigger だけを許す（host.rs に 1 件）。
        let mut raw = 0;
        for (idx, _) in code.match_indices("-trigger\"") {
            let start = code[..idx].rfind('"').map_or(0, |p| p + 1);
            let lit = format!("{}-trigger", &code[start..idx]);
            if lit == RAW_TRIGGER.0 && rel == RAW_TRIGGER.1 {
                raw += 1;
            } else if !HIDDEN_NAMES.contains(&lit.as_str()) {
                out.push(format!("{rel}: 許可の外の trigger の名前 {lit}"));
            }
        }
        if rel == RAW_TRIGGER.1 {
            if raw != 1 {
                out.push(format!("{rel}: raw-trigger は 1 件のはずが {raw} 件"));
            } else if !code.contains("#[cfg(feature=\"test-hooks\")]children.push(") {
                out.push(format!("{rel}: raw-trigger が test-hooks の背後にない"));
            }
        }
        // `("data-action", "tick")` の形。`("data-action-keydown", ACTION_ESC)` は閉じ引用符の違いで一致しない。
        for head in ["(\"data-action\",", "(ACTION_ATTR,"] {
            for action in ROUTED_ACTIONS {
                if code.contains(&format!("{head}{action}")) {
                    out.push(format!(
                        "{rel}: 進行の宛先を data-action に置く {head}{action}"
                    ));
                }
            }
        }
        // 宛先の符号化の正は action.rs の RuntimeAction::encode だけ。
        if rel != "action.rs" {
            for id in ["ACTION_TICK", "ACTION_COMPLETE"] {
                if code.contains(id) {
                    out.push(format!("{rel}: {id} を action.rs の外で使う"));
                }
            }
        }
        let (lits, idents) = PAYLOAD_ALLOWED
            .iter()
            .find(|(f, _, _)| *f == rel)
            .map_or((0, 0), |(_, l, i)| (*l, *i));
        let got_lits = count(&code, "\"data-payload\"");
        let got_idents = count(&code, "PAYLOAD_ATTR");
        if got_lits != lits || got_idents != idents {
            out.push(format!(
                "{rel}: data-payload は {lits} 件・PAYLOAD_ATTR は {idents} 件のはずが {got_lits} 件・{got_idents} 件"
            ));
        }
        if code.contains("set_attribute(") {
            out.push(format!("{rel}: DOM への属性の書き込み set_attribute("));
        }
        let clicks = count(&code, ".click()");
        if rel == "glue.rs" {
            // 各 `.click()` の直前 300 文字に錨があること。送信ボタン（`SUBMIT_SELECTOR`）が 1 件、
            // 左の行（`click_row`。`row_key_effect` の結果）が 1 件のちょうど 2 件だけを許す（TASK-358 1/2）。
            let (mut submit, mut row) = (0, 0);
            for (p, _) in code.match_indices(".click()") {
                let near = &code[p.saturating_sub(300)..p];
                // 近い方の錨を採る（行の click の前に送信の処理が入りうるため）。
                match (near.rfind("SUBMIT_SELECTOR"), near.rfind("click_row")) {
                    (Some(a), Some(b)) if a > b => submit += 1,
                    (Some(_), None) => submit += 1,
                    (_, Some(_)) => row += 1,
                    (None, None) => {}
                }
            }
            if clicks != 2 || submit != 1 || row != 1 {
                out.push(format!(
                    "{rel}: .click() は送信ボタンへの 1 件と左の行への 1 件だけのはず（{clicks} 件。送信 {submit}・行 {row}）"
                ));
            }
        } else if clicks != 0 {
            out.push(format!("{rel}: .click() が {clicks} 件"));
        }
    }
    for (rel, body) in statics {
        for name in HIDDEN_NAMES.iter().chain(STATIC_FORBIDDEN.iter()) {
            if body.contains(name) {
                out.push(format!("static/{rel}: 禁止の文字列 {name}"));
            }
        }
        let squeezed: String = body.split_whitespace().collect();
        let writes = squeezed.matches(STATIC_HREF_WRITE).count();
        if rel.ends_with(".js") {
            let guarded = squeezed.contains(STATIC_HREF_GUARD.replace(' ', "").as_str());
            if writes != STATIC_HREF_WRITE_COUNT || !guarded {
                out.push(format!(
                    "static/{rel}: href への書き込みは blob: の検査つきの {STATIC_HREF_WRITE_COUNT} 件だけのはず（{writes} 件・検査 {guarded}）"
                ));
            }
        } else if writes != 0 {
            out.push(format!("static/{rel}: href への書き込み"));
        }
    }
    out
}

fn real_statics() -> Vec<(String, String)> {
    ["app.js", "index.html"]
        .iter()
        .map(|f| {
            let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("static")
                .join(f);
            let body = std::fs::read_to_string(&p).expect("static を読めない");
            ((*f).to_owned(), body)
        })
        .collect()
}

/// 許可の表を満たす最小の試料。BAD の試料はこれに 1 件足して作る。
fn good_src() -> Vec<(String, String)> {
    let f = |a: &str, b: &str| (a.to_owned(), b.to_owned());
    vec![
        f(
            "host.rs",
            concat!(
                "fn view() { let mut children = vec![]; ",
                "#[cfg(feature = \"test-hooks\")] children.push(el(\"button\", ",
                "vec![(\"data-testid\", \"raw-trigger\"), (\"data-action\", \"raw\"), (\"data-payload\", \"\")], vec![])); }",
            ),
        ),
        f(
            "artifact_view.rs",
            concat!(
                "pub const PAYLOAD_ATTR: &str = \"data-payload\"; pub const ACTION_ATTR: &str = \"data-action\"; ",
                "fn a() { let x = [(ACTION_ATTR, POINT_ACTION), (PAYLOAD_ATTR, p)]; }",
            ),
        ),
        f(
            "action.rs",
            concat!(
                "pub const ACTION_TICK: &str = \"tick\"; pub const ACTION_ESC: &str = \"esc\"; ",
                "pub const ESC_KEYDOWN_ATTRS: [(&str, &str); 2] = ",
                "[(\"data-action-keydown\", ACTION_ESC), (\"a\", \"b\")];",
            ),
        ),
        f(
            "glue.rs",
            concat!(
                "fn on_key() { if let Some(b) = find_in_root(&t, SUBMIT_SELECTOR) { b.click(); } } ",
                "fn on_row() { let e = row_key_effect(&i, r, true); if e.click_row { row.click(); } }",
            ),
        ),
    ]
}

/// 良い試料の `extra_file` に `extra` を足した一覧（ファイルが無ければ新しく置く）。
fn with(extra_file: &str, extra: &str) -> Vec<(String, String)> {
    let mut v = good_src();
    match v.iter_mut().find(|(r, _)| r == extra_file) {
        Some((_, b)) => b.push_str(extra),
        None => v.push((extra_file.to_owned(), extra.to_owned())),
    }
    v
}

fn any(v: &[String], needle: &str) -> bool {
    v.iter().any(|m| m.contains(needle))
}

#[test]
fn 実ソースは違反なし() {
    let v = violations(&web_src_sources(), &real_statics());
    assert!(v.is_empty(), "違反: {v:#?}");
}

#[test]
fn 正当な形は違反にしない() {
    // Esc の keydown の属性・指せる行・試験の設定の raw-trigger・送信ボタンへの click は許す。
    assert!(violations(&good_src(), &[]).is_empty());
}

#[test]
fn 隠し要素の名前を検出する() {
    for name in HIDDEN_NAMES {
        let extra = format!("fn v() {{ el(\"button\", vec![(\"data-testid\", \"{name}\")]); }}");
        let v = violations(&with("view.rs", &extra), &[]);
        assert!(any(&v, name), "{name} を検出できない: {v:?}");
    }
}

#[test]
fn data_action_と進行の宛先の組を検出する() {
    for extra in [
        "fn v() { let a = (\"data-action\", \"tick\"); }",
        "fn v() { let a = (\"data-action\", \"complete\"); }",
        "fn v() { let a = (ACTION_ATTR, \"esc\"); }",
        "fn v() { let a = (\"data-action\", ACTION_ESC); }",
    ] {
        let v = violations(&with("view.rs", extra), &[]);
        assert!(
            any(&v, "data-action に置く"),
            "{extra} を検出できない: {v:?}"
        );
    }
}

#[test]
fn 宛先の識別子が_action_rs_の外にあるのを検出する() {
    let v = violations(&with("view.rs", "fn v() { let a = ACTION_TICK; }"), &[]);
    assert!(any(&v, "action.rs の外"), "{v:?}");
}

#[test]
fn 許可の外の_data_payload_を検出する() {
    let v = violations(
        &with("view.rs", "fn v() { let a = (\"data-payload\", \"\"); }"),
        &[],
    );
    assert!(any(&v, "data-payload は 0 件"), "{v:?}");
    // 許可したファイルでも件数が増えれば失敗する。
    let v = violations(
        &with("host.rs", "fn w() { let a = (\"data-payload\", \"x\"); }"),
        &[],
    );
    assert!(any(&v, "host.rs: data-payload は 1 件"), "{v:?}");
}

#[test]
fn 許可の外の_trigger_の名前を検出する() {
    let v = violations(
        &with("view.rs", "fn v() { let a = \"other-trigger\"; }"),
        &[],
    );
    assert!(any(&v, "other-trigger"), "{v:?}");
}

#[test]
fn set_attribute_と_click_の追加を検出する() {
    let v = violations(
        &with(
            "view.rs",
            "fn v() { el.set_attribute(\"data-payload\", p); }",
        ),
        &[],
    );
    assert!(any(&v, "set_attribute("), "{v:?}");
    let v = violations(&with("view.rs", "fn v() { b.click(); }"), &[]);
    assert!(any(&v, "view.rs: .click() が 1 件"), "{v:?}");
    // 3 件目・錨の無い click・送信の click を消して 1 件になった場合も検出する。
    let v = violations(&with("glue.rs", "fn w() { other.click(); }"), &[]);
    assert!(any(&v, "glue.rs: .click()"), "{v:?}");
    let v = violations(
        &with("glue.rs", "fn w() { if e.click_row { x.click(); } }"),
        &[],
    );
    assert!(any(&v, "glue.rs: .click()"), "{v:?}");
    let only_row = vec![(
        "glue.rs".to_owned(),
        "fn on_row() { let e = row_key_effect(&i, r, true); if e.click_row { row.click(); } }"
            .to_owned(),
    )];
    let v = violations(&only_row, &[]);
    assert!(any(&v, "送信 0・行 1"), "{v:?}");
}

#[test]
fn 配信する静的ファイルの違反を検出する() {
    let s = |b: &str| vec![("app.js".to_owned(), b.to_owned())];
    for bad in [
        "const t = 'tick-trigger';",
        "el.setAttribute('data-payload', x);",
        "document.querySelector('x').click();",
        "a.href = url; b.href = 'x';",
    ] {
        let v = violations(&good_src(), &s(bad));
        assert!(any(&v, "static/app.js"), "{bad} を検出できない: {v:?}");
    }
}
