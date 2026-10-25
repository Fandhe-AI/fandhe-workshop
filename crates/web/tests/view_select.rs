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
    // 宣言の絞り込みを確かめる教材なので `view_tabs: declared` を書く（既定の `all` は 7 種を選べる）。
    v["pack"]["left_pane"] = serde_json::json!({"view_tabs": "declared"});
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

#[test]
fn 先頭以外を選んで別の_step_へ移り_戻ると先頭に戻る() {
    let mut host = host_of(multi_view_ir());
    select(&mut host, ViewKind::FileTree);
    let html = render_component_html(&host);
    assert!(!is_hidden(&view_list_html(&html, ViewKind::FileTree).0));
    for to in ["main/st-2", "main/st-1"] {
        let go: Op = serde_json::from_value(serde_json::json!({"control": {"go_to": to}})).unwrap();
        host.update(HostAction::Engine(go));
        let cur = host.model().session().unwrap().state().run.current.clone();
        assert_eq!(
            cur.map(|s| s.0),
            Some(to.to_owned()),
            "移動が受け付けられない"
        );
    }
    let html = render_component_html(&host);
    // st-1 の宣言の先頭（diff）に戻り、以前の選択（file_tree）は戻らない（ART-12）。
    for (kind, hidden) in list_tags(&html) {
        assert_eq!(hidden, kind != ViewKind::Diff, "{}", kind.as_str());
    }
    assert_eq!(
        buttons(&html),
        vec![
            ("diff".to_owned(), true),
            ("card_list".to_owned(), false),
            ("file_tree".to_owned(), false),
        ]
    );
}

/// 世界状態の経路が `n` 件になるログを持つホスト。ログの `changed` を導出が世界状態へ写す。
fn host_with_paths(ir: Ir, n: usize) -> EngineHost {
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let mut initial = sample["initial"].clone();
    let log: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            serde_json::json!({"changed": {
                "at": {"generation": 1, "step": "main/st-1"},
                "path": format!("p/{i}"),
                "before": null,
                "after": {"fixture": null, "attrs": {"kind": "none", "state": "created"}},
                "cause": "input",
            }})
        })
        .collect();
    initial["log"] = serde_json::Value::Array(log);
    let state: SessionState = serde_json::from_value(initial).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

#[test]
fn 行が上限を超えると選ばれたビューの_list_は先頭だけを出し状態の行で知らせる() {
    let host = host_with_paths(multi_view_ir(), 4_097);
    assert_eq!(host.model().world().len(), 4_097);
    let html = render_component_html(&host);
    let (tag, body) = view_list_html(&html, ViewKind::Diff);
    assert!(!is_hidden(&tag), "{tag}");
    assert!(tag.contains("data-list-limit=\"4097\""), "{tag}");
    assert!(!tag.contains("data-list-error"), "{tag}");
    assert!(tag.contains("data-bind-list=\"view_diff\""), "{tag}");
    assert!(body.contains("上限超過: 先頭 4,095 件だけ表示（全 4,097 件）"));
    assert_eq!(rows_in(&body), 4_095);
    // 選ばれていない list は空で、印も出さない。
    for kind in ViewKind::ALL {
        if kind != ViewKind::Diff {
            let (tag, body) = view_list_html(&html, kind);
            assert!(!tag.contains("data-list-error"), "{}", kind.as_str());
            assert!(!tag.contains("data-list-limit"), "{}", kind.as_str());
            assert!(!body.contains("<li"), "{}", kind.as_str());
        }
    }
}

#[test]
fn 行が上限の直前なら状態の行を出さない() {
    let host = host_with_paths(multi_view_ir(), 4_095);
    let html = render_component_html(&host);
    let (tag, body) = view_list_html(&html, ViewKind::Diff);
    assert!(!tag.contains("data-list-limit"), "{tag}");
    assert_eq!(rows_in(&body), 4_095);
}

#[test]
fn 行が上限ちょうどなら通知を出さない() {
    let host = host_with_paths(multi_view_ir(), 4_096);
    let html = render_component_html(&host);
    let (tag, body) = view_list_html(&html, ViewKind::Diff);
    assert!(!tag.contains("data-list-error"), "{tag}");
    assert!(!tag.contains("data-list-limit"), "{tag}");
    assert_eq!(rows_in(&body), 4_096);
}

fn dirty_after(host: &mut EngineHost, op: serde_json::Value) -> Vec<&'static str> {
    let op: Op = serde_json::from_value(op).unwrap();
    host.update(HostAction::Engine(op));
    host.dirty_fields().to_vec()
}

fn view_lists_in(dirty: &[&str]) -> Vec<String> {
    dirty
        .iter()
        .filter(|n| n.starts_with("view_") && **n != "view_select")
        .map(|n| (*n).to_owned())
        .collect()
}

fn text_op(text: &str) -> serde_json::Value {
    serde_json::json!({"input": {"text": text}})
}

