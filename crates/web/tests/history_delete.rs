//! 保存済みの履歴の削除の試験（TASK-341 1/2。SAVE-32・SHELL-38）。
//!
//! 保存先の層（`HistoryStorage::remove_pack`・`PendingSave::discard_pack`）とホストの層（確認待ちの関門・始め直し）を
//! native で確かめる。変異の検出先はこの層である（`tests/mutations/SAVE32-3`〜`SAVE32-5`）。
//! ブラウザの試験は `tools/browser/delete-history.test.mjs`。鍵・値の文字列は失敗の文に出さない。
//! 試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::cell::{Cell, RefCell};
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
use storage_fakes::{fixture, FakeIdb, FakeLocal, Storage};

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

type Outcome = Rc<RefCell<Option<Result<usize, StorageError>>>>;

/// `remove_pack` を呼び、完了の通知の結果を受ける箱を返す（通知前は `None`）。
fn remove(storage: &Storage, scope: &DeliveryScope, pack: &str) -> Outcome {
    let out: Outcome = Rc::new(RefCell::new(None));
    let o = Rc::clone(&out);
    storage.remove_pack(
        scope,
        pack,
        Box::new(move |r| {
            assert!(o.borrow_mut().replace(r).is_none(), "完了は 1 回だけ");
        }),
    );
    out
}

#[test]
fn remove_pack_は対象の鍵だけを消し件数を返す() {
    let f = fixture(false);
    let keep = seed(&f.local);
    let before = snapshot(&f.local, &keep);
    let n = remove(&f.storage, &scope(), PACK)
        .borrow()
        .unwrap()
        .unwrap();
    assert_eq!(n, 2);
    let left = f.storage.keys().unwrap();
    assert!(!left.iter().any(|x| x.belongs_to(&scope(), PACK)));
    assert_eq!(snapshot(&f.local, &keep), before, "対象外は件数も値も不変");
    assert_eq!(remove(&f.storage, &scope(), PACK).borrow().unwrap(), Ok(0));
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
    let out = remove(&storage, &scope(), PACK);
    assert!(out.borrow().is_none(), "delete の確定までは完了を返さない");
    idb.settle(Ok(()));
    assert_eq!(*out.borrow(), Some(Ok(2)));
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
        *remove(&storage, &scope(), PACK).borrow(),
        Some(Err(StorageError::Unavailable))
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

#[test]
fn discard_key_は指定した鍵の保留だけを外す() {
    let pending = PendingSave::new();
    pending.stage(k(scope(), PACK, "s-1"), "a".into());
    pending.stage(k(scope(), PACK, "s-4"), "b".into());
    pending.stage(k(scope(), "other-pack", "s-1"), "c".into());
    assert_eq!(pending.discard_key(&k(scope(), PACK, "s-1")), 1);
    assert_eq!(pending.discard_key(&k(scope(), PACK, "s-1")), 0);
    assert_eq!(pending.pending_len(), 2);
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
    /// 直近に頼まれた削除の世代。
    gen: Rc<Cell<u64>>,
    saved: Rc<Cell<u32>>,
    cancelled: Rc<Cell<u32>>,
}

/// 口は頼まれた世代を控えるだけで、完了は試験が `DeleteResult` で届ける。
fn probe() -> Probe {
    let mut host = page();
    let deleted = Rc::new(Cell::new(0));
    let saved = Rc::new(Cell::new(0));
    let cancelled = Rc::new(Cell::new(0));
    let d = Rc::clone(&deleted);
    let gen = Rc::new(Cell::new(0));
    let g = Rc::clone(&gen);
    host.set_history_deleter(Box::new(move |generation| {
        g.set(generation);
        d.set(d.get() + 1);
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
        gen,
        saved,
        cancelled,
    }
}

/// 削除の完了を届ける（頼まれた世代で）。
fn complete(p: &mut Probe, result: Result<usize, StorageError>) {
    p.host.update(HostAction::DeleteResult(p.gen.get(), result));
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
    let mut p = probe();
    step(&mut p.host);
    let moved = current(&p.host);
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 0);
    assert_eq!(current(&p.host), moved);
}

#[test]
fn やめるの後の確定も消さない() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    send(&mut p.host, "cancel");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 0);
}

