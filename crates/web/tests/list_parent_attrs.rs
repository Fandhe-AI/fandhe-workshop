//! keyed list の親要素の属性が `view()` に書かれていることの契約（TASK-359 1/2。FEND-15）。
//!
//! 上流は keyed list の更新のたびに親要素の属性を `view()` の列へ同期し、無い属性を消す。そのため親の属性は
//! `list_policy::keyed_view` の `attrs` に書く。本試験は次の 3 点を固定する。
//! - `src/` で上流の `keyed_list` を呼ぶのは `list_policy.rs` だけである（迂回を許さない）。
//! - 標準の登録簿の KeyedList field は、どれも親要素がちょうど 1 つ描かれる（部品が増えると対象も増える）。
//! - 3 部品と入力欄の送信済み一覧の親の属性列が、操作の前後で変わらない。
//!
//! 失敗の文面には、ファイル名・行番号・field 名だけを出す（教材由来の値を出さない）。試験では `unwrap` を使ってよい。

mod common;

use std::path::PathBuf;

use common::{strip_comments, web_src_sources};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::{Component, FieldKind};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn sample() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(testdata().join("contract/event-sequence.json")).unwrap())
        .unwrap()
}

fn begun_host() -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let ir: Ir = load_ir(&bytes).unwrap();
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn sample_ops() -> Vec<Op> {
    sample()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e["op"].clone()).unwrap())
        .collect()
}

/// 描画した HTML から、`data-bind-list="<field>"` を持つ開始タグの属性列（`名前=値`。整列済み）を全部返す。
fn parent_attrs(html: &str, field: &str) -> Vec<Vec<String>> {
    let needle = format!("data-bind-list=\"{field}\"");
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = html[from..].find(&needle) {
        let at = from + pos;
        let start = html[..at].rfind('<').unwrap();
        let end = at + html[at..].find('>').unwrap();
        let tag = &html[start + 1..end];
        let mut attrs: Vec<String> = Vec::new();
        let mut rest = tag.split_once(char::is_whitespace).map_or("", |x| x.1);
        while let Some((name, tail)) = rest.split_once("=\"") {
            let (value, after) = tail.split_once('"').unwrap();
            attrs.push(format!("{}={value}", name.trim()));
            rest = after;
        }
        attrs.sort();
        out.push(attrs);
        from = end;
    }
    out
}

#[test]
fn 上流の_keyed_list_を呼ぶのは_list_policy_だけである() {
    let mut violations = Vec::new();
    for (rel, src) in web_src_sources() {
        if rel == "list_policy.rs" {
            continue;
        }
        for (i, line) in strip_comments(&src).lines().enumerate() {
            let compact: String = line.chars().filter(|c| !c.is_whitespace()).collect();
            if compact.contains("keyed_list(") || compact.contains("keyed::keyed_list") {
                violations.push(format!("{rel}:{}", i + 1));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "keyed_view を通していない: {violations:?}"
    );
}

#[test]
fn 登録簿の_keyed_list_field_はどれも親要素がちょうど_1_つ描かれる() {
    let host = begun_host();
    let html = render_component_html(&host);
    let mut fields: Vec<&str> = host
        .registry()
        .fields()
        .filter(|f| f.kind == FieldKind::KeyedList)
        .map(|f| f.name)
        .collect();
    fields.push("learner_log");
    assert!(fields.len() >= 4, "{fields:?}");
    for f in fields {
        assert_eq!(parent_attrs(&html, f).len(), 1, "親が 1 つでない: {f}");
    }
}

#[test]
fn 親の属性列は_view_に書かれ_操作の前後で変わらない() {
    // 既定の教材（`view_tabs: all`）はビューを宣言しないため、ファイルツリーが選ばれ、残り 6 本は hidden。選択のボタンは 7 種が出るので親は hidden にならない。
    let mut expected: Vec<(&str, Vec<String>)> = [
        ("engine_log_rows", vec!["data-testid=part-log-list"]),
        ("review_items", vec!["data-testid=part-review-list"]),
        ("learner_log", vec!["data-testid=log-list"]),
        ("view_select", vec!["data-testid=view-select"]),
    ]
    .into_iter()
    .map(|(f, a)| (f, a.into_iter().map(str::to_owned).collect()))
    .collect();
    for kind in fandhe_workshop_web::artifact_view::ViewKind::ALL {
        let mut attrs = vec![
            "data-testid=view-list".to_owned(),
            format!("data-view={}", kind.as_str()),
        ];
        if kind != fandhe_workshop_web::artifact_view::ViewKind::FileTree {
            attrs.push("hidden=".to_owned());
        }
        expected.push((kind.list_field(), attrs));
    }
    let check = |host: &EngineHost| {
        let html = render_component_html(host);
        for (field, attrs) in &expected {
            let mut want: Vec<String> = attrs.clone();
            want.push(format!("data-bind-list={field}"));
            want.sort();
            assert_eq!(parent_attrs(&html, field), vec![want], "{field}");
        }
    };
    let mut host = begun_host();
    check(&host);
    for op in sample_ops() {
        host.update(HostAction::Engine(op));
        check(&host);
    }
}