#[test]
fn 同じ_step_とビューで中身が変わると選ばれたビューの_list_だけが_dirty_になる() {
    let mut host = host_of(multi_view_ir());
    let dirty = dirty_after(&mut host, text_op("hello"));
    assert_eq!(
        view_lists_in(&dirty),
        vec!["view_diff".to_owned()],
        "{dirty:?}"
    );
    assert!(!dirty.contains(&"view_select"), "{dirty:?}");
    // ログの list など、ビューの外の list は従来どおり積まれる。
    assert!(dirty.len() > 1, "{dirty:?}");
}

#[test]
fn step_が変わりビューの種類が変わると旧と新の_list_とボタンが_dirty_になる() {
    let mut host = host_of(multi_view_ir());
    let dirty = dirty_after(
        &mut host,
        serde_json::json!({"control": {"go_to": "main/st-2"}}),
    );
    let mut got = view_lists_in(&dirty);
    got.sort();
    assert_eq!(
        got,
        vec!["view_card_list".to_owned(), "view_diff".to_owned()],
        "{dirty:?}"
    );
    assert!(dirty.contains(&"view_select"), "{dirty:?}");
}

/// st-2 と st-3 がどちらも card_list だけを宣言する IR（Step が変わっても種類が同じ経路）。
fn same_kind_ir() -> Ir {
    let mut v = base_json();
    v["views"] = serde_json::json!({"v-c": {"kind": "card_list"}});
    for node in v["nodes"].as_array_mut().unwrap() {
        node["step"]["views"] = serde_json::json!(["v-c"]);
    }
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

#[test]
fn step_が変わっても種類が同じなら_その_list_とボタンだけが_dirty_になる() {
    let mut host = host_of(same_kind_ir());
    let dirty = dirty_after(
        &mut host,
        serde_json::json!({"control": {"go_to": "main/st-2"}}),
    );
    assert_eq!(
        view_lists_in(&dirty),
        vec!["view_card_list".to_owned()],
        "{dirty:?}"
    );
    assert!(dirty.contains(&"view_select"), "{dirty:?}");
}

#[test]
fn 続けて別の_step_へ移ると_その都度_旧と新の_list_とボタンが_dirty_になる() {
    let mut host = host_of(multi_view_ir());
    dirty_after(
        &mut host,
        serde_json::json!({"control": {"go_to": "main/st-2"}}),
    );
    // st-3 は宣言が無く、既定の file_tree になる。st-2 から移ると種類が変わる。
    let dirty = dirty_after(
        &mut host,
        serde_json::json!({"control": {"go_to": "main/st-3"}}),
    );
    assert_eq!(view_lists_in(&dirty).len(), 2, "{dirty:?}");
    assert!(dirty.contains(&"view_select"), "{dirty:?}");
}

#[test]
fn 絞り込みの後も描画の_hidden_と行は選ばれたビューと一致する() {
    let mut host = host_of(multi_view_ir());
    for op in [
        text_op("hello"),
        serde_json::json!({"control": {"go_to": "main/st-2"}}),
        text_op("again"),
    ] {
        dirty_after(&mut host, op);
        let html = render_component_html(&host);
        let shown = host.model().selected_view();
        for (kind, hidden) in list_tags(&html) {
            assert_eq!(hidden, kind != shown, "{}", kind.as_str());
        }
        check_no_undefined(&host.view()).unwrap();
    }
}

#[cfg(feature = "test-hooks")]
mod views_fixture {
    use super::*;
    use fandhe_workshop_web::host::{FixtureName, RAW_LOAD_FIXTURE};

    #[test]
    fn load_fixture_views_は復号され_st_1_で_3_種のボタンが出る() {
        let action = EngineHost::decode_action("raw", &format!("{RAW_LOAD_FIXTURE}views")).unwrap();
        assert!(matches!(
            action,
            HostAction::LoadFixture(FixtureName::Views)
        ));
        let mut host = EngineHost::new();
        host.update(action);
        let html = render_component_html(&host);
        assert_eq!(
            buttons(&html),
            vec![
                ("file_tree".to_owned(), true),
                ("card_list".to_owned(), false),
                ("check_list".to_owned(), false),
            ]
        );
        // st-1 -> st-2 は既定が変わり、st-2 -> st-3 は card_list のまま（1 本とボタン）。
        let mut dirty = dirty_after(
            &mut host,
            serde_json::json!({"control": {"go_to": "main/st-2"}}),
        );
        assert_eq!(view_lists_in(&dirty).len(), 2, "{dirty:?}");
        assert!(dirty.contains(&"view_select"), "{dirty:?}");
        dirty = dirty_after(
            &mut host,
            serde_json::json!({"control": {"go_to": "main/st-3"}}),
        );
        assert_eq!(
            view_lists_in(&dirty),
            vec!["view_card_list".to_owned()],
            "{dirty:?}"
        );
        assert!(dirty.contains(&"view_select"), "{dirty:?}");
        for bad in ["load-fixture:Views", "load-fixture:view"] {
            assert!(EngineHost::decode_action("raw", bad).is_none(), "{bad}");
        }
    }
}
