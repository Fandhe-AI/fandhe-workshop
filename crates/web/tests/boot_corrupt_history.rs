//! 起動時に壊れた履歴を検出し保存を止める動作の試験（SAVE-21・SHELL-17。TASK-333 1/2、#883）。
//!
//! 壊れた履歴 7 種（B-1〜B-7）を保存済みの値として `boot` に渡すと、(1) 開かず新しい状態で始まり、(2) 通知欄に
//! `corrupt-history` が 1 件だけ理由つきで出て、(3) 閉じる操作は無く、(4) 操作しても保存の値が出ない（上書きしない）。
//! 「捨てて始める」の処理は 2/2（TASK-333 2/2）なので、ここでは復号されないことまでを固定する。
//! 作り方は `broken_histories_host.rs` と同じ。失敗の文には ID・件数・真偽・固定のコードだけを出す。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::{a11y, shell};

const IDS: [&str; 7] = ["B-1", "B-2", "B-3", "B-4", "B-5", "B-6", "B-7"];
const LIMIT: usize = 10 * 1024 * 1024;
const SENTINEL: &str = "SENTINEL-ZQ-7781";

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

fn host() -> EngineHost {
    let ir = load_ir(&ir_json("1.0.0")).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json("1.0.0")).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    let mut v: serde_json::Value = serde_json::from_slice(&export_bytes(&built).unwrap()).unwrap();
    if version != "1.0.0" {
        // 内容の主版を上げた履歴（表示だけで開く）。
        let major: u64 = version.split('.').next().unwrap().parse().unwrap();
        v["versions"]["content"] = serde_json::json!(format!("{major}.0.0"));
    }
    serde_json::to_vec(&v).unwrap()
}

fn edited(edit: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    edit(&mut v);
    serde_json::to_vec(&v).unwrap()
}

/// B-1〜B-7 の作り方。どれも番兵の文字列を履歴の中に含める（画面に出ないことの確認用）。
fn make(id: &str) -> Vec<u8> {
    match id {
        "B-1" => edited(|v| {
            let ops = v["ops"].as_array_mut().unwrap();
            let op = ops.iter_mut().find(|o| o.get("submit").is_some()).unwrap();
            // 未知の欄を足すと malformed が先に出るので、番兵は入れない（置換後の値を番兵にする）。
            op["submit"] = serde_json::json!(format!("/make-note {SENTINEL}"));
        }),
        "B-2" => {
            let b = edited(|v| v["sentinel"] = serde_json::json!(SENTINEL));
            b[..b.len() * 55 / 100].to_vec()
        }
        "B-3" => {
            let s =
                String::from_utf8(edited(|v| v["sentinel"] = serde_json::json!(SENTINEL))).unwrap();
            let at = s.find("\"ops\":[").unwrap() + "\"ops\":[".len();
            format!("{}}}{}", &s[..at], &s[at..]).into_bytes()
        }
        "B-4" => {
            let base = edited(|v| v["padding"] = serde_json::json!("")).len();
            let fill = format!(
                "{SENTINEL}{}",
                "x".repeat(LIMIT + 1 - base - SENTINEL.len())
            );
            let out = edited(|v| v["padding"] = serde_json::json!(fill));
            assert_eq!(out.len(), LIMIT + 1);
            out
        }
        "B-5" => edited(|v| {
            v.as_object_mut().unwrap().remove("versions");
            v["sentinel"] = serde_json::json!(SENTINEL);
        }),
        "B-6" => edited(|v| {
            v["pack_id"] = serde_json::json!(SENTINEL);
        }),
        "B-7" => edited(|v| {
            v["snapshot"]["run"]["generation"] = serde_json::json!(SENTINEL);
        }),
        other => panic!("unknown id {other}"),
    }
}

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

fn items_of(h: &EngineHost, kind: &str) -> usize {
    let view = h.view();
    all(&view, shell::NOTICE_ITEM_TESTID)
        .into_iter()
        .filter(|n| attr(n, "data-notice-kind") == Some(kind))
        .count()
}

