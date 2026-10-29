//! 保存済みの履歴の削除の試験（TASK-341 1/2。SAVE-32・SHELL-38）。
//!
//! 保存先の層（`HistoryStorage::remove_pack`・`PendingSave::discard_pack`）とホストの層（確認待ちの関門・始め直し）を
//! native で確かめる。変異の検出先はこの層である（`tests/mutations/SAVE32-3`〜`SAVE32-5`）。
//! ブラウザの試験は `tools/browser/delete-history.test.mjs`。鍵・値の文字列は失敗の文に出さない。
//! 試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore, StorageError};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::save_pending::PendingSave;
use fandhe_workshop_web::storage_select::HistoryStorage;
use storage_fakes::{fixture, FakeIdb, FakeLocal};

const PACK: &str = "chapter-links";

fn scope() -> DeliveryScope {
    DeliveryScope::named("repo")
}

fn k(scope: DeliveryScope, pack: &str, start: &str) -> HistoryKey {
    HistoryKey::for_pack(scope, pack, start)
}

// ---- 保存先の層 ----

/// 対象 2 本と、対象外（別の教材・別の配信先・unreadable・履歴でない鍵）を置く。対象外の鍵の一覧を返す。
fn seed(local: &FakeLocal) -> Vec<String> {
    let targets = [k(scope(), PACK, "s-1"), k(scope(), PACK, "s-4")];
    let others = [
        k(scope(), "other-pack", "s-1"),
        k(DeliveryScope::named("elsewhere"), PACK, "s-1"),
        k(DeliveryScope::root(), PACK, "s-1"),
        HistoryKey::unreadable(scope(), "main.json"),
    ];
    let mut map = local.map.borrow_mut();
    for t in &targets {
        map.insert(t.render(), "v-target".into());
    }
    for o in &others {
        map.insert(o.render(), "v-other".into());
    }
    map.insert("fandhe-workshop/meta/boot/x".into(), "7".into());
    map.insert("unrelated-key".into(), "u".into());
    others.iter().map(HistoryKey::render).collect()
}

fn snapshot(local: &FakeLocal, keep: &[String]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = keep
        .iter()
        .map(|k| (k.clone(), local.raw(k).unwrap()))
        .collect();
    for extra in ["fandhe-workshop/meta/boot/x", "unrelated-key"] {
        out.push((extra.into(), local.raw(extra).unwrap()));
    }
    out
}

#[test]
fn remove_pack_は対象の鍵だけを消し件数を返す() {
    let f = fixture(false);
    let keep = seed(&f.local);
    let before = snapshot(&f.local, &keep);
    let n = f.storage.remove_pack(&scope(), PACK).unwrap();
    assert_eq!(n, 2);
    let left = f.storage.keys().unwrap();
    assert!(!left.iter().any(|x| x.belongs_to(&scope(), PACK)));
    assert_eq!(snapshot(&f.local, &keep), before, "対象外は件数も値も不変");
    assert_eq!(f.storage.remove_pack(&scope(), PACK).unwrap(), 0);
}

#[test]
fn remove_pack_は_indexeddb_の写しと保存先の両方から消す() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let mut initial = BTreeMap::new();
    initial.insert(k(scope(), PACK, "s-4").render(), "v-idb".to_string());
    initial.insert(k(scope(), "other-pack", "s-1").render(), "o".to_string());
    idb.store.borrow_mut().extend(initial.clone());
    let storage = HistoryStorage::new(Rc::clone(&local), Some(Rc::clone(&idb)), initial);
    let keep = seed(&local);
    assert_eq!(storage.remove_pack(&scope(), PACK).unwrap(), 2);
    idb.settle(Ok(()));
    assert!(!idb
        .store
        .borrow()
        .keys()
        .any(|x| HistoryKey::parse(x).is_some_and(|h| h.belongs_to(&scope(), PACK))));
    assert!(idb
        .store
        .borrow()
        .contains_key(&k(scope(), "other-pack", "s-1").render()));
    assert!(!storage
        .keys()
        .unwrap()
        .iter()
        .any(|x| x.belongs_to(&scope(), PACK)));
    for key in keep {
        assert!(local.raw(&key).is_some());
    }
}

#[test]
fn 読めない保存先では何も消さず_unavailable() {
    let local = Rc::new(FakeLocal::default());
    let keep = seed(&local);
    let storage = HistoryStorage::<_, Rc<FakeIdb>>::unreadable(Rc::clone(&local));
    assert_eq!(
        storage.remove_pack(&scope(), PACK),
        Err(StorageError::Unavailable)
    );
    assert!(local.raw(&k(scope(), PACK, "s-1").render()).is_some());
    assert!(local.raw(&keep[0]).is_some());
}

#[test]
fn discard_pack_は対象の保留だけを外し作り直させない() {
    let f = fixture(false);
    let pending = PendingSave::new();
    pending.stage(k(scope(), PACK, "s-1"), "a".into());
    pending.stage(k(scope(), PACK, "s-4"), "b".into());
    pending.stage(k(scope(), "other-pack", "s-1"), "c".into());
    assert_eq!(pending.discard_pack(&scope(), PACK), 2);
    assert_eq!(pending.pending_len(), 1);
    let out = pending.flush(&f.storage);
    assert_eq!((out.written, out.failed), (1, 0));
    assert!(f.storage.read(&k(scope(), PACK, "s-1")).unwrap().is_none());
    assert!(f
        .storage
        .read(&k(scope(), "other-pack", "s-1"))
        .unwrap()
        .is_some());
}

