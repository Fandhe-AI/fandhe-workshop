//! ビューごとの keyed list とビューの選択の契約（TASK-470 1/2。ART-5・ART-12・FEND-15）。
//!
//! 製品の `view()` が 7 本の keyed list を置き、選ばれていない list は空で `hidden` を持つこと、
//! `select-view` が今の Step の宣言に含まれる種類だけを受け、旧と新の束縛点を dirty にすることを確かめる。
//! IR は `min-3step.json` に複数のビュー（と一覧外の種類・未定義の ID）の宣言を足して作る。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::DirtyTracked;
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::Op;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::artifact_view::{list_attrs, ViewKind};
use fandhe_workshop_web::engine_parts::{artifact_rows, standard_registry};
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::view_select::ACTION_SELECT_VIEW;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn base_json() -> serde_json::Value {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

/// 3 種のビューと一覧外の種類 1 件を足し、各 Step の宣言を差し替えた IR。
fn multi_view_ir() -> Ir {
    let mut v = base_json();
    v["views"] = serde_json::json!({
        "v-a": {"kind": "file_tree"},
        "v-b": {"kind": "diff"},
        "v-c": {"kind": "card_list"},
        "v-x": {"kind": "not_a_view"},
    });
    let decl = [
        (
            "main/st-1",
            serde_json::json!(["v-b", "v-c", "v-x", "v-a", "v-none"]),
        ),
        ("main/st-2", serde_json::json!(["v-c"])),
        ("main/st-3", serde_json::json!([])),
    ];
    for node in v["nodes"].as_array_mut().unwrap() {
        for (uid, views) in &decl {
            if node["uid"] == *uid {
                node["step"]["views"] = views.clone();
            }
        }
    }
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

fn host_of(ir: Ir) -> EngineHost {
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

/// 描画した HTML から `data-view="<kind>"` を持つ開始タグを探し、(タグ全体, 閉じタグまでの本体) を返す。
fn view_list_html(html: &str, kind: ViewKind) -> (String, String) {
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

fn is_hidden(tag: &str) -> bool {
    tag.contains(" hidden")
}

fn rows_in(body: &str) -> usize {
    body.matches("data-path=").count()
}

fn buttons(html: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut rest = html;
    while let Some(i) = rest.find("data-testid=\"view-select-button\"") {
        let start = rest[..i].rfind('<').unwrap();
        let end = i + rest[i..].find('>').unwrap();
        let tag = &rest[start..=end];
        let payload = tag.split("data-payload=\"").nth(1).unwrap();
        let payload = payload.split('"').next().unwrap().to_owned();
        out.push((payload, tag.contains("data-selected=\"true\"")));
        rest = &rest[end..];
    }
    out
}

fn select(host: &mut EngineHost, kind: ViewKind) {
    host.update(HostAction::SelectView(kind));
}

fn list_tags(html: &str) -> Vec<(ViewKind, bool)> {
    ViewKind::ALL
        .iter()
        .map(|k| (*k, is_hidden(&view_list_html(html, *k).0)))
        .collect()
}

#[test]
fn 開始直後は宣言の先頭だけが見え_他の_6_本は空で_hidden() {
    let host = host_of(multi_view_ir());
    let html = render_component_html(&host);
    assert_eq!(html.matches("data-testid=\"view-list\"").count(), 7);
    for (kind, hidden) in list_tags(&html) {
        assert_eq!(hidden, kind != ViewKind::Diff, "{}", kind.as_str());
        let (_, body) = view_list_html(&html, kind);
        if kind == ViewKind::Diff {
            let want = artifact_rows(host.model().world(), host.model().projection().marks()).len();
            assert_eq!(rows_in(&body), want);
        } else {
            assert_eq!(rows_in(&body), 0, "{}", kind.as_str());
            assert!(!body.contains("<li"), "{}", kind.as_str());
        }
    }
    // ボタンは宣言の順で、一覧外の種類と未定義の ID は出ない。
    assert_eq!(
        buttons(&html),
        vec![
            ("diff".to_owned(), true),
            ("card_list".to_owned(), false),
            ("file_tree".to_owned(), false),
        ]
    );
    check_no_undefined(&host.view()).unwrap();
}

#[test]
fn 選ぶと旧と新の束縛点とボタンの_list_が_dirty_になり_描画が切り替わる() {
    let mut host = host_of(multi_view_ir());
    select(&mut host, ViewKind::CardList);
    assert_eq!(
        host.dirty_fields(),
        &["view_diff", "view_card_list", "view_select"]
    );
    let html = render_component_html(&host);
    for (kind, hidden) in list_tags(&html) {
        assert_eq!(hidden, kind != ViewKind::CardList, "{}", kind.as_str());
    }
    assert!(buttons(&html).contains(&("card_list".to_owned(), true)));
    check_no_undefined(&host.view()).unwrap();

    // 同じ種類をもう一度選んでも何も変わらない。
    select(&mut host, ViewKind::CardList);
    assert!(host.dirty_fields().is_empty());
}

#[test]
fn 宣言に無い種類の選択は無視され_dirty_を積まない() {
    let mut host = host_of(multi_view_ir());
    let before = render_component_html(&host);
    select(&mut host, ViewKind::TaskTree);
    assert!(host.dirty_fields().is_empty());
    assert_eq!(render_component_html(&host), before);
}

#[test]
fn session_が無いホストでは選んでも何も変わらない() {
    let mut host = EngineHost::new();
    select(&mut host, ViewKind::Diff);
    assert!(host.dirty_fields().is_empty());
    let html = render_component_html(&host);
    for (kind, hidden) in list_tags(&html) {
        assert_eq!(hidden, kind != ViewKind::FileTree, "{}", kind.as_str());
    }
    assert!(buttons(&html).is_empty());
    // ボタンの親は 0 件でも描かれ、hidden を持つ。
    let at = html.find("data-bind-list=\"view_select\"").unwrap();
    let start = html[..at].rfind('<').unwrap();
    let end = at + html[at..].find('>').unwrap();
    assert!(is_hidden(&html[start..=end]));
}

#[test]
fn select_view_の復号は綴りの完全一致だけを受ける() {
    for k in ViewKind::ALL {
        assert!(matches!(
            EngineHost::decode_action(ACTION_SELECT_VIEW, k.as_str()),
            Some(HostAction::SelectView(got)) if got == k
        ));
    }
    for bad in ["", "Diff", "diff ", " diff", "DIFF", "log", "file-tree"] {
        assert!(
            EngineHost::decode_action(ACTION_SELECT_VIEW, bad).is_none(),
            "{bad:?}"
        );
    }
}

#[test]
fn step_を移ると新しい_step_の宣言の先頭に戻る() {
    let mut host = host_of(multi_view_ir());
    select(&mut host, ViewKind::FileTree);
    let html = render_component_html(&host);
    assert!(!is_hidden(&view_list_html(&html, ViewKind::FileTree).0));
    let go: Op =
        serde_json::from_value(serde_json::json!({"control": {"go_to": "main/st-2"}})).unwrap();
    host.update(HostAction::Engine(go));
    let html = render_component_html(&host);
    // st-2 は card_list だけを宣言する。1 種だけなのでボタンは出ない。
    assert!(!is_hidden(&view_list_html(&html, ViewKind::CardList).0));
    assert!(buttons(&html).is_empty());
    let go: Op =
        serde_json::from_value(serde_json::json!({"control": {"go_to": "main/st-3"}})).unwrap();
    host.update(HostAction::Engine(go));
    let html = render_component_html(&host);
    // st-3 は宣言が無い。ファイルツリーになる。
    for (kind, hidden) in list_tags(&html) {
        assert_eq!(hidden, kind != ViewKind::FileTree, "{}", kind.as_str());
    }
}

#[test]
fn 親の属性列は選択と操作を繰り返しても_list_attrs_と一致する() {
    let mut host = host_of(multi_view_ir());
    let kinds = [
        ViewKind::CardList,
        ViewKind::FileTree,
        ViewKind::Diff,
        ViewKind::TaskTree,
    ];
    for k in kinds {
        select(&mut host, k);
        let html = render_component_html(&host);
        let selected = list_tags(&html)
            .into_iter()
            .find(|(_, h)| !h)
            .map(|(k, _)| k)
            .unwrap();
        for kind in ViewKind::ALL {
            let (tag, _) = view_list_html(&html, kind);
            for (name, value) in list_attrs(kind, kind == selected) {
                let want = if value.is_empty() {
                    format!(" {name}")
                } else {
                    format!("{name}=\"{value}\"")
                };
                assert!(tag.contains(&want), "{tag} / {want}");
            }
        }
    }
}
