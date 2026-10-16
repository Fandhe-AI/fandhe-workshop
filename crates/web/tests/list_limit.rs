//! keyed list の項目数が上限（4,096 件）を超えたとき、`Err` になり全再描画にならないことの契約（TASK-359 2/2。
//! FEND-15・PoC-8）。
//!
//! 上流（`fandhe-frontend-wasm-full` 0.46.0）は、dirty な field が束縛点の対応表にも keyed list
//! （`[data-bind-list="<field>"]`）にも解決できないと、構造のフォールバックで `app-root` の子を丸ごと置き換える
//! （全再描画）。そのため `Err` の代替も `data-bind-list` を持つ親を保つ。本試験は次を固定する。
//! - 4,096 件は通り、4,097 件は `keyed_view` が `TooManyItems` で拒否する。
//! - 代替（`keyed_view_or_notice`）は `data-bind-list` を保った親を 1 つ描き、データ行は 0 件・通知の行は 1 件である。
//! - ホスト全体で、ログが 4,097 行になった更新の直後も、dirty な field がすべて描画結果の束縛点として解決でき
//!   （全再描画の条件が成り立たない）、部品の `section` と束縛点が残る。
//!
//! 失敗の文面には、field 名と件数だけを出す（教材由来の値を出さない）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::keyed::KeyedListError;
use fandhe_frontend_core::{el, render, text, Node};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::list_policy::{
    keyed_view, keyed_view_or_notice, ListError, LIST_ERROR_ATTR,
};
use fandhe_workshop_web::log_lines::line_from_event;
use fandhe_workshop_web::parts::{Component, DirtyTracked, FieldKind, PartField};

const LIST_FIELD: PartField = PartField {
    name: "rows",
    kind: FieldKind::KeyedList,
};

fn items(n: usize) -> Vec<(String, Node)> {
    (0..n)
        .map(|i| {
            (
                format!("r{i}"),
                el("li", vec![("data-event", "x")], vec![text("x")]),
            )
        })
        .collect()
}

fn count(html: &str, needle: &str) -> usize {
    html.matches(needle).count()
}

#[test]
fn limit_4096_items_pass_without_notice() {
    let node = keyed_view_or_notice("ul", vec![("data-testid", "t")], &LIST_FIELD, items(4_096));
    let html = render(&node);
    assert_eq!(count(&html, "<li"), 4_096);
    assert_eq!(count(&html, LIST_ERROR_ATTR), 0);
}

#[test]
fn limit_4097_items_are_rejected_with_too_many_items() {
    let err = keyed_view("ul", vec![], &LIST_FIELD, items(4_097)).unwrap_err();
    assert!(
        matches!(
            err,
            ListError::Upstream(KeyedListError::TooManyItems { count: 4_097 })
        ),
        "想定外の拒否: {err}"
    );
}

#[test]
fn limit_4097_fallback_keeps_bind_list_with_zero_data_rows() {
    let node = keyed_view_or_notice("ul", vec![("data-testid", "t")], &LIST_FIELD, items(4_097));
    let html = render(&node);
    assert_eq!(
        count(&html, "data-bind-list=\"rows\""),
        1,
        "親が 1 つでない"
    );
    assert_eq!(count(&html, "data-testid=\"t\""), 1);
    assert_eq!(count(&html, "data-list-error=\"limit\""), 2, "親と通知の行");
    assert_eq!(count(&html, "data-event="), 0, "データ行が残っている");
    assert_eq!(count(&html, "<li"), 1, "通知の行が 1 件でない");
}

fn begun_host() -> EngineHost {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let bytes =
        std::fs::read(root.join("engine-event-consistency/test-data/min-3step.json")).unwrap();
    let ir: Ir = load_ir(&bytes).unwrap();
    let sample: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("contract/event-sequence.json")).unwrap())
            .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

/// ログの部品に出る行の数。行になるのは `line_from_event` が写すイベントだけ（TASK-434）。
fn log_len(host: &EngineHost) -> usize {
    let events = host.model().session().unwrap().log();
    host.model()
        .projection()
        .log()
        .iter()
        .filter(|r| {
            line_from_event(&events[r.event as usize], host.model().judged_text(r.event)).is_some()
        })
        .count()
}

/// `data-bind-*` 属性の値（`field` または `field:...`）の最初の部分をすべて集める。
fn bound_names(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    for kind in ["list", "text", "attr", "class"] {
        let needle = format!("data-bind-{kind}=\"");
        let mut from = 0;
        while let Some(pos) = html[from..].find(&needle) {
            let start = from + pos + needle.len();
            let end = start + html[start..].find('"').unwrap();
            out.push(html[start..end].split(':').next().unwrap().to_owned());
            from = end;
        }
    }
    out
}

#[test]
fn limit_4097_host_log_does_not_trigger_full_rerender() {
    let mut host = begun_host();
    let op = || HostAction::Engine(Op::Input(Input::Text("x".to_owned())));
    // 入力（`Input::Text`）は 1 回で行が 2 件増える（入力の記録の行と、拒否の行。TASK-437）。
    // それを確かめながら、4,096 行まで進める（開始時の行数が偶数なら、ちょうど 4,096 に届く）。
    let mut guard = 0;
    while log_len(&host) < 4_096 {
        let before = log_len(&host);
        host.update(op());
        assert_eq!(log_len(&host), before + 2, "行が 2 件ずつ増えない");
        guard += 1;
        assert!(guard <= 5_000, "上限に届かない: {}", log_len(&host));
    }
    assert_eq!(log_len(&host), 4_096);
    let html = render_component_html(&host);
    assert_eq!(count(&html, "data-event="), 4_096);
    assert_eq!(count(&html, LIST_ERROR_ATTR), 0);

    host.update(op());
    assert!(log_len(&host) > 4_096);
    let html = render_component_html(&host);

    // 1. ログの親に印があり、データ行は 0 件。
    assert_eq!(count(&html, "data-event="), 0, "データ行が残っている");
    assert_eq!(count(&html, "data-list-error=\"limit\""), 2, "親と通知の行");
    // 2. KeyedList の field ごとに親がちょうど 1 つ。
    let mut fields: Vec<&str> = host
        .registry()
        .fields()
        .filter(|f| f.kind == FieldKind::KeyedList)
        .map(|f| f.name)
        .collect();
    fields.push("learner_log");
    for f in &fields {
        assert_eq!(
            count(&html, &format!("data-bind-list=\"{f}\"")),
            1,
            "親が 1 つでない: {f}"
        );
    }
    // 3. dirty な field がすべて束縛点として解決できる（構造のフォールバックに落ちない）。
    let names = bound_names(&html);
    for d in host.dirty_fields() {
        assert!(names.iter().any(|n| n == d), "解決できない dirty: {d}");
    }
    // 4. 部品の枠と束縛点が残る。
    assert_eq!(count(&html, "data-testid=\"part-log\""), 1);
    assert_eq!(count(&html, "data-testid=\"part-log-consumed\""), 1);
    assert_eq!(count(&html, "data-testid=\"part-log-list\""), 1);
}
