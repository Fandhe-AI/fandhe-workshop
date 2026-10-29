//! 断った後に保存済みの履歴と画面が書き換わらないことのホストの層の試験（SAFE-40。TASK-169）。
//!
//! 履歴を何手か進めて「保存済みの履歴がある」状態にしてから、上限超過・入れ子・形の不一致などの入力を
//! `EngineHost::open_history` に渡し、断った後に次を確かめる。ブラウザでの保存先（localStorage・IndexedDB）と
//! DOM の比較は `tools/browser/refusal-keeps-saved.test.mjs` の担当である。
//!   - 状態・導出した値・保存する値（`history_to_save`）が前と同じで、保存の口が 1 回も呼ばれない。
//!   - 描いた画面（要素の文字と属性）に入力へ入れた番兵が出ない。
//!   - 通知の文が `import_refused_message` の固定の文と一致し、入力の違いで変わらない。
//!
//! 番兵は場合ごとに別の ASCII 文字列で、入力は試験の中で組み立てる。`tests/` は `no_progress.rs` の対象外で、
//! 試験では `unwrap` を使ってよい。失敗の文には理由のコード・件数・真偽だけを出す。

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::load_reason::LoadReason;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, ImportOutcome};
use fandhe_workshop_web::shell::NOTICE_TEXT_TESTID;
use fandhe_workshop_web::shell_notice::import_refused_message;
use serde_json::{json, Value};

const MAX_HISTORY: usize = 10 * 1024 * 1024;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn ir() -> Ir {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    load_ir(&bytes).unwrap()
}

fn initial() -> SessionState {
    let v: Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

/// 保存の口の呼び出し回数を数える口を結んだホスト。
fn host() -> (EngineHost, Rc<Cell<u32>>) {
    let mut h = EngineHost::with_session(Session::from_state(ir(), initial()), standard_registry())
        .unwrap();
    let calls = Rc::new(Cell::new(0));
    let c = Rc::clone(&calls);
    h.set_history_saver(Box::new(move |_| {
        c.set(c.get() + 1);
        Ok(())
    }));
    (h, calls)
}

/// 何手か進めて、保存する履歴がある状態にする。
fn advance(h: &mut EngineHost) {
    for (name, payload) in [("draft", "/make-note first-note"), ("submit", "")] {
        h.update(EngineHost::decode_action(name, payload).unwrap());
    }
    assert!(h.history_to_save().is_some(), "a saved history exists");
}

/// 開ける包み（試料の `ops` を適用し直した結果を写しにしたもの）の JSON。
fn loadable() -> Value {
    let ir = ir();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    serde_json::from_slice(&export_bytes(&built).unwrap()).unwrap()
}

fn nested(depth: usize, inner: &str) -> Vec<u8> {
    format!("{}\"{inner}\"{}", "[".repeat(depth), "]".repeat(depth)).into_bytes()
}

/// 断る入力の一覧（理由, 番兵, 入力）。番兵は入力のどこかに入っている。
fn refused_inputs() -> Vec<(LoadReason, &'static str, Vec<u8>)> {
    let mut large = b"SAFE40-SENTINEL-LARGE".to_vec();
    large.resize(MAX_HISTORY + 1, b' ');
    let mut malformed = loadable();
    malformed["ops"] = json!("SAFE40-SENTINEL-MALFORMED");
    let mut other = loadable();
    other["pack_id"] = json!("SAFE40-SENTINEL-OTHERPACK");
    let mut many = loadable();
    many["ops"] = json!(vec![json!({"draft": "SAFE40-SENTINEL-OPS"}); 20_001]);
    vec![
        (LoadReason::TooLarge, "SAFE40-SENTINEL-LARGE", large),
        (
            LoadReason::TooDeep,
            "SAFE40-SENTINEL-DEEP",
            nested(65, "SAFE40-SENTINEL-DEEP"),
        ),
        (
            LoadReason::NotJson,
            "SAFE40-SENTINEL-NOTJSON",
            b"SAFE40-SENTINEL-NOTJSON is not json".to_vec(),
        ),
        (
            LoadReason::MissingVersion,
            "SAFE40-SENTINEL-SHAPE",
            br#"{"SAFE40-SENTINEL-SHAPE":"SAFE40-SENTINEL-SHAPE"}"#.to_vec(),
        ),
        (
            LoadReason::Malformed,
            "SAFE40-SENTINEL-MALFORMED",
            serde_json::to_vec(&malformed).unwrap(),
        ),
        (
            LoadReason::OtherPack,
            "SAFE40-SENTINEL-OTHERPACK",
            serde_json::to_vec(&other).unwrap(),
        ),
        (
            LoadReason::TooManyOps,
            "SAFE40-SENTINEL-OPS",
            serde_json::to_vec(&many).unwrap(),
        ),
    ]
}

/// 木の文字と属性（名前と値）を 1 本の文字列にする（番兵の探索用）。
fn flatten(n: &Node, out: &mut String) {
    match n {
        Node::Text(t) => out.push_str(t),
        Node::Element {
            attrs, children, ..
        } => {
            for (k, v) in attrs {
                out.push_str(k);
                out.push_str(v);
            }
            for c in children {
                flatten(c, out);
            }
        }
        _ => {}
    }
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

fn text_only(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        Node::Element { children, .. } => children.iter().map(text_only).collect(),
        _ => String::new(),
    }
}

fn collect_texts(n: &Node, testid: &str, out: &mut Vec<String>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(text_only(n));
    }
    if let Node::Element { children, .. } = n {
        for c in children {
            collect_texts(c, testid, out);
        }
    }
}

