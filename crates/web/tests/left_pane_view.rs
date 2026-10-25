//! `pack.left_pane`（`world_list`・`view_tabs`）が左のビューに反映されることの契約（PACK-22。TASK-481 1/2）。
//!
//! `world_list` 2 値 × `view_tabs` 3 値の全 6 通りで、選択のボタンの種類・親の `hidden`・世界状態の行の件数を確かめ、
//! 宣言外の選択の無視（`declared`・`hidden`）・Step 移動での先頭への復帰・読めない設定の既定への fail-closed を固定する。
//! IR は `min-3step.json` に 3 種のビューと各 Step の宣言、`pack.left_pane` を足して作る。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::artifact_view::ViewKind;
use fandhe_workshop_web::engine_parts::{artifact_rows, standard_registry};
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn sample() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(testdata().join("contract/event-sequence.json")).unwrap())
        .unwrap()
}

fn host_with(left_pane: Option<serde_json::Value>) -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    if let Some(lp) = left_pane {
        v["pack"]["left_pane"] = lp;
    }
    v["views"] = serde_json::json!({
        "v-a": {"kind": "file_tree"},
        "v-b": {"kind": "diff"},
        "v-c": {"kind": "card_list"},
    });
    let decl = [
        ("main/st-1", serde_json::json!(["v-b", "v-c", "v-a"])),
        ("main/st-2", serde_json::json!(["v-c", "v-b"])),
        ("main/st-3", serde_json::json!([])),
    ];
    for node in v["nodes"].as_array_mut().unwrap() {
        for (uid, views) in &decl {
            if node["uid"] == *uid {
                node["step"]["views"] = views.clone();
            }
        }
    }
    let ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn setting(world: &str, tabs: &str) -> Option<serde_json::Value> {
    Some(serde_json::json!({"world_list": world, "view_tabs": tabs}))
}

/// 世界状態に行を作るため、契約の試料の操作列を流す。
fn feed_rows(host: &mut EngineHost) {
    let ops: Vec<Op> = sample()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e["op"].clone()).unwrap())
        .collect();
    for op in ops {
        host.update(HostAction::Engine(op));
    }
}

fn go(host: &mut EngineHost, uid: &str) {
    let op: Op = serde_json::from_value(serde_json::json!({"control": {"go_to": uid}})).unwrap();
    host.update(HostAction::Engine(op));
}

fn list_tag_and_body(html: &str, kind: ViewKind) -> (String, String) {
    let needle = format!("data-view=\"{}\"", kind.as_str());
    let at = html.find(&needle).unwrap();
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    let close = end + html[end..].find("</ul>").unwrap();
    (
        html[start..=end].to_owned(),
        html[end + 1..close].to_owned(),
    )
}

fn visible_kinds(html: &str) -> Vec<ViewKind> {
    ViewKind::ALL
        .into_iter()
        .filter(|k| !list_tag_and_body(html, *k).0.contains(" hidden"))
        .collect()
}

fn row_total(html: &str) -> usize {
    ViewKind::ALL
        .into_iter()
        .map(|k| list_tag_and_body(html, k).1.matches("data-path=").count())
        .sum()
}

fn button_payloads(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("data-testid=\"view-select-button\"") {
        let start = rest[..i].rfind('<').unwrap();
        let end = i + rest[i..].find('>').unwrap();
        let tag = &rest[start..=end];
        let payload = tag.split("data-payload=\"").nth(1).unwrap();
        out.push(payload.split('"').next().unwrap().to_owned());
        rest = &rest[end..];
    }
    out
}

fn select_parent_hidden(html: &str) -> bool {
    let at = html.find("data-testid=\"view-select\"").unwrap();
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    html[start..=end].contains(" hidden")
}

fn all_names() -> Vec<String> {
    ViewKind::ALL
        .iter()
        .map(|k| k.as_str().to_owned())
        .collect()
}