#[test]
fn 壊れた履歴_7_種は起動時に開かず_corrupt_history_が理由つきで出て_新しい状態で始まる() {
    let fresh = {
        let mut f = host();
        assert_eq!(f.boot(None), BootKind::Fresh);
        snapshot(&f)
    };
    for (id, code) in table_codes() {
        let mut h = host();
        assert_eq!(h.boot(Some(&make(&id))), BootKind::NotOpened, "{id}");
        assert_eq!(snapshot(&h), fresh, "{id}: 新しい状態で始まる");
        assert_eq!(h.model().view_only_reason(), None, "{id}");

        let view = h.view();
        let items = all(&view, shell::NOTICE_ITEM_TESTID);
        assert_eq!(items.len(), 1, "{id}");
        assert_eq!(
            attr(items[0], "data-notice-kind"),
            Some("corrupt-history"),
            "{id}"
        );
        assert_eq!(
            attr(items[0], "data-notice-reason"),
            Some(code.as_str()),
            "{id}"
        );
        let text = text_of(items[0]);
        assert!(!text.trim().is_empty(), "{id}");
        assert!(!text.contains(SENTINEL), "{id}: 履歴の値が画面に出た");
        assert!(!format!("{view:?}").contains(SENTINEL), "{id}");
        assert_eq!(items_of(&h, "import-refused"), 0, "{id}");
        assert_eq!(all(&view, "shell-discard-start").len(), 1, "{id}");
        assert!(text.contains("保存された履歴を捨てて始める"), "{id}");
        assert_eq!(all(&view, "shell-dismiss").len(), 0, "{id}");
        assert!(shell::check_complete_v2(&view).is_ok(), "{id}");
        assert!(a11y::check_no_undefined(&view).is_ok(), "{id}");
    }
}

#[test]
fn 壊れた履歴の起動の後は操作しても保存の値が出ない() {
    for (id, _) in table_codes() {
        let mut h = host();
        h.boot(Some(&make(&id)));
        assert!(h.history_to_save().is_none(), "{id}: 起動直後");
        operate(&mut h);
        assert!(h.history_to_save().is_none(), "{id}: 操作の後");
    }
}

#[test]
fn 閉じる操作と_捨てて始める_の_action_は何も変えない() {
    let mut h = host();
    h.boot(Some(&make("B-7")));
    let _ = h.dirty_fields();
    h.update(act("dismiss-notice", "corrupt-history"));
    assert_eq!(items_of(&h, "corrupt-history"), 1);
    assert!(!h.dirty_fields().contains(&"shell_notices"));
    assert!(EngineHost::decode_action("discard-and-start", "").is_none());
}

#[test]
fn 壊れた起動の後に開ける履歴を読み込んでも通知と保存の停止は残る() {
    let mut h = host();
    h.boot(Some(&make("B-2")));
    assert_eq!(h.open_history(&history("1.0.0")), ImportOutcome::Open);
    assert_eq!(items_of(&h, "corrupt-history"), 1);
    operate(&mut h);
    assert!(h.history_to_save().is_none());
}

#[test]
fn 壊れた起動の後に読み込みが拒否されると_2_件が順に出て_後者は閉じられない() {
    let mut h = host();
    h.boot(Some(&make("B-2")));
    assert!(matches!(
        h.open_history(&make("B-5")),
        ImportOutcome::Refused(_)
    ));
    let view = h.view();
    let kinds: Vec<&str> = all(&view, shell::NOTICE_ITEM_TESTID)
        .into_iter()
        .filter_map(|n| attr(n, "data-notice-kind"))
        .collect();
    assert_eq!(kinds, vec!["corrupt-history", "import-refused"]);
    assert_eq!(all(&view, "shell-dismiss").len(), 0);
}

#[test]
fn 版の判定の段の起動時の拒否は_import_refused_だけで出る() {
    let mut h = host();
    let mut v: serde_json::Value = serde_json::from_slice(&history("1.0.0")).unwrap();
    let n = v["versions"]["ir_format"].as_u64().unwrap();
    v["versions"]["ir_format"] = serde_json::json!(n + 1);
    assert_eq!(
        h.boot(Some(&serde_json::to_vec(&v).unwrap())),
        BootKind::NotOpened
    );
    assert_eq!(items_of(&h, "import-refused"), 1);
    assert_eq!(items_of(&h, "corrupt-history"), 0);
}

#[test]
fn 表示だけの起動と開ける起動と保存なしの起動では出ない() {
    let mut h = host();
    assert_eq!(h.boot(Some(&history("2.0.0"))), BootKind::NotOpened);
    assert_eq!(items_of(&h, "corrupt-history"), 0);

    let mut h = host();
    assert_eq!(h.boot(Some(&history("1.0.0"))), BootKind::Restored);
    assert_eq!(items_of(&h, "corrupt-history"), 0);

    let mut h = host();
    assert_eq!(h.boot(None), BootKind::Fresh);
    assert_eq!(items_of(&h, "corrupt-history"), 0);
}