fn notice_texts(h: &EngineHost) -> Vec<String> {
    let mut v = Vec::new();
    collect_texts(&h.view(), NOTICE_TEXT_TESTID, &mut v);
    v
}

#[test]
fn refusals_keep_state_saved_value_and_screen_free_of_input_values() {
    for (reason, sentinel, bytes) in refused_inputs() {
        let code = reason.as_str();
        let (mut h, calls) = host();
        advance(&mut h);
        let state = h.model().session().unwrap().state().clone();
        let projection = h.model().projection().clone();
        let world = h.model().world().clone();
        let saved = h.history_to_save();
        let calls_before = calls.get();
        // 前の操作の dirty を使い切ってから、断った結果だけを見る。
        let _ = h.dirty_fields();

        let outcome = h.open_history(&bytes);
        assert_eq!(outcome.reason(), Some(reason), "{code}");
        assert!(!outcome.is_loaded(), "{code}: loaded");
        assert!(matches!(outcome, ImportOutcome::Refused(_)), "{code}");
        assert_eq!(
            h.model().session().unwrap().state(),
            &state,
            "{code}: state"
        );
        assert_eq!(h.model().projection(), &projection, "{code}: projection");
        assert_eq!(h.model().world(), &world, "{code}: world");
        assert_eq!(h.history_to_save(), saved, "{code}: saved value");
        assert_eq!(calls.get(), calls_before, "{code}: saver called");

        let mut screen = String::new();
        flatten(&h.view(), &mut screen);
        assert!(!screen.contains(sentinel), "{code}: input value on screen");
        assert_eq!(
            notice_texts(&h),
            vec![import_refused_message(reason)],
            "{code}: notice text"
        );
    }
}

#[test]
fn notice_text_depends_only_on_the_code_not_on_the_input() {
    // 同じ理由で入力の中身だけが違う 2 本は、通知の文が一致する。
    let mut texts = Vec::new();
    for bytes in [
        nested(65, "SAFE40-SENTINEL-DEEP-A"),
        nested(70, "SAFE40-SENTINEL-DEEP-B"),
    ] {
        let (mut h, _) = host();
        h.open_history(&bytes);
        texts.push(notice_texts(&h));
    }
    assert_eq!(texts[0], texts[1]);
    assert_eq!(texts[0].len(), 1);
}

#[test]
fn every_reason_message_carries_its_code_and_a_fixed_head() {
    let head = |s: &str| s.split('（').next().unwrap().to_owned();
    let first = head(&import_refused_message(LoadReason::ALL[0]));
    for reason in LoadReason::ALL {
        let m = import_refused_message(reason);
        assert!(m.contains(reason.as_str()), "{}", reason.as_str());
        assert_eq!(head(&m), first, "{}: head differs", reason.as_str());
    }
}