#[test]
fn r1_6_通りでボタンの種類と親の_hidden_が設定どおりになる() {
    for world in ["shown", "hidden"] {
        for tabs in ["all", "declared", "hidden"] {
            let host = host_with(setting(world, tabs));
            let html = render_component_html(&host);
            let (want, hidden): (Vec<String>, bool) = match tabs {
                "all" => (all_names(), false),
                "declared" => (
                    vec!["diff".into(), "card_list".into(), "file_tree".into()],
                    false,
                ),
                _ => (Vec::new(), true),
            };
            assert_eq!(button_payloads(&html), want, "{world}/{tabs}");
            assert_eq!(select_parent_hidden(&html), hidden, "{world}/{tabs}");
        }
    }
}

#[test]
fn r2_world_list_hidden_では_7_本の_list_の項目が_0_件で_shown_では行が出る() {
    for tabs in ["all", "declared", "hidden"] {
        let mut shown = host_with(setting("shown", tabs));
        feed_rows(&mut shown);
        let want = artifact_rows(shown.model().world(), shown.model().projection().marks()).len();
        assert!(want > 0, "試料の操作列が行を作らない");
        assert_eq!(row_total(&render_component_html(&shown)), want, "{tabs}");

        let mut hidden = host_with(setting("hidden", tabs));
        feed_rows(&mut hidden);
        let html = render_component_html(&hidden);
        assert_eq!(row_total(&html), 0, "{tabs}");
        for k in ViewKind::ALL {
            assert!(!list_tag_and_body(&html, k).1.contains("<li"), "{tabs}");
        }
        assert!(html.contains("part-artifact-consumed"));
    }
}

#[test]
fn r3_declared_と_hidden_では宣言外の選択が無視される() {
    for tabs in ["declared", "hidden"] {
        let mut host = host_with(setting("shown", tabs));
        let before = render_component_html(&host);
        host.update(HostAction::SelectView(ViewKind::TaskTree));
        assert!(host.dirty_fields().is_empty(), "{tabs}");
        assert_eq!(render_component_html(&host), before, "{tabs}");
    }
}

#[test]
fn r4_all_では宣言外の種類も選べるが指せる行にならない() {
    let mut host = host_with(setting("shown", "all"));
    feed_rows(&mut host);
    host.update(HostAction::SelectView(ViewKind::TaskTree));
    let html = render_component_html(&host);
    assert_eq!(visible_kinds(&html), vec![ViewKind::TaskTree]);
    let (_, body) = list_tag_and_body(&html, ViewKind::TaskTree);
    assert!(body.contains("data-path="));
    assert!(
        !body.contains("data-action"),
        "宣言外のビューの行は指せない"
    );
}

#[test]
fn r5_hidden_では宣言の_2_番目の選択が無視され先頭のままである() {
    let mut host = host_with(setting("shown", "hidden"));
    host.update(HostAction::SelectView(ViewKind::CardList));
    let html = render_component_html(&host);
    assert_eq!(visible_kinds(&html), vec![ViewKind::Diff]);
    host.update(HostAction::SelectView(ViewKind::Diff));
    assert_eq!(
        visible_kinds(&render_component_html(&host)),
        vec![ViewKind::Diff]
    );
}

#[test]
fn r6_step_を移ると先頭のビューに戻る() {
    for tabs in ["all", "declared"] {
        let mut host = host_with(setting("shown", tabs));
        host.update(HostAction::SelectView(ViewKind::FileTree));
        assert_eq!(
            visible_kinds(&render_component_html(&host)),
            vec![ViewKind::FileTree]
        );
        go(&mut host, "main/st-2");
        assert_eq!(
            visible_kinds(&render_component_html(&host)),
            vec![ViewKind::CardList],
            "{tabs}"
        );
        go(&mut host, "main/st-3");
        assert_eq!(
            visible_kinds(&render_component_html(&host)),
            vec![ViewKind::FileTree],
            "{tabs}"
        );
    }
}

#[test]
fn r7_読めない_left_pane_は既定として描く() {
    for bad in [
        serde_json::json!({"view_tabs": "bogus"}),
        serde_json::json!({"world_list": 3}),
        serde_json::json!("text"),
    ] {
        let mut host = host_with(Some(bad));
        feed_rows(&mut host);
        let html = render_component_html(&host);
        assert_eq!(button_payloads(&html), all_names());
        assert!(row_total(&html) > 0);
    }
}