// ---- ホストの層 ----

fn data(rel: &str) -> Vec<u8> {
    std::fs::read(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap()
}

fn page() -> EngineHost {
    let mut v: serde_json::Value =
        serde_json::from_slice(&data("tests/data/reload_points_ch2/ir.json")).unwrap();
    v["pack"]["id"] = PACK.into();
    let ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let node = ir.starts[0].node.clone();
    let sample: serde_json::Value =
        serde_json::from_slice(&data("../../testdata/contract/event-sequence.json")).unwrap();
    let text = sample["initial"].to_string().replace("main/st-1", &node);
    let state: SessionState = serde_json::from_str(&text).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
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

fn step(h: &mut EngineHost) {
    h.update(act("draft", "/make-note note-1"));
    h.update(act("submit", ""));
}

struct Probe {
    host: EngineHost,
    deleted: Rc<Cell<u32>>,
    saved: Rc<Cell<u32>>,
    cancelled: Rc<Cell<u32>>,
}

fn probe(result: Result<usize, StorageError>) -> Probe {
    let mut host = page();
    let deleted = Rc::new(Cell::new(0));
    let saved = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let d = Rc::clone(&deleted);
    host.set_history_deleter(Box::new(move || {
        d.set(d.get() + 1);
        result
    }));
    let s = Rc::clone(&saved);
    host.set_history_saver(Box::new(move |_| {
        s.set(s.get() + 1);
        Ok(())
    }));
    let c = Rc::clone(&cancelled);
    host.set_save_cancel(Box::new(move || c.set(c.get() + 1)));
    Probe {
        host,
        deleted,
        saved,
        cancelled,
    }
}

fn send(h: &mut EngineHost, which: &str) {
    h.update(match which {
        "request" => HostAction::DeleteHistory,
        "confirm" => HostAction::ConfirmDelete,
        _ => HostAction::CancelDelete,
    });
}

#[test]
fn 確認待ちなしの確定は何も消さず状態も変えない() {
    let mut p = probe(Ok(2));
    step(&mut p.host);
    let moved = current(&p.host);
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 0);
    assert_eq!(current(&p.host), moved);
}

#[test]
fn やめるの後の確定も消さない() {
    let mut p = probe(Ok(2));
    step(&mut p.host);
    send(&mut p.host, "request");
    send(&mut p.host, "cancel");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 0);
}

#[test]
fn 確定すると最初の_step_から新しく始まり保存しない() {
    let mut p = probe(Ok(2));
    let first = current(&p.host);
    step(&mut p.host);
    assert!(
        p.host.history_to_save().is_some(),
        "操作の後は保存する値がある"
    );
    let saved_before = p.saved.get();
    send(&mut p.host, "request");
    assert_eq!(p.deleted.get(), 0, "要求だけでは消さない");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1);
    assert_eq!(current(&p.host), first);
    assert!(p.host.history_to_save().is_none());
    assert_eq!(p.saved.get(), saved_before, "削除の更新で保存しない");
    assert!(p.cancelled.get() >= 1, "予約済みの保存を取り下げる");
    // 確認待ちの印は 1 回で消える。
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1);
}

#[test]
fn 削除に失敗したら状態を変えない() {
    let mut p = probe(Err(StorageError::Failed));
    step(&mut p.host);
    let moved = current(&p.host);
    let to_save = p.host.history_to_save();
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1);
    assert_eq!(current(&p.host), moved);
    assert_eq!(p.host.history_to_save(), to_save);
}

#[test]
fn 口が未設定の確定は何も起きない() {
    let mut host = page();
    step(&mut host);
    let moved = current(&host);
    send(&mut host, "request");
    send(&mut host, "confirm");
    assert_eq!(current(&host), moved);
}

#[test]
fn 開けなかった履歴からの削除の成功で保存の停止が解ける() {
    let mut p = probe(Ok(1));
    assert_ne!(p.host.boot(Some(b"not a history")), BootKind::Restored);
    step(&mut p.host);
    assert!(p.host.history_to_save().is_none(), "停止中は保存しない");
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    step(&mut p.host);
    assert!(p.host.history_to_save().is_some(), "停止が解けた");
}

#[test]
fn remove_pack_は起動後に別のタブが書いた_indexeddb_の鍵も消す() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let storage = HistoryStorage::new(Rc::clone(&local), Some(Rc::clone(&idb)), BTreeMap::new());
    let late = k(scope(), PACK, "s-9").render();
    let other = k(scope(), "other-pack", "s-1").render();
    idb.store.borrow_mut().insert(late.clone(), "x".into());
    idb.store.borrow_mut().insert(other.clone(), "o".into());
    storage.remove_pack(&scope(), PACK).unwrap();
    assert!(!idb.store.borrow().contains_key(&late));
    assert!(idb.store.borrow().contains_key(&other));
}

#[test]
fn remove_pack_は_localstorage_の鍵の一覧を引けなければ失敗する() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    local.fail_keys.set(true);
    let storage = HistoryStorage::new(Rc::clone(&local), Some(Rc::clone(&idb)), BTreeMap::new());
    assert!(storage.remove_pack(&scope(), PACK).is_err());
}
