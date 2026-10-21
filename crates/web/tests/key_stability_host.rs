//! 教材の版・内容が変わっても保存先の鍵が変わらず、読めない IR で保存先が変わらないことの試験。host の層（TASK-326 2/2、SAVE-14・DLV-8）。
//!
//! 鍵は教材の ID と最初の開始点の ID だけで決まる（試作 F-1）。ここでは web の保存の経路（ホストの記録・`history_to_save`・
//! 保存先の偽物・`boot`）を通して、版・題名・主版が変わった教材が同じ鍵で保存済みの履歴を読むこと、開けないときは上書きしないこと、
//! 読めない IR は保存先を読まず書かないこと、IR が読めないときの鍵が教材の鍵の範囲に入らないことを確かめる。
//! 変異の検出先はこの層である（`tests/mutations/SAVE14-6`）。ブラウザの試験は `tools/browser/key-stability.test.mjs`。
//!
//! 教材は `tests/data/reload_points_ch2/ir.json` の `pack.id` と版・題名を書き換えたもの
//! （`src/test_fixture.rs` の `chapter_ir_bytes` と同じ書き換えで、試験用の口は feature の背後のため写している）。
//! 試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;

use fandhe_workshop_contract::storage::{pack_prefix, DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::save_pending::PendingSave;
use storage_fakes::{fixture, Fixture};

const PACK: &str = "chapter-links";

#[derive(Clone, Copy)]
enum Page {
    /// 元の版（`0.0.1`）。
    A,
    /// 版を `0.1.0`・題名を変えたもの（主版は同じ）。
    Minor,
    /// 主版を `1.0.0` に上げたもの。
    Major,
}

fn data(rel: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

fn base_value() -> serde_json::Value {
    let mut v: serde_json::Value =
        serde_json::from_slice(&data("tests/data/reload_points_ch2/ir.json")).unwrap();
    v["pack"]["id"] = PACK.into();
    v["pack"]["title"] = "章のリンクの試験データ".into();
    v
}

fn ir_bytes(p: Page) -> Vec<u8> {
    let mut v = base_value();
    match p {
        Page::A => {}
        Page::Minor => {
            v["pack"]["version"] = "0.1.0".into();
            v["pack"]["title"] = "章のリンクの試験データ（改訂）".into();
        }
        Page::Major => {
            v["pack"]["version"] = "1.0.0".into();
            v["pack"]["title"] = "章のリンクの試験データ（大改訂）".into();
        }
    }
    serde_json::to_vec(&v).unwrap()
}

/// 開始直後の状態から始まる、起動前のホスト（新しいページを模す）。
fn page(p: Page) -> EngineHost {
    let bytes = ir_bytes(p);
    let ir = load_ir(&bytes).unwrap();
    let node = ir.starts[0].node.clone();
    let sample: serde_json::Value =
        serde_json::from_slice(&data("../../testdata/contract/event-sequence.json")).unwrap();
    let text = sample["initial"].to_string().replace("main/st-1", &node);
    let state: SessionState = serde_json::from_str(&text).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn key(h: &EngineHost) -> HistoryKey {
    h.history_key(DeliveryScope::named("repo")).unwrap()
}

fn current(h: &EngineHost) -> String {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .current
        .as_ref()
        .unwrap()
        .0
        .clone()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

/// 保存先から読んで `boot` する（`persist::boot_and_mount` の native 版）。
fn open(f: &Fixture, p: Page) -> (EngineHost, BootKind) {
    let mut h = page(p);
    let saved = f.storage.read(&key(&h)).unwrap();
    let kind = h.boot(saved.as_deref().map(str::as_bytes));
    (h, kind)
}

fn save(h: &EngineHost, f: &Fixture) {
    let pending = PendingSave::new();
    pending.stage(key(h), h.history_to_save().expect("保存する値がある"));
    let out = pending.flush(&f.storage);
    assert_eq!((out.written, out.failed), (1, 0));
}

fn move_a(h: &mut EngineHost, name: &str) {
    h.update(act("draft", &format!("/make-note {name}")));
    h.update(act("submit", ""));
}

fn history_keys(f: &Fixture) -> Vec<String> {
    f.local
        .map
        .borrow()
        .keys()
        .filter(|k| k.starts_with("fandhe-workshop/history/"))
        .cloned()
        .collect()
}

/// 保存先（localStorage の偽物）の全体。一致かどうかだけを比べる。
fn whole(f: &Fixture) -> Vec<(String, String)> {
    f.local
        .map
        .borrow()
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

#[test]
fn 版と内容を変えた教材でも同じ鍵で保存済みの履歴が開け鍵は増えない() {
    let f = fixture(false);
    let (mut a, _) = open(&f, Page::A);
    move_a(&mut a, "note-1");
    let a_after = current(&a);
    save(&a, &f);
    let a_key = key(&a).render();

    let (mut n, kind) = open(&f, Page::Minor);
    assert_eq!(key(&n).render(), a_key, "版・題名が変わっても鍵は同じ");
    assert_eq!(
        kind,
        BootKind::Restored,
        "同じ主版の変更は保存済みの履歴で開く"
    );
    assert_eq!(current(&n), a_after);
    assert_eq!(history_keys(&f), vec![a_key.clone()]);

    move_a(&mut n, "note-2");
    save(&n, &f);
    assert_eq!(
        history_keys(&f),
        vec![a_key],
        "書かれた鍵は章 A の 1 本だけ"
    );
}

#[test]
fn 主版を変えた教材でも同じ鍵で読み開けなければ上書きしない() {
    let f = fixture(false);
    let (mut a, _) = open(&f, Page::A);
    move_a(&mut a, "note-1");
    save(&a, &f);
    let a_key = key(&a).render();
    let before = whole(&f);

    let (m, kind) = open(&f, Page::Major);
    assert_eq!(key(&m).render(), a_key, "主版が変わっても鍵は同じ");
    assert_eq!(kind, BootKind::NotOpened, "同じ鍵で読んだうえで開けない");
    assert!(
        m.history_to_save().is_none(),
        "開けないときは保存する値が無い"
    );
    assert_eq!(whole(&f), before, "保存先は 1 文字も変わらない");
}

#[test]
fn 読めない_ir_は保存先を読まず書かない() {
    let f = fixture(false);
    let (mut a, _) = open(&f, Page::A);
    move_a(&mut a, "note-1");
    save(&a, &f);
    let before = whole(&f);

    let mut v = base_value();
    v["format_version"] = 3.into();
    v["pack"]["ir_format"] = 3.into();
    v["future_v3_x"] = "x".into();
    let err = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap_err();
    let mut h = EngineHost::refused(&err);
    assert!(h.history_key(DeliveryScope::named("repo")).is_none());
    assert_eq!(h.boot(None), BootKind::NoSession);
    assert!(h.history_to_save().is_none());
    assert_eq!(whole(&f), before, "保存先は 1 文字も変わらない");
}

#[test]
fn 読めない_ir_の鍵は教材の鍵の範囲に入らず教材の履歴を変えない() {
    let f = fixture(false);
    let (mut a, _) = open(&f, Page::A);
    move_a(&mut a, "note-1");
    let a_after = current(&a);
    save(&a, &f);
    let a_key = key(&a).render();

    let scope = DeliveryScope::named("repo");
    let unreadable = HistoryKey::unreadable(scope.clone(), "ir-name");
    let rendered = unreadable.render();
    assert!(!rendered.starts_with(&pack_prefix(&scope, PACK)));
    assert!(!unreadable.belongs_to(&scope, PACK));
    assert_ne!(rendered, a_key);
    assert_eq!(HistoryKey::parse(&rendered), Some(unreadable));

    f.local.map.borrow_mut().insert(rendered, "{}".to_owned());
    let (r, kind) = open(&f, Page::A);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(current(&r), a_after);
}
