//! 書き出しに未送信の下書きが入るかの口（`EngineHost::export_includes_unsent_draft`。SAVE-18・TASK-329）の試験。
//!
//! 確かめるのは、(1) 下書きで真・送信後と空文字列で偽、(2) 真のときの書き出しのバイト列が同じ判定で真、
//! (3) 表示だけで開いた履歴では読み込んだバイト列の判定と一致し、表示だけを抜けると控えが残らないこと、
//! (4) チェックポイントの保存を挟んで消した下書きは、入力欄が空でも真で、挟んでも送った下書きは偽（D-111）。
//! 下書きは受講者のデータのため、失敗の文には値を出さない。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;

use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::{bytes_include_unsent_draft, export_bytes};
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::history_export::ExportGate;
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};

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

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// `version` の教材で書いた履歴。`draft` を渡すと末尾に下書きの記録を足す。
fn history(version: &str, draft: Option<&str>) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let mut ops = env.ops;
    if let Some(d) = draft {
        ops.push(HistoryOp::Draft(d.to_owned()));
    }
    let state = replay(&ir, initial(), &ops).unwrap();
    export_bytes(&build_envelope(&ir, &env.start, ops, state).unwrap()).unwrap()
}

#[test]
fn 下書きで真になり_書き出しのバイト列も真_送信後は偽() {
    let mut h = host("1.0.0");
    h.boot(None);
    assert!(!h.export_includes_unsent_draft());
    h.update(act("draft", "memo draft text"));
    assert!(h.export_includes_unsent_draft());
    let ExportGate::Ready(bytes) = h.export_live() else {
        panic!("書き出しが Ready でない");
    };
    assert!(bytes_include_unsent_draft(&bytes));
    // 拒否された送信は入力欄に下書きを残す（`keeps_draft`）ため、受け付けられる入力で送る。
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert!(!h.export_includes_unsent_draft());
}

#[test]
fn 空文字列の下書きは偽() {
    let mut h = host("1.0.0");
    h.boot(None);
    h.update(act("draft", "memo draft text"));
    h.update(act("draft", ""));
    assert!(!h.export_includes_unsent_draft());
}

#[test]
fn 保存を挟んで消した下書きは真_挟んでも送った下書きは偽() {
    let mut h = host("1.0.0");
    h.boot(None);
    h.update(act("draft", "memo draft text"));
    h.update(act("save-checkpoint", ""));
    h.update(act("draft", ""));
    assert!(
        h.export_includes_unsent_draft(),
        "消した下書きが ops に残る"
    );
    let ExportGate::Ready(bytes) = h.export_live() else {
        panic!("書き出しが Ready でない");
    };
    assert!(bytes_include_unsent_draft(&bytes));
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert!(
        v["snapshot"]["drafts"]
            .as_object()
            .unwrap()
            .values()
            .all(|d| d.as_str() == Some("")),
        "対照: 写しに空でない下書きが無い"
    );

    let mut h = host("1.0.0");
    h.boot(None);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("save-checkpoint", ""));
    h.update(act("submit", ""));
    assert!(!h.export_includes_unsent_draft(), "送った下書きは数えない");
}

#[test]
fn 表示だけの履歴では読み込んだバイト列の判定と一致し_抜けると控えが残らない() {
    for draft in [None, Some("memo draft text")] {
        let mut h = host("1.0.0");
        h.boot(None);
        let bytes = history("2.0.0", draft);
        let expected = bytes_include_unsent_draft(&bytes);
        assert!(matches!(h.open_history(&bytes), ImportOutcome::ViewOnly(_)));
        assert_eq!(h.export_includes_unsent_draft(), expected);
        let open = history("1.0.0", None);
        assert_eq!(h.open_history(&open), ImportOutcome::Open);
        assert_eq!(
            h.export_includes_unsent_draft(),
            bytes_include_unsent_draft(&open),
            "表示だけの控えが残っている"
        );
    }
}
