//! 送信（Enter・送信ボタン）の結線の試験（TASK-438。TERM-1・TERM-2）。
//! 画面から届く `submit` の action が、core の記録に本文つきで残ること、拒否されなかった送信の後に
//! 下書きが空になり書き戻しが要求されること、改行が保たれることを native で確かめる。試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::{Component, DirtyTracked};
use fandhe_workshop_contract::judgment::Judgment;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::log_lines::line_from_event;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 開始直後の状態から始まるホストと、書き戻しの要求の回数。
fn host() -> (EngineHost, Rc<Cell<u32>>) {
    let ir = load_ir(
        &std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
            .unwrap(),
    )
    .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap();
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    h.set_draft_sync_request(Box::new(move || c.set(c.get() + 1)));
    (h, n)
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn type_and_submit(h: &mut EngineHost, text: &str) {
    h.update(act("draft", text));
    h.update(act("submit", ""));
}

fn received(h: &EngineHost) -> Vec<Input> {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter_map(|e| match e {
            Event::InputReceived { input, .. } => Some(input.clone()),
            _ => None,
        })
        .collect()
}

fn rejected(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| {
            matches!(
                e,
                Event::Judged {
                    judgment: Judgment::Rejected(_),
                    ..
                }
            )
        })
        .count()
}

#[test]
fn 拒否されない送信は本文つきで記録され_下書きが空になり_書き戻しが要求される() {
    let (mut h, sync) = host();
    type_and_submit(&mut h, "/make-note first-note");
    assert_eq!(
        received(&h),
        [Input::Command("/make-note first-note".to_owned())]
    );
    assert_eq!(rejected(&h), 0);
    assert_eq!(h.draft(), "");
    assert_eq!(sync.get(), 1);
    assert!(h.dirty_fields().contains(&"learner_log"));
    assert!(h.dirty_fields().contains(&"engine_log_rows"));
}

#[test]
fn 拒否された送信は記録され_下書きは残り_書き戻しは要求されない() {
    let (mut h, sync) = host();
    type_and_submit(&mut h, "/unknown-x");
    assert_eq!(received(&h), [Input::Command("/unknown-x".to_owned())]);
    assert_eq!(rejected(&h), 1);
    assert_eq!(h.draft(), "/unknown-x");
    assert_eq!(sync.get(), 0);
}

#[test]
fn 複数行の本文は改行を保った_1_件として記録され_ログの行にも保たれる() {
    for body in [
        "/make-note first-note\n二行目のメモ",
        "一行目のメモ\n二行目のメモ",
        "確認事項：\nログインの手順",
        "a\n\nb",
    ] {
        let (mut h, _) = host();
        type_and_submit(&mut h, body);
        let got = received(&h);
        assert_eq!(got.len(), 1, "{body:?}");
        let text = match &got[0] {
            Input::Command(s) | Input::Text(s) => s.clone(),
            other => panic!("想定外の種類: {other:?}"),
        };
        assert_eq!(text, body);
        let log = h.model().session().unwrap().log();
        let line = log
            .iter()
            .filter(|e| matches!(e, Event::InputReceived { .. }))
            .filter_map(line_from_event)
            .next()
            .unwrap();
        assert!(
            format!("{line:?}").contains(&format!("{body:?}")),
            "{body:?}"
        );
    }
}

#[test]
fn 空白だけの送信はエンジンに送らない() {
    for body in ["   ", "\n\t"] {
        let (mut h, _) = host();
        let before = h.model().session().unwrap().log().len();
        type_and_submit(&mut h, body);
        assert_eq!(h.model().session().unwrap().log().len(), before, "{body:?}");
        assert_eq!(h.draft(), "", "{body:?}");
    }
}

#[test]
fn session_が無くても一覧に積み_下書きは空になる() {
    let mut h = EngineHost::new();
    h.update(act("draft", "hello"));
    h.update(act("submit", ""));
    assert_eq!(h.draft(), "");
    assert_eq!(h.dirty_fields(), ["learner_log"]);
}

#[test]
fn 下書きが空の送信は何もしない() {
    let (mut h, sync) = host();
    h.update(act("submit", ""));
    assert!(received(&h).is_empty());
    assert_eq!(sync.get(), 0);
    assert!(h.dirty_fields().is_empty());
}
