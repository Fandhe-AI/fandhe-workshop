//! 保存先の口と履歴の鍵（`storage`）の適合試験。鍵が教材の版・内容に依らず教材 ID と開始点 ID で一意に決まること
//! （SAVE-14）、配信先の区間（D-56・SAVE-15）、鍵の符号化の単射性、口の契約（D-52）を固定する。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_workshop_contract::ir::{Ir, Start};
use fandhe_workshop_contract::storage::*;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet};

fn base_ir() -> Ir {
    let src = include_str!("data/ir_format_1/minimal.json");
    let mut ir: Ir = serde_json::from_str(src).unwrap();
    ir.pack.id = "bbs-tutorial".to_string();
    let start: Start =
        serde_json::from_value(serde_json::json!({"id": "s-0-1", "node": "n"})).unwrap();
    ir.starts = vec![start];
    ir
}

fn repo() -> DeliveryScope {
    DeliveryScope::named("repo")
}

#[test]
fn storage_contract_version_is_1() {
    assert_eq!(STORAGE_CONTRACT_VERSION, 1);
}

#[test]
fn default_kind_is_local_storage() {
    assert_eq!(DEFAULT_STORAGE_KIND, StorageKind::LocalStorage);
}

#[test]
fn key_ignores_pack_version_and_content() {
    let a = base_ir();
    let mut b = base_ir();
    b.pack.version = "9.9.9".to_string();
    b.pack.title = "別の題名".to_string();
    b.pack.guidance = "C".to_string();
    b.format_version = 2;
    b.pack.ir_format = 2;
    b.hints.insert("h".to_string(), serde_json::json!({"x": 1}));
    b.starts[0].node = "other-node".to_string();
    let ka = HistoryKey::from_ir(repo(), &a).unwrap();
    let kb = HistoryKey::from_ir(repo(), &b).unwrap();
    assert_eq!(ka, kb);
    assert_eq!(ka.render(), kb.render());
}

#[test]
fn key_uses_first_start_only() {
    let mut ir = base_ir();
    let mut second = ir.starts[0].clone();
    second.id = "s-9".to_string();
    ir.starts.push(second);
    let k = HistoryKey::from_ir(repo(), &ir).unwrap();
    assert_eq!(
        k.render(),
        "fandhe-workshop/history/repo/bbs-tutorial/s-0-1"
    );
}

#[test]
fn key_differs_by_start() {
    let a = HistoryKey::for_pack(repo(), "p", "s1");
    let b = HistoryKey::for_pack(repo(), "p", "s2");
    assert_ne!(a.render(), b.render());
}

#[test]
fn key_differs_by_pack() {
    let a = HistoryKey::for_pack(repo(), "p1", "s");
    let b = HistoryKey::for_pack(repo(), "p2", "s");
    assert_ne!(a.render(), b.render());
}

#[test]
fn key_differs_by_delivery_scope() {
    let a = HistoryKey::for_pack(DeliveryScope::named("a"), "p", "s");
    let b = HistoryKey::for_pack(DeliveryScope::named("b"), "p", "s");
    let c = HistoryKey::for_pack(DeliveryScope::root(), "p", "s");
    assert_ne!(a.render(), b.render());
    assert_ne!(a.render(), c.render());
}

#[test]
fn prototype_ids_render_verbatim() {
    let root = HistoryKey::for_pack(DeliveryScope::root(), "bbs-tutorial", "s-0-1");
    assert_eq!(
        root.render(),
        "fandhe-workshop/history/~/bbs-tutorial/s-0-1"
    );
    assert_eq!(root.to_string(), root.render());
    let k = HistoryKey::for_pack(repo(), "bbs-tutorial", "s-0-1");
    assert_eq!(
        k.render(),
        "fandhe-workshop/history/repo/bbs-tutorial/s-0-1"
    );
}

#[test]
fn unreadable_key_form() {
    let k = HistoryKey::unreadable(repo(), "my.ir.json");
    assert_eq!(
        k.render(),
        "fandhe-workshop/history/repo/unreadable/my.ir.json"
    );
}