#[test]
fn 確定すると最初の_step_から新しく始まり保存しない() {
    let mut p = probe();
    let first = current(&p.host);
    step(&mut p.host);
    assert!(
        p.host.history_to_save().is_some(),
        "操作の後は保存する値がある"
    );
    let moved = current(&p.host);
    let saved_before = p.saved.get();
    send(&mut p.host, "request");
    assert_eq!(p.deleted.get(), 0, "要求だけでは消さない");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1);
    assert_eq!(current(&p.host), moved, "完了までは画面を変えない");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1, "頼んでいる間は重ねない");
    complete(&mut p, Ok(2));
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
    let mut p = probe();
    step(&mut p.host);
    let moved = current(&p.host);
    let to_save = p.host.history_to_save();
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 1);
    complete(&mut p, Err(StorageError::Failed));
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
    let mut p = probe();
    assert_ne!(p.host.boot(Some(b"not a history")), BootKind::Restored);
    step(&mut p.host);
    assert!(p.host.history_to_save().is_none(), "停止中は保存しない");
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    complete(&mut p, Ok(1));
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
    let out = remove(&storage, &scope(), PACK);
    idb.settle(Ok(()));
    assert!(!idb.store.borrow().contains_key(&late));
    assert!(idb.store.borrow().contains_key(&other));
    // 起動後に別のタブが足した鍵だけでも件数に入る。
    assert_eq!(*out.borrow(), Some(Ok(1)));
}

#[test]
fn remove_pack_は_localstorage_の鍵の一覧を引けなければ失敗する() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    local.fail_keys.set(true);
    let storage = HistoryStorage::new(Rc::clone(&local), Some(Rc::clone(&idb)), BTreeMap::new());
    assert!(matches!(
        *remove(&storage, &scope(), PACK).borrow(),
        Some(Err(_))
    ));
}

#[test]
fn remove_pack_は_indexeddb_の_delete_が失敗したら失敗を返し写しを残す() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let key = k(scope(), PACK, "s-4").render();
    let initial = BTreeMap::from([(key.clone(), "v".to_string())]);
    idb.store.borrow_mut().extend(initial.clone());
    let storage = HistoryStorage::new(Rc::clone(&local), Some(Rc::clone(&idb)), initial);
    let out = remove(&storage, &scope(), PACK);
    assert!(out.borrow().is_none());
    idb.settle(Err(StorageError::Failed));
    assert_eq!(*out.borrow(), Some(Err(StorageError::Failed)));
    assert!(
        storage
            .keys()
            .unwrap()
            .iter()
            .any(|x| x.belongs_to(&scope(), PACK)),
        "確定していない鍵は写しに残る"
    );
}

#[test]
fn 世代の違う完了と頼んでいない完了は捨てる() {
    let mut p = probe();
    let first = current(&p.host);
    step(&mut p.host);
    let moved = current(&p.host);
    complete(&mut p, Ok(1));
    assert_eq!(current(&p.host), moved, "頼んでいない完了は捨てる");
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    p.host
        .update(HostAction::DeleteResult(p.gen.get() + 9, Ok(1)));
    assert_eq!(current(&p.host), moved, "世代の違う完了は捨てる");
    complete(&mut p, Ok(1));
    assert_eq!(current(&p.host), first);
}

#[test]
fn 削除の完了待ちの間は保存せず失敗すると保存へ戻る() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    let cancelled = p.cancelled.get();
    send(&mut p.host, "confirm");
    assert!(
        p.cancelled.get() > cancelled,
        "開始時に保存の予約を取り下げる"
    );
    assert!(
        p.host.history_to_save().is_none(),
        "完了待ちの間は保存しない"
    );
    complete(&mut p, Err(StorageError::Failed));
    assert!(p.host.history_to_save().is_some(), "失敗したら保存へ戻る");
}

#[test]
fn 削除の失敗と世代不一致の完了で自動保存を再び走らせる() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    let before = p.saved.get();
    complete(&mut p, Err(StorageError::Failed));
    assert!(p.saved.get() > before, "失敗で止めていた分を保存し直す");

    send(&mut p.host, "request");
    let text = p.host.history_to_save().expect("書き出せる履歴がある");
    send(&mut p.host, "confirm");
    p.host.update(HostAction::ImportHistory(text));
    let before = p.saved.get();
    complete(&mut p, Ok(1));
    assert!(
        p.saved.get() > before,
        "差し替え後の完了でも現在の履歴を保存し直す"
    );
}

