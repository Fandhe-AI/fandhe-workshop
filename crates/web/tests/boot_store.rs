//! 起動の分岐とホストの保存の結線の試験（TASK-319 1/3、SAVE-3・SAVE-28）。
//!
//! 保存済みの履歴が無い場合と、未操作の場合に、新しい状態で始まり世代が上がらないことを、web の保存の経路
//! （ホストの記録・`history_to_save`・保留と書き出し・保存先の偽物からの読み戻し）を通して確かめる。
//! 陽性の対照として、操作 1 回の後は保存され、新しいページで復元されて世代が +1 になることを 1 地点だけ確かめる。
//! 開けない保存済みの値があるときは新しく始め、保存済みの値を上書きしない。試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::save_pending::PendingSave;
use storage_fakes::{fixture, Fixture};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 開始直後の状態から始まる、起動前のホスト（新しいページを模す）。
fn page() -> EngineHost {
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
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn key(h: &EngineHost) -> HistoryKey {
    h.history_key(DeliveryScope::named("repo")).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn generation(h: &EngineHost) -> u64 {
    h.model().session().unwrap().generation().0
}

fn reloaded(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Reloaded { .. }))
        .count()
}

/// 新しいページを開く。保存先から読んで `boot` する（`persist::boot_and_mount` の native 版）。
fn open_page(f: &Fixture) -> (EngineHost, BootKind) {
    let mut h = page();
    let saved = f.storage.read(&key(&h)).unwrap();
    let kind = h.boot(saved.as_deref().map(str::as_bytes));
    (h, kind)
}

/// 離れる直前の書き出し（`persist::install_save_on_leave` の native 版）。
fn leave(h: &EngineHost, f: &Fixture) {
    if let Some(v) = h.history_to_save() {
        let pending = PendingSave::new();
        pending.stage(key(h), v);
        let out = pending.flush(&f.storage);
        assert_eq!((out.written, out.failed), (1, 0));
    }
}

#[test]
fn 保存済みの履歴が無ければ新しく始まり世代は上がらない() {
    let f = fixture(false);
    let (h, kind) = open_page(&f);
    assert_eq!(kind, BootKind::Fresh);
    assert_eq!(generation(&h), 1);
    assert_eq!(reloaded(&h), 0);
    assert_eq!(h.history_to_save(), None);
}

#[test]
fn 未操作のまま離れても何も保存されず次の起動も新しく始まる() {
    let f = fixture(false);
    for _ in 0..3 {
        let (h, kind) = open_page(&f);
        assert_eq!(kind, BootKind::Fresh);
        assert_eq!(generation(&h), 1);
        assert_eq!(reloaded(&h), 0);
        assert_eq!(h.history_to_save(), None, "未操作は保存しない");
        leave(&h, &f);
        assert!(f.local.map.borrow().is_empty(), "鍵が 1 件も書かれない");
    }
}

#[test]
fn 空の下書きへの更新だけでは操作に数えない() {
    let f = fixture(false);
    let (mut h, _) = open_page(&f);
    h.update(act("draft", ""));
    assert_eq!(h.history_to_save(), None);
}

#[test]
fn 対照_操作の後は保存され新しいページで復元されて世代が_1_上がる() {
    let f = fixture(false);
    let (mut h, _) = open_page(&f);
    h.update(act("draft", "/make-note first-note"));
    h.update(act("submit", ""));
    assert!(h.history_to_save().is_some());
    let before = generation(&h);
    leave(&h, &f);
    assert_eq!(f.local.map.borrow().len(), 1);

    let (h2, kind) = open_page(&f);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(generation(&h2), before + 1);
    assert_eq!(reloaded(&h2), 1);
    // 復元の後も保存でき、さらに復元できる（記録は読み込んだ ops の続きになる）。
    leave(&h2, &f);
    let (h3, kind) = open_page(&f);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(generation(&h3), before + 2);
    assert_eq!(reloaded(&h3), 2);
}

#[test]
fn 入力途中の下書きだけでも未操作ではなく復元される() {
    let f = fixture(false);
    let (mut h, _) = open_page(&f);
    h.update(act("draft", "書きかけ"));
    leave(&h, &f);
    let (h2, kind) = open_page(&f);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(h2.draft(), "書きかけ", "入力欄の下書きが戻る");
}

#[test]
fn 開けない保存済みの値は新しく始め上書きしない() {
    let f = fixture(false);
    let k = key(&page());
    f.storage.write(&k, "{broken").unwrap();
    let (mut h, kind) = open_page(&f);
    assert_eq!(kind, BootKind::NotOpened);
    assert_eq!(generation(&h), 1);
    // 操作しても、開けなかった履歴を上書きするために保存はしない。
    h.update(act("draft", "/make-note x"));
    h.update(act("submit", ""));
    assert_eq!(h.history_to_save(), None);
    leave(&h, &f);
    assert_eq!(f.local.raw(&k.render()).as_deref(), Some("{broken"));
}
