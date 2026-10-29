//! 壊れた履歴 7 種（SAVE-20 の B-1〜B-7）をホストへ読み込ませたときの動作の試験（TASK-332 1/2、#880）。
//!
//! 7 種はどれも `ImportOutcome::Refused` で、理由コードが表（`testdata/broken-histories/expectations.json`）と一致し、
//! 通知欄に空でない理由が 1 件出て、状態・ログ・下書き・導出した値・保存の値が変わらず、表示だけのモードにもならない。
//! 作り方は core の `broken_histories.rs` と同じ規則を `serde_json::Value` の書き換えで短く再現する（crate をまたいで部品を共有できないため）。
//! 失敗の文には ID・件数・真偽・固定のコードだけを出す（履歴の中身を出さない）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::shell;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir_json(version: &str) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    serde_json::to_vec(&v).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn host(version: &str) -> EngineHost {
    let ir = load_ir(&ir_json(version)).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn operate(h: &mut EngineHost) {
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
}

fn snapshot(h: &EngineHost) -> (String, usize, String) {
    let s = h.model().session().unwrap();
    (
        serde_json::to_string(s.state()).unwrap(),
        s.log().len(),
        h.draft().to_owned(),
    )
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    if let Node::Element { children, .. } = n {
        for c in children {
            find(c, testid, out);
        }
    }
}

fn all<'a>(n: &'a Node, testid: &str) -> Vec<&'a Node> {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    v
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(text_of).collect(),
        _ => String::new(),
    }
}

const IDS: [&str; 7] = ["B-1", "B-2", "B-3", "B-4", "B-5", "B-6", "B-7"];
const LIMIT: usize = 10 * 1024 * 1024;

fn table_codes() -> Vec<(String, String)> {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("broken-histories/expectations.json")).unwrap(),
    )
    .unwrap();
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| IDS.contains(&r["id"].as_str().unwrap()))
        .map(|r| {
            (
                r["id"].as_str().unwrap().to_owned(),
                r["expect"]["code"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}

fn edited(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    edit(&mut v);
    serde_json::to_vec(&v).unwrap()
}

fn make(id: &str) -> Vec<u8> {
    match id {
        "B-1" => edited(|v| {
            let ops = v["ops"].as_array_mut().unwrap();
            let op = ops.iter_mut().find(|o| o.get("submit").is_some()).unwrap();
            op["submit"] = serde_json::json!("/make-note other-note");
        }),
        "B-2" => {
            let b = history("1.0.0");
            b[..b.len() * 55 / 100].to_vec()
        }
        "B-3" => {
            let s = String::from_utf8(history("1.0.0")).unwrap();
            let at = s.find("\"ops\":[").unwrap() + "\"ops\":[".len();
            format!("{}}}{}", &s[..at], &s[at..]).into_bytes()
        }
        "B-4" => {
            let base = edited(|v| v["padding"] = serde_json::json!("")).len();
            let fill = "x".repeat(LIMIT + 1 - base);
            let out = edited(|v| v["padding"] = serde_json::json!(fill));
            assert_eq!(out.len(), LIMIT + 1);
            out
        }
        "B-5" => edited(|v| {
            v.as_object_mut().unwrap().remove("versions");
        }),
        "B-6" => edited(|v| v["pack_id"] = serde_json::json!("other-tutorial")),
        "B-7" => edited(|v| v["snapshot"]["run"]["generation"] = serde_json::json!("one")),
        other => panic!("unknown id {other}"),
    }
}

#[test]
fn 表の理由コードが定数と一致する() {
    let want: Vec<(String, String)> = [
        ("B-1", "replay_mismatch"),
        ("B-2", "truncated"),
        ("B-3", "not_json"),
        ("B-4", "too_large"),
        ("B-5", "missing_version"),
        ("B-6", "other_pack"),
        ("B-7", "malformed"),
    ]
    .iter()
    .map(|(a, b)| ((*a).to_owned(), (*b).to_owned()))
    .collect();
    assert_eq!(table_codes(), want);
}

#[test]
fn 壊れた履歴_7_種はどれも開かず_通知が出て_状態も保存も変わらない() {
    for (id, code) in table_codes() {
        let mut h = host("1.0.0");
        operate(&mut h);
        let before = snapshot(&h);
        let saved = h.history_to_save();
        let projection = h.model().projection().clone();
        let world = h.model().world().clone();

        let outcome = h.open_history(&make(&id));
        let ImportOutcome::Refused(got) = outcome else {
            panic!("{id}: not refused");
        };
        assert_eq!(got.as_str(), code, "{id}");
        assert_eq!(snapshot(&h), before, "{id}");
        assert_eq!(h.model().projection(), &projection, "{id}");
        assert_eq!(h.model().world(), &world, "{id}");
        assert_eq!(h.history_to_save(), saved, "{id}");
        assert_eq!(h.model().view_only_reason(), None, "{id}");

        let view = h.view();
        let item = all(&view, shell::NOTICE_ITEM_TESTID);
        assert_eq!(item.len(), 1, "{id}");
        assert_eq!(
            attr(item[0], "data-notice-kind"),
            Some("import-refused"),
            "{id}"
        );
        assert_eq!(
            attr(item[0], "data-notice-reason"),
            Some(code.as_str()),
            "{id}"
        );
        assert!(!text_of(item[0]).trim().is_empty(), "{id}");
        assert!(shell::check_complete_v2(&view).is_ok(), "{id}");
        assert!(a11y::check_no_undefined(&view).is_ok(), "{id}");
    }
}

#[test]
fn 拒否で積む_field_は通知欄だけ() {
    for (id, _) in table_codes() {
        let mut h = host("1.0.0");
        h.open_history(&make(&id));
        assert_eq!(h.dirty_fields(), vec!["shell_notices"], "{id}");
    }
}

#[test]
fn 対照の基準の履歴は開く() {
    let mut h = host("1.0.0");
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    assert_eq!(items_of_refused(&h), 0);
}

fn items_of_refused(h: &EngineHost) -> usize {
    all(&h.view(), shell::NOTICE_ITEM_TESTID)
        .into_iter()
        .filter(|n| attr(n, "data-notice-kind") == Some("import-refused"))
        .count()
}