#[test]
fn pack_named_unreadable_does_not_collide() {
    let pack = HistoryKey::for_pack(repo(), "unreadable", "x");
    let ir = HistoryKey::unreadable(repo(), "x");
    assert_ne!(pack.render(), ir.render());
    assert_eq!(pack.render(), "fandhe-workshop/history/repo/%75nreadable/x");
    assert_eq!(HistoryKey::parse(&pack.render()), Some(pack));
    assert_eq!(HistoryKey::parse(&ir.render()), Some(ir));
}

#[test]
fn delivery_scope_from_pathname() {
    let root = DeliveryScope::root();
    for p in ["/", "/index.html", "//", "", "/repo", "index.html"] {
        assert_eq!(DeliveryScope::from_pathname(p), root, "{p:?}");
    }
    for p in ["/repo/", "/repo/index.html", "/repo/v2/x/", "//repo/"] {
        assert_eq!(DeliveryScope::from_pathname(p), repo(), "{p:?}");
    }
    assert_eq!(repo().segment(), Some("repo"));
    assert_eq!(root.segment(), None);
    assert_eq!(DeliveryScope::named(""), root);
}

#[test]
fn render_parse_roundtrip_and_injective() {
    let alphabet = ["a", "/", "%", "~", ".", "あ", "", "unreadable", "%41"];
    for sc in alphabet {
        for p in alphabet {
            for s in alphabet {
                let scope = DeliveryScope::named(sc);
                let pack = HistoryKey::for_pack(scope.clone(), p, s);
                let ir = HistoryKey::unreadable(scope, s);
                for k in [pack, ir] {
                    let r = k.render();
                    assert_eq!(HistoryKey::parse(&r), Some(k.clone()), "{r}");
                    let rest = r.strip_prefix("fandhe-workshop/history/").unwrap();
                    assert_eq!(rest.split('/').count(), 3, "{r}");
                }
            }
        }
    }
    // 空の区間は根と同じ扱い（named("") == root）なので、区間が空でない組についてだけ、
    // 鍵の文字列がすべて異なることを確かめる。
    let mut distinct = HashSet::new();
    let mut n = 0usize;
    for sc in alphabet.iter().filter(|s| !s.is_empty()) {
        for p in alphabet {
            for s in alphabet {
                distinct.insert(HistoryKey::for_pack(DeliveryScope::named(sc), p, s).render());
                n += 1;
            }
        }
    }
    for sc in alphabet.iter().filter(|s| !s.is_empty()) {
        for s in alphabet {
            distinct.insert(HistoryKey::unreadable(DeliveryScope::named(sc), s).render());
            n += 1;
        }
    }
    assert_eq!(distinct.len(), n);
}

#[test]
fn parse_rejects_non_canonical() {
    let p = "fandhe-workshop/history";
    for bad in [
        format!("{p}/repo/a/%e3"),
        format!("{p}/repo/%41/b"),
        format!("{p}/repo/a"),
        format!("{p}/repo/a/b/c"),
        format!("{p}/repo/a/%"),
        format!("{p}/repo/a/%4"),
        format!("{p}/repo/a/%FF"),
        format!("{p}/repo/unreadable"),
        format!("{p}/repo/%75nreadable/%75"),
        format!("{p}x/repo/a/b"),
        format!("{p}//a/b"),
        "fandhe-workshop/meta/repo/a/b".to_string(),
        "other/repo/a/b".to_string(),
        String::new(),
    ] {
        assert_eq!(HistoryKey::parse(&bad), None, "{bad}");
    }
}

#[test]
fn from_ir_without_starts_is_none() {
    let mut ir = base_ir();
    ir.starts.clear();
    assert_eq!(HistoryKey::from_ir(repo(), &ir), None);
}

#[test]
fn pack_prefix_does_not_reach_other_scope_or_pack() {
    let root_prefix = pack_prefix(&DeliveryScope::root(), "a");
    let scoped = HistoryKey::for_pack(repo(), "a", "s").render();
    assert!(!scoped.starts_with(&root_prefix));
    let a = pack_prefix(&repo(), "a");
    assert!(a.ends_with('/'));
    assert!(!HistoryKey::for_pack(repo(), "ab", "s")
        .render()
        .starts_with(&a));
    assert!(scoped.starts_with(&a));
    // 区間や ID に `/` や `..` を含めても、別の名前空間には届かない。
    let evil = HistoryKey::for_pack(DeliveryScope::named("repo/a"), "../x", "s").render();
    assert_eq!(evil.split('/').count(), 5);
    assert!(!evil.starts_with(&a));
}

