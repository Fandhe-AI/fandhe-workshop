//! 製品の左エリアのアプリのデモ（TASK-799・D-100。#2058）。
//!
//! (b) の部品で行を描くこと、行の種類を属性 `kind` の 6 語の閉じた語彙だけで読み、語彙の外が文字のワイヤーになること、
//! パスワードの欄が出ないこと（ART-24）、指せる行の中の操作要素が行の click を邪魔しないこと、キーが `p<経路>`
//! （長い経路は `i<位置>`）であることを、製品の画面（`EngineHost`）で確かめる。
//! 期待値は手で数えた値で、実装の関数から導かない。試験では `unwrap` を使ってよい。

use std::path::{Path, PathBuf};

use fandhe_frontend_interactive::Component;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::artifact_view::ViewKind;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use serde_json::{json, Value};

fn pack_ir() -> Ir {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/product_view_pack");
    let bytes = compile_dir(&dir).unwrap_or_else(|d| panic!("compile: {} 件の診断", d.len()));
    load_ir(&bytes).unwrap()
}

fn demo_ir() -> Ir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch4_9/ir.json");
    load_ir(&std::fs::read(p).unwrap()).unwrap()
}

/// 行（経路と属性）を世界状態に置いた状態。`step` は現在の Step（`main/st-1` の置き換え先）。
fn state_with(step: &str, rows: &[(String, Value)]) -> SessionState {
    let testdata = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let sample: Value = serde_json::from_slice(
        &std::fs::read(testdata.join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let text = serde_json::to_string(&sample["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{step}\""));
    let mut initial: Value = serde_json::from_str(&text).unwrap();
    let log: Vec<Value> = rows
        .iter()
        .map(|(path, attrs)| {
            json!({"changed": {
                "at": {"generation": 1, "step": step},
                "path": path,
                "before": null,
                "after": {"fixture": null, "attrs": attrs},
                "cause": "input",
            }})
        })
        .collect();
    initial["log"] = Value::Array(log);
    serde_json::from_value(initial).unwrap()
}

fn host(ir: Ir, step: &str, rows: &[(String, Value)]) -> EngineHost {
    let mut h = EngineHost::with_session(
        Session::from_state(ir, state_with(step, rows)),
        standard_registry(),
    )
    .unwrap();
    h.update(HostAction::SelectView(ViewKind::AppDemo));
    h
}

/// アプリのデモの親の開始タグと本体。この種類の行の中には `ul` が無いので、次の `</ul>` までが本体。
fn list_of(h: &EngineHost) -> (String, String) {
    let html = render_component_html(h);
    let at = html.find("data-view=\"app_demo\"").unwrap();
    let open = html[..at].rfind("<ul").unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    (html[open..=end].to_owned(), html[end + 1..close].to_owned())
}

fn count(s: &str, pat: &str) -> usize {
    s.matches(pat).count()
}

fn row(path: &str, attrs: Value) -> (String, Value) {
    (path.to_owned(), attrs)
}

/// パスワードらしい値。固定の文字列を置かず、試験の中で組み立てる。
fn secret_like() -> String {
    format!("{}{}-{}", "hunter", 2, "secret")
}

fn mixed_rows() -> Vec<(String, Value)> {
    vec![
        row(
            "demo/a-heading",
            json!({"kind": "heading", "title": "見出し"}),
        ),
        row(
            "demo/b-tab",
            json!({"kind": "tab", "title": "タブ", "state": "active"}),
        ),
        row(
            "demo/c-field",
            json!({"kind": "field", "title": "名前", "detail": "山田"}),
        ),
        row(
            "demo/d-pw",
            json!({"kind": "field", "title": "パスワード", "detail": secret_like()}),
        ),
        row("demo/e-button", json!({"kind": "button", "title": "投稿"})),
        row(
            "demo/f-item",
            json!({"kind": "item", "title": "項目", "detail": "補足"}),
        ),
        row("demo/g-text", json!({"kind": "text", "title": "文字"})),
        row("demo/h-none", json!({"title": "属性なし"})),
        row("demo/i-card", json!({"kind": "card", "title": "カード"})),
        row(
            "demo/j-dir",
            json!({"kind": "dir", "title": "ディレクトリ"}),
        ),
        row("demo/k-upper", json!({"kind": "Field", "title": "大文字"})),
        row("demo/l-html", json!({"kind": "<b>x</b>", "title": "形"})),
    ]
}

#[test]
fn 行の種類は_6_語だけで読み語彙の外は文字のワイヤーになる() {
    let h = host(pack_ir(), "main/st-1", &mixed_rows());
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    assert_eq!(count(&body, "data-path="), 12, "{body}");
    // 6 語は自分の種類で出る。text は明示の 1 件に、属性なし・語彙の外の 5 件が加わって 6 件。
    for (pat, n) in [
        ("data-kind=\"heading\"", 1),
        ("data-kind=\"tab\"", 1),
        ("data-kind=\"field\"", 2),
        ("data-kind=\"button\"", 1),
        ("data-kind=\"item\"", 1),
        ("data-kind=\"text\"", 6),
    ] {
        assert_eq!(count(&body, pat), n, "{pat}: {body}");
    }
    // 語彙の外の値は属性にも文字にも出ない。
    for pat in [
        "data-kind=\"card\"",
        "data-kind=\"dir\"",
        "data-kind=\"Field\"",
        "<b>x</b>",
        "&lt;b&gt;",
    ] {
        assert_eq!(count(&body, pat), 0, "{pat}: {body}");
    }
    // 種類ごとの形。
    assert_eq!(count(&body, "class=\"app-demo-heading\""), 1, "{body}");
    assert_eq!(count(&body, "data-testid=\"app-demo-tab\""), 1, "{body}");
    assert_eq!(
        count(&body, "data-testid=\"app-demo-item-label\""),
        1,
        "{body}"
    );
    assert_eq!(
        count(&body, "data-testid=\"app-demo-item-detail\""),
        1,
        "{body}"
    );
    assert_eq!(count(&body, "<button"), 1, "{body}");
    assert_eq!(count(&body, "type=\"button\""), 1, "{body}");
    assert_eq!(count(&body, "<label"), 2, "{body}");
    assert!(body.contains("（選択中）"), "{body}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}

#[test]
fn 欄は読み取り専用の_text_だけでパスワードの欄は出ない() {
    let h = host(pack_ir(), "main/st-1", &mixed_rows());
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "<input"), 2, "{body}");
    assert_eq!(count(&body, " readonly=\"\""), 2, "{body}");
    assert_eq!(count(&body, "type=\"text\""), 2, "{body}");
    assert_eq!(count(&body, "type=\"password\""), 0, "{body}");
    assert_eq!(count(&body, "autocomplete"), 0, "{body}");
    // 値は `value` の中に文字として出るだけで、新しい要素にならない。
    assert_eq!(
        count(&body, &format!("value=\"{}\"", secret_like())),
        1,
        "{body}"
    );
    // 種類が欄でない行に password を名乗らせても欄は増えない。
    let rows = vec![row("demo/p", json!({"kind": "password", "title": "x"}))];
    let h = host(pack_ir(), "main/st-1", &rows);
    let (_, body) = list_of(&h);
    assert_eq!(count(&body, "<input"), 0, "{body}");
    assert_eq!(count(&body, "data-kind=\"text\""), 1, "{body}");
}

#[test]
fn キーは経路になり長い経路は位置に退き欄の_id_は位置から作られる() {
    let long = format!("demo/{}", "x".repeat(300));
    let rows = vec![
        row("demo/f", json!({"kind": "field", "title": "名前"})),
        row(long.as_str(), json!({"kind": "field", "title": "長い"})),
    ];
    let h = host(pack_ir(), "main/st-1", &rows);
    let (_, body) = list_of(&h);
    assert!(body.contains("data-key=\"pdemo/f\""), "{body}");
    assert!(body.contains("data-key=\"i1\""), "{body}");
    assert!(body.contains("id=\"app-demo-field-0-control\""), "{body}");
    assert!(body.contains("id=\"app-demo-field-1-control\""), "{body}");
    assert_eq!(count(&body, "id=\"pane-"), 0, "{body}");
}

#[test]
fn 指せる行の中の欄とボタンは行の_click_を妨げない() {
    let rows = vec![
        row("demo/name-pick", json!({"kind": "field", "title": "名前"})),
        row("demo/post", json!({"kind": "button", "title": "投稿"})),
    ];
    let h = host(demo_ir(), "main/st-8-1", &rows);
    let (head, body) = list_of(&h);
    assert!(!head.contains("role="), "{head}");
    assert_eq!(count(&body, "data-path="), 2, "{body}");
    // 宛先を引ける行はどちらも指せる行の 4 属性を持つ（ビューの宣言があり全行が宛先になる）。
    assert_eq!(count(&body, "data-action=\"ui\""), 2, "{body}");
    assert_eq!(count(&body, "data-row=\"demo/name-pick\""), 1, "{body}");
    assert_eq!(count(&body, "data-payload="), 2, "{body}");
    assert_eq!(count(&body, "tabindex="), 2, "{body}");
    // 内側の欄とボタンは操作の属性を持たず、disabled にもならない（disabled は click を発火しない）。
    assert_eq!(count(&body, "disabled"), 0, "{body}");
    let li_end = body.find("</li>").unwrap();
    let first = &body[..li_end];
    assert!(
        first.contains("<input") && first.contains("readonly"),
        "{first}"
    );
    assert_eq!(count(first, "data-action="), 1, "{first}");
    assert!(a11y::check_no_undefined(&h.view()).is_ok());
}