#[test]
fn 履歴を差し替えると削除の確認待ちも取り消される() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    let text = p.host.history_to_save().expect("書き出せる履歴がある");
    p.host.update(HostAction::ImportHistory(text));
    send(&mut p.host, "confirm");
    assert_eq!(p.deleted.get(), 0, "差し替えの後は改めて要求が要る");
}

#[test]
fn 削除の完了待ちに履歴を差し替えても実処理の完了まで保存を止める() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    let text = p.host.history_to_save().expect("書き出せる履歴がある");
    send(&mut p.host, "confirm");
    p.host.update(HostAction::ImportHistory(text));
    let moved = current(&p.host);
    p.host.update(act("draft", "/make-note note-2"));
    p.host.update(act("submit", ""));
    assert!(
        p.host.history_to_save().is_none(),
        "差し替えの後も、実処理の完了までは保存しない（古い delete が新しい保存を消すため）"
    );
    complete(&mut p, Ok(1));
    assert_eq!(
        current(&p.host),
        moved,
        "差し替え後の状態を完了で巻き戻さない"
    );
    assert!(p.host.history_to_save().is_some(), "完了の後は保存へ戻る");
}

/// `min-3step` を指定の版にした IR（`view_only.rs` と同じ作り方）。
fn min3_ir(version: &str) -> fandhe_workshop_contract::ir::Ir {
    let mut v: serde_json::Value = serde_json::from_slice(&data(
        "../../testdata/engine-event-consistency/test-data/min-3step.json",
    ))
    .unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

fn min3_initial() -> SessionState {
    let v: serde_json::Value =
        serde_json::from_slice(&data("../../testdata/contract/event-sequence.json")).unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

/// `version` の教材で書いた履歴のバイト列。
fn min3_history(version: &str) -> Vec<u8> {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
    use fandhe_workshop_core::history_replay::replay;
    let ir = min3_ir(version);
    let sample = data("../../testdata/contract/history/two-rounds.json");
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, min3_initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

#[test]
fn 削除の完了待ちに表示だけへ入っても完了で保存の停止が解ける() {
    use fandhe_workshop_web::host::ImportOutcome;
    let ir = min3_ir("1.0.0");
    let mut host =
        EngineHost::with_session(Session::from_state(ir, min3_initial()), standard_registry())
            .unwrap();
    let gen = Rc::new(Cell::new(0));
    let g = Rc::clone(&gen);
    host.set_history_deleter(Box::new(move |generation| g.set(generation)));
    host.set_history_saver(Box::new(|_| Ok(())));
    host.set_save_cancel(Box::new(|| {}));

    step(&mut host);
    send(&mut host, "request");
    send(&mut host, "confirm");

    assert!(matches!(
        host.open_history(&min3_history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    host.update(HostAction::DeleteResult(
        gen.get(),
        Err(StorageError::Failed),
    ));
    assert_eq!(
        host.open_history(&min3_history("1.0.0")),
        ImportOutcome::Open
    );
    assert!(
        host.history_to_save().is_some(),
        "表示だけの間に届いた完了で、保存の停止が解けている"
    );
}

// ---- 削除の確認の表示（TASK-539 1/2。SHELL-38・D-75） ----

fn view_dump(h: &EngineHost) -> String {
    format!("{:?}", h.view())
}

fn confirm_items(h: &EngineHost) -> usize {
    view_dump(h).matches("shell-delete-confirm").count()
}

#[test]
fn 削除の3つの名前は空の_payload_だけ復号する() {
    use fandhe_workshop_web::shell::{
        ACTION_CANCEL_DELETE, ACTION_CONFIRM_DELETE, ACTION_DELETE_HISTORY,
    };
    for name in [
        ACTION_DELETE_HISTORY,
        ACTION_CONFIRM_DELETE,
        ACTION_CANCEL_DELETE,
    ] {
        assert!(EngineHost::decode_action(name, "").is_some(), "{name}");
        assert!(EngineHost::decode_action(name, "x").is_none(), "{name}");
    }
}

#[test]
fn 要求で確認の通知が出て口は呼ばれず通知欄が_dirty_になる() {
    use fandhe_frontend_interactive::DirtyTracked;
    let mut p = probe();
    step(&mut p.host);
    assert_eq!(confirm_items(&p.host), 0);
    send(&mut p.host, "request");
    assert_eq!(confirm_items(&p.host), 1);
    assert!(p.host.dirty_fields().contains(&"shell_notices"));
    let v = view_dump(&p.host);
    assert_eq!(v.matches("shell-delete-confirm").count(), 1, "{v}");
    assert_eq!(v.matches("shell-delete-cancel").count(), 1, "{v}");
    assert!(v.contains(fandhe_workshop_web::shell_notice::DELETE_CONFIRM_NOTICE));
    assert!(v.contains("この教材の保存済みの履歴"), "対象を示す");
    assert!(
        v.contains("書き出し済みのファイルは消えません"),
        "書き出し済みは消えない"
    );
    assert_eq!(p.deleted.get(), 0, "確認の前には消さない");
    // 同じ要求の繰り返しは通知欄を dirty にしない。
    send(&mut p.host, "request");
    assert!(!p.host.dirty_fields().contains(&"shell_notices"));
}

#[test]
fn やめるで確認の通知が消え口は呼ばれない() {
    use fandhe_frontend_interactive::DirtyTracked;
    let mut p = probe();
    send(&mut p.host, "request");
    send(&mut p.host, "cancel");
    assert_eq!(
        view_dump(&p.host).matches("shell-delete-confirm").count(),
        0
    );
    assert!(p.host.dirty_fields().contains(&"shell_notices"));
    assert_eq!(p.deleted.get(), 0);
}

#[test]
fn 履歴の差し替えで確認の通知も消える() {
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    let text = p.host.history_to_save().expect("書き出せる履歴がある");
    p.host.update(HostAction::ImportHistory(text));
    assert_eq!(
        view_dump(&p.host).matches("shell-delete-confirm").count(),
        0
    );
}

#[test]
fn 削除のボタンは書き出しの入れ物の中にあり1個だけある() {
    let p = probe();
    let v = view_dump(&p.host);
    assert_eq!(v.matches("\"shell-delete\"").count(), 1, "{v}");
    let ops = v.find("history-ops").expect("history-ops");
    assert!(ops < v.find("\"shell-delete\"").unwrap(), "{v}");
}

#[test]
fn 削除に失敗すると失敗の文と再試行の確認が出て状態は保たれる() {
    use fandhe_workshop_web::shell_notice::DELETE_FAILED_NOTICE;
    let mut p = probe();
    step(&mut p.host);
    let moved = current(&p.host);
    send(&mut p.host, "request");
    assert!(!view_dump(&p.host).contains(DELETE_FAILED_NOTICE));
    send(&mut p.host, "confirm");
    complete(&mut p, Err(StorageError::Failed));
    let v = view_dump(&p.host);
    assert!(v.contains(DELETE_FAILED_NOTICE), "失敗を知らせる");
    assert_eq!(confirm_items(&p.host), 1, "再試行の確認が残る");
    assert_eq!(current(&p.host), moved, "状態は保たれる");
    send(&mut p.host, "cancel");
    assert!(!view_dump(&p.host).contains(DELETE_FAILED_NOTICE));
    assert_eq!(confirm_items(&p.host), 0);
    send(&mut p.host, "request");
    assert!(!view_dump(&p.host).contains(DELETE_FAILED_NOTICE));
}

#[test]
fn 削除中に再度要求しても失敗の文が出る() {
    use fandhe_workshop_web::shell_notice::DELETE_FAILED_NOTICE;
    let mut p = probe();
    step(&mut p.host);
    send(&mut p.host, "request");
    send(&mut p.host, "confirm");
    // 完了待ちの間に常設の削除ボタンをもう一度押す。
    send(&mut p.host, "request");
    complete(&mut p, Err(StorageError::Failed));
    let v = view_dump(&p.host);
    assert!(v.contains(DELETE_FAILED_NOTICE), "{v}");
    assert_eq!(confirm_items(&p.host), 1);
}
