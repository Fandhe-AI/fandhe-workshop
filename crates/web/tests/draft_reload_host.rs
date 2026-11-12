//! 下書きが「保存の値 → 新しいホストの起動」で 1 文字も変わらず戻ることの試験（SAVE-23。TASK-328 2/3。#967）。
//!
//! 画面の再読み込みの復元はブラウザ試験 `tools/browser/draft-reload.test.mjs` が確かめる。ここはその native の対で、
//! `EngineHost` の `update` → `history_to_save`（離れる直前に書く値と同じ）→ 別のホストの `boot` の往復だけを見る。
//! 拒否の後に欄へ残る下書きを記録し直す `submit` の経路（変異 SAVE23-3）は、ここだけが守る。
//! 下書きは受講者のデータのため、失敗の文には値を出さず、長さだけを出す。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;

use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn host() -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let ir = load_ir(&bytes).unwrap();
    let initial: SessionState = serde_json::from_value(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
        )
        .unwrap()["initial"]
            .clone(),
    )
    .unwrap();
    EngineHost::with_session(Session::from_state(ir, initial), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// 保存の値で新しいホストを起動し、欄の下書きを返す。
fn reloaded_draft(h: &EngineHost) -> String {
    let saved = h.history_to_save().expect("保存する値がある");
    let mut h2 = host();
    h2.boot(Some(saved.as_bytes()));
    h2.draft().to_owned()
}

fn assert_same(label: &str, expected: &str, got: &str) {
    assert!(
        got == expected,
        "{label}: 復元した下書きが違う（期待 {} 文字・実際 {} 文字）",
        expected.chars().count(),
        got.chars().count()
    );
}

const KINDS: [(&str, &str); 8] = [
    ("単一行", "hello world"),
    (
        "複数行と空行と Tab",
        "1 行目\n\n3 行目（空行の後）\t（Tab 文字）",
    ),
    ("先頭が改行", "\n先頭が改行"),
    ("先頭が二つの改行", "\n\n二つ"),
    ("末尾が改行", "末尾が改行\n"),
    ("空白だけ", "   "),
    ("Tab だけ", "\t"),
    ("全角空白", "\u{3000}全角空白"),
];

#[test]
fn 文字の種類ごとに下書きが保存の値から戻る() {
    for (label, text) in KINDS {
        let mut h = host();
        h.boot(None);
        h.update(act("draft", text));
        assert_same(label, text, &reloaded_draft(&h));
    }
}

#[test]
fn 拒否された下書きは欄に残り_保存の値から戻る() {
    for (label, text) in [
        ("未知のコマンド", "/unknown-x"),
        ("複数行の自然文", "\n  メモを書く\n\n  二行目  \n"),
    ] {
        let mut h = host();
        h.boot(None);
        h.update(act("draft", text));
        h.update(act("submit", ""));
        assert_same(&format!("{label}（拒否の後の欄）"), text, h.draft());
        assert_same(
            &format!("{label}（拒否の後の復元）"),
            text,
            &reloaded_draft(&h),
        );
    }
}

#[test]
fn 送った本文は戻らず_その後に打った下書きだけが戻る() {
    let mut h = host();
    h.boot(None);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert_same("送信の後の欄", "", h.draft());
    let later = "次の\n複数行\t下書き";
    h.update(act("draft", later));
    assert_same("送信の後の下書き", later, &reloaded_draft(&h));
}

#[test]
fn 打った後に空にした下書きは空で戻る() {
    let mut h = host();
    h.boot(None);
    h.update(act("draft", "消す下書き"));
    h.update(act("draft", ""));
    assert_same("空にした下書き", "", &reloaded_draft(&h));
}