struct MemStore {
    kind: StorageKind,
    map: RefCell<BTreeMap<String, String>>,
    full: bool,
}

impl MemStore {
    fn new(kind: StorageKind) -> Self {
        MemStore {
            kind,
            map: RefCell::new(BTreeMap::new()),
            full: false,
        }
    }
}

impl HistoryStore for MemStore {
    fn kind(&self) -> StorageKind {
        self.kind
    }
    fn write(&self, key: &HistoryKey, value: &str) -> Result<(), StorageError> {
        if self.full {
            return Err(StorageError::QuotaExceeded);
        }
        self.map
            .borrow_mut()
            .insert(key.render(), value.to_string());
        Ok(())
    }
    fn read(&self, key: &HistoryKey) -> Result<Option<String>, StorageError> {
        Ok(self.map.borrow().get(&key.render()).cloned())
    }
    fn remove(&self, key: &HistoryKey) -> Result<(), StorageError> {
        self.map.borrow_mut().remove(&key.render());
        Ok(())
    }
    fn keys(&self) -> Result<Vec<HistoryKey>, StorageError> {
        Ok(self
            .map
            .borrow()
            .keys()
            .filter_map(|k| HistoryKey::parse(k))
            .collect())
    }
}

fn exercise(store: &dyn HistoryStore) {
    let k1 = HistoryKey::for_pack(repo(), "a", "s");
    let k2 = HistoryKey::for_pack(repo(), "b", "s");
    let k3 = HistoryKey::for_pack(DeliveryScope::root(), "a", "s");
    store.write(&k1, "v1").unwrap();
    assert_eq!(store.read(&k1).unwrap().as_deref(), Some("v1"));
    store.write(&k1, "v2").unwrap();
    assert_eq!(store.read(&k1).unwrap().as_deref(), Some("v2"));
    store.write(&k2, "x").unwrap();
    store.write(&k3, "y").unwrap();
    let mut keys = store.keys().unwrap();
    keys.sort();
    let mut want = vec![k1.clone(), k2.clone(), k3.clone()];
    want.sort();
    assert_eq!(keys, want);
    // SAVE-32 相当: 教材 a の削除は、別の教材・別の区間の鍵を残す。
    for k in store.keys().unwrap() {
        if k.belongs_to(&repo(), "a") {
            store.remove(&k).unwrap();
        }
    }
    assert_eq!(store.read(&k1).unwrap(), None);
    assert!(store.read(&k2).unwrap().is_some());
    assert!(store.read(&k3).unwrap().is_some());
    store.remove(&k1).unwrap();
}

#[test]
fn store_port_roundtrip() {
    let store = MemStore::new(StorageKind::LocalStorage);
    store
        .map
        .borrow_mut()
        .insert("unrelated".to_string(), "z".to_string());
    store
        .map
        .borrow_mut()
        .insert("fandhe-workshop/meta/x".to_string(), "z".to_string());
    exercise(&store);
    assert_eq!(store.map.borrow().len(), 4);

    let boxed: Box<dyn HistoryStore> = Box::new(MemStore::new(StorageKind::IndexedDb));
    assert_eq!(boxed.kind(), StorageKind::IndexedDb);
    exercise(&boxed);
    exercise(&*boxed);
    let by_ref: &dyn HistoryStore = &store;
    assert_eq!(by_ref.kind(), StorageKind::LocalStorage);
}

#[test]
fn quota_error_is_distinguishable() {
    let mut store = MemStore::new(StorageKind::LocalStorage);
    store.full = true;
    let k = HistoryKey::for_pack(repo(), "a", "s");
    let e = store.write(&k, "v").unwrap_err();
    assert_eq!(e, StorageError::QuotaExceeded);
    assert_ne!(e, StorageError::Unavailable);
    assert_ne!(e, StorageError::Failed);
    assert_eq!(e.to_string(), "storage quota exceeded");
    let _: &dyn std::error::Error = &e;
}
