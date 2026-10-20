//! 章ごとの保存先の鍵で、別の章の履歴が混ざらないことの試験。host の層（TASK-326 1/2、SAVE-14・DLV-8）。
//!
//! 試作では、同じ教材の別の章（開始点の違う IR）のリンクを同じオリジンで順に開くと、先に保存した別の章の履歴が出た。
//! 鍵が固定の文字列で、章ごとの IR は内容のハッシュが違うため「変換」の経路で開いたためである。
//! ここでは web の保存の経路（ホストの記録・`history_to_save`・保存先の偽物・`boot`）を通して、
//! 章 A・章 B・別の教材が別々の鍵を持ち、互いの履歴を開かないことを確かめる。
//! 変異の検出先はこの層である（`tests/mutations/SAVE14-4`・`SAVE14-5`）。ブラウザの試験は `tools/browser/chapter-keys.test.mjs`。
//!
//! 教材は `tests/data/reload_points_ch2/ir.json` の `pack.id` と `starts` を書き換えたもの
//! （`src/test_fixture.rs` の `chapter_ir_bytes` と同じ書き換えで、試験用の口は feature の背後のため写している）。
//! 試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::path::PathBuf;

use fandhe_workshop_contract::storage::{pack_prefix, DeliveryScope, HistoryKey, HistoryStore};
use fandhe_workshop_core::input::Op;
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
    A,
    B,
    /// 別の教材（`min-3step`）。
    Other,
}

fn data(rel: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

fn ir_bytes(page: Page) -> Vec<u8> {
    if matches!(page, Page::Other) {
        return data("../../testdata/engine-event-consistency/test-data/min-3step.json");
    }
    let mut v: serde_json::Value =
        serde_json::from_slice(&data("tests/data/reload_points_ch2/ir.json")).unwrap();
    v["pack"]["id"] = PACK.into();
    if matches!(page, Page::B) {
        v["starts"] = serde_json::json!([{ "id": "s-4", "node": "main/st-4", "effects": [] }]);
    }
    serde_json::to_vec(&v).unwrap()
}

/// 開始直後の状態から始まる、起動前のホスト（新しいページを模す）。開始の節点は教材の `starts[0].node`。
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

/// 章 A の最初の 1 手（コマンドの入力）。
fn move_a(h: &mut EngineHost) {
    h.update(act("draft", "/make-note note-1"));
    h.update(act("submit", ""));
}

/// 章 B の最初の 1 手（選択肢の選択。確認が `none` のため達成まで進む）。
fn move_b(h: &mut EngineHost) {
    h.update(act("choose", "opt-a"));
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
    }
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

#[test]
fn 章_a_を保存してから章_b_を開くと章_b_は新しく始まり章_a_の履歴は変わらない() {
    let f = fixture(false);
    let (mut a, kind) = open(&f, Page::A);
    assert_eq!(kind, BootKind::Fresh);
    move_a(&mut a);
    let a_after = current(&a);
    save(&a, &f);
    let a_key = key(&a).render();
    let a_saved = f.local.raw(&a_key).expect("章 A の履歴が保存された");

    let (mut b, kind) = open(&f, Page::B);
    assert_eq!(kind, BootKind::Fresh, "章 B は新しく始まる");
    assert_eq!(current(&b), "main/st-4", "章 B の最初の Step");
    assert_eq!(
        f.local.raw(&a_key),
        Some(a_saved.clone()),
        "章 A の履歴は変わらない"
    );
    assert_ne!(key(&b).render(), a_key);
    let prefix = pack_prefix(&DeliveryScope::named("repo"), PACK);
    assert!(key(&b).render().starts_with(&prefix) && a_key.starts_with(&prefix));

    // 章 B が保存を止めていない（`NotOpened` なら保存する値が無い）。
    move_b(&mut b);
    let b_after = current(&b);
    assert_ne!(b_after, "main/st-4");
    save(&b, &f);
    assert_eq!(history_keys(&f).len(), 2, "保存先の履歴の鍵は 2 本");
    assert_eq!(f.local.raw(&a_key), Some(a_saved));

    // それぞれの鍵で開き直すと、保存した時点の Step に戻る。
    let (ra, kind) = open(&f, Page::A);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(current(&ra), a_after);
    let (rb, kind) = open(&f, Page::B);
    assert_eq!(kind, BootKind::Restored);
    assert_eq!(current(&rb), b_after);
}

#[test]
fn 別の教材の鍵は章の鍵と違い履歴が混ざらない() {
    let f = fixture(false);
    let (mut a, _) = open(&f, Page::A);
    move_a(&mut a);
    save(&a, &f);
    let (mut b, _) = open(&f, Page::B);
    move_b(&mut b);
    save(&b, &f);

    let other = page(Page::Other);
    let other_key = key(&other).render();
    assert_ne!(other_key, key(&a).render());
    assert_ne!(other_key, key(&b).render());
    let (o, kind) = open(&f, Page::Other);
    assert_eq!(kind, BootKind::Fresh);
    assert_eq!(current(&o), "main/st-1");
    assert_eq!(history_keys(&f).len(), 2, "別の教材を開いても鍵は増えない");
    assert!(f.storage.read(&key(&other)).unwrap().is_none());
}
