//! 保存の直前の起動番号の照合の試験（SAVE-12・TASK-324 1/2）。
//!
//! 2 つのタブは、同じ localStorage の偽物（meta と履歴で共有）を持つ 2 つの `BootGuard` と保存先で模す。
//! 古いタブの書き込みが保存領域に届かないこと、新しいタブの書き込みは届くこと（陽性の対照）、
//! 照合を外すと上書きされること（対照）を確かめる。値・鍵の文字列は失敗の文に出さない。試験では `unwrap` を使ってよい。

#[path = "common/storage_fakes.rs"]
mod storage_fakes;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

use fandhe_workshop_contract::storage::{
    pack_prefix, DeliveryScope, HistoryKey, HistoryStore, StorageError,
};
use fandhe_workshop_web::boot_guard::{
    boot_meta_key, format_boot, parse_boot, BootGuard, Guarded, BOOT_META_PREFIX,
};
use fandhe_workshop_web::save_pending::PendingSave;
use fandhe_workshop_web::storage_select::{HistoryStorage, SyncKv};
use storage_fakes::{key, FakeIdb, FakeLocal};

type Storage = HistoryStorage<Rc<FakeLocal>, Rc<FakeIdb>>;

fn storage(local: &Rc<FakeLocal>, idb: Option<&Rc<FakeIdb>>) -> Storage {
    HistoryStorage::new(Rc::clone(local), idb.map(Rc::clone), BTreeMap::new())
}

fn history_raw(local: &FakeLocal, k: &HistoryKey) -> Option<String> {
    local.raw(&k.render())
}

#[test]
fn 古いタブの書き込みは保存先に届かず_新しいタブの書き込みは届く() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 0xa);
    let b = BootGuard::claim(Rc::clone(&local), &k, 0xb);
    let st = storage(&local, None);

    assert!(b.is_current(), "新しいタブは書ける（陽性の対照）");
    Guarded {
        inner: &st,
        guard: &b,
    }
    .write(&k, "from-b")
    .unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-b"));

    assert!(!a.is_stale(), "照合の前は古いと決めない");
    Guarded {
        inner: &st,
        guard: &a,
    }
    .write(&k, "from-a")
    .unwrap();
    assert!(a.is_stale());
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-b"));
}

#[test]
fn 古いタブの保留の書き出しは新しいタブの保存を上書きせず_保留は空になる() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 1);
    let _b = BootGuard::claim(Rc::clone(&local), &k, 2);
    local.set(&k.render(), "from-b").unwrap();
    let st = storage(&local, None);

    let pending = PendingSave::new();
    pending.stage(k.clone(), "from-a".to_owned());
    let out = pending.flush(&Guarded {
        inner: &st,
        guard: &a,
    });
    assert_eq!(out.first_error, None);
    assert!(!pending.has_pending());
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-b"));
}

#[test]
fn 対照_照合が無いと古いタブが新しいタブの保存を上書きする() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let _a = BootGuard::claim(Rc::clone(&local), &k, 1);
    let _b = BootGuard::claim(Rc::clone(&local), &k, 2);
    let st = storage(&local, None);
    st.write(&k, "from-b").unwrap();
    // 世代（メモリの中の話）を上げても、保存先には関係しない。照合が無ければ上書きされる。
    st.write(&k, "from-a").unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-a"));
}

#[test]
fn 古いタブは戻らない() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 1);
    let b = BootGuard::claim(Rc::clone(&local), &k, 2);
    assert!(!a.is_current());
    assert!(a.is_stale());
    // meta が A の値に戻っても、一度古いと分かったタブは書かない。
    local.set(&boot_meta_key(&k), &format_boot(1, 1)).unwrap();
    assert!(!a.is_current());
    assert!(
        !b.is_current(),
        "meta が書き換わった側も一致しなければ書かない"
    );
}

#[test]
fn 別の鍵のタブ同士は互いを古くしない() {
    let local = Rc::new(FakeLocal::default());
    let scope_a = HistoryKey::for_pack(DeliveryScope::named("repo"), "pack-a", "s-1");
    let others = [
        HistoryKey::for_pack(DeliveryScope::named("repo"), "pack-b", "s-1"),
        HistoryKey::for_pack(DeliveryScope::named("repo"), "pack-a", "s-2"),
        HistoryKey::for_pack(DeliveryScope::named("other"), "pack-a", "s-1"),
    ];
    let a = BootGuard::claim(Rc::clone(&local), &scope_a, 1);
    let guards: Vec<_> = others
        .iter()
        .map(|o| BootGuard::claim(Rc::clone(&local), o, 2))
        .collect();
    assert!(a.is_current());
    assert!(!a.is_stale());
    assert!(guards.iter().all(BootGuard::is_current));
}

struct FailingSet {
    inner: Rc<FakeLocal>,
    set_error: Option<StorageError>,
    get_error: Option<StorageError>,
}

impl SyncKv for FailingSet {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        match self.get_error {
            Some(e) => Err(e),
            None => self.inner.get(key),
        }
    }
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        match self.set_error {
            Some(e) => Err(e),
            None => self.inner.set(key, value),
        }
    }
    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.inner.remove(key)
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        self.inner.keys()
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        self.inner.total_utf16()
    }
}

#[test]
fn 取得の書き込みが失敗すると書かない() {
    for e in [StorageError::Unavailable, StorageError::QuotaExceeded] {
        let local = Rc::new(FakeLocal::default());
        let k = key("pack-a");
        let kv = FailingSet {
            inner: Rc::clone(&local),
            set_error: Some(e),
            get_error: None,
        };
        let g = BootGuard::claim(kv, &k, 1);
        assert!(!g.is_current());
        assert!(!g.is_stale());
        let st = storage(&local, None);
        let r = Guarded {
            inner: &st,
            guard: &g,
        }
        .write(&k, "v");
        assert_eq!(r, Err(e), "取得の失敗は保存の失敗として返す");
        assert_eq!(history_raw(&local, &k), None);
    }
}

#[test]
fn 照合の読み取りが失敗すると書かないが_古いとは決めない() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let unreadable = FailingSet {
        inner: Rc::clone(&local),
        set_error: None,
        get_error: Some(StorageError::Failed),
    };
    // get が常に失敗する kv では、取得は 1 から始まり meta に書けるが、照合は読めず書かない。
    let h = BootGuard::claim(unreadable, &k, 3);
    assert!(!h.is_current());
    assert!(!h.is_stale());
    let st = storage(&local, None);
    let r = Guarded {
        inner: &st,
        guard: &h,
    }
    .write(&k, "v");
    assert_eq!(r, Err(StorageError::Failed), "読めない失敗は返す");
    assert_eq!(history_raw(&local, &k), None);
}

#[test]
fn meta_が消えていると書かずに失敗を返す() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let g = BootGuard::claim(Rc::clone(&local), &k, 1);
    local.remove(&boot_meta_key(&k)).unwrap();
    let st = storage(&local, None);
    let r = Guarded {
        inner: &st,
        guard: &g,
    }
    .write(&k, "v");
    assert_eq!(r, Err(StorageError::Failed));
    assert!(!g.is_stale());
    assert_eq!(history_raw(&local, &k), None);
}

#[test]
fn 書いた直後の再照合で追い越されていれば以後は書かない() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 1);
    let st = storage(&local, None);
    let ga = Guarded {
        inner: &st,
        guard: &a,
    };
    ga.write(&k, "1").unwrap();
    let _b = BootGuard::claim(Rc::clone(&local), &k, 2);
    ga.write(&k, "2").unwrap();
    assert!(a.is_stale());
    assert_eq!(history_raw(&local, &k).as_deref(), Some("1"));
}

#[test]
fn 起動番号は前の値に_1_を足し_壊れた値や無い値からは_1_で始める() {
    let k = key("pack-a");
    let meta = boot_meta_key(&k);
    let cases: [(Option<&str>, &str); 8] = [
        (None, "1:00000001"),
        (Some("4:0000000a"), "5:00000001"),
        (Some("abc"), "1:00000001"),
        (Some("+4:0000000a"), "1:00000001"),
        (Some("-4:0000000a"), "1:00000001"),
        (Some(":0000000a"), "1:00000001"),
        (Some("4"), "1:00000001"),
        (
            Some("18446744073709551615:0000000a"),
            "18446744073709551615:00000001",
        ),
    ];
    for (prev, want) in cases {
        let local = Rc::new(FakeLocal::default());
        if let Some(p) = prev {
            local.set(&meta, p).unwrap();
        }
        let _g = BootGuard::claim(Rc::clone(&local), &k, 1);
        assert_eq!(local.raw(&meta).as_deref(), Some(want), "case {prev:?}");
    }
}

#[test]
fn 値の解析は厳格() {
    assert_eq!(parse_boot("7:0000abcd"), Some(7));
    for bad in [
        "",
        "7",
        "7:",
        "7:abcd",
        "7:0000ABCD",
        "7:0000abcdef",
        "x:0000abcd",
        "7:0000abcg",
        "7:0000abcd:1",
        "7 :0000abcd",
    ] {
        assert_eq!(parse_boot(bad), None, "{bad:?}");
    }
    assert_eq!(format_boot(3, 0xab), "3:000000ab");
}

#[test]
fn 同じ番号でも_tag_が違えば先に取得した側は古くなる() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 1);
    // 同時の起動を模す。同じ番号 1 を別の tag で書く。
    local.set(&boot_meta_key(&k), &format_boot(1, 2)).unwrap();
    assert!(!a.is_current());
    assert!(a.is_stale());
}

#[test]
fn 切り替えた_indexeddb_の保存先でも古いタブは_put_を発行しない() {
    let local = Rc::new(FakeLocal::default());
    let idb = Rc::new(FakeIdb::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 1);
    let b = BootGuard::claim(Rc::clone(&local), &k, 2);
    let st = storage(&local, Some(&idb));
    // 閾値を超える値で IndexedDB へ切り替える。
    let big = "x".repeat(5_000_000);
    Guarded {
        inner: &st,
        guard: &a,
    }
    .write(&k, &big)
    .unwrap();
    assert_eq!(idb.pending_len(), 0, "古いタブは put を発行しない");
    Guarded {
        inner: &st,
        guard: &b,
    }
    .write(&k, &big)
    .unwrap();
    assert_eq!(
        idb.pending_len(),
        1,
        "新しいタブは put を発行する（陽性の対照）"
    );
}

#[test]
fn meta_の鍵は履歴の鍵として解析されず一覧と削除の対象に入らない() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let meta = boot_meta_key(&k);
    assert!(meta.starts_with(BOOT_META_PREFIX));
    assert_eq!(HistoryKey::parse(&meta), None);
    let _g = BootGuard::claim(Rc::clone(&local), &k, 1);
    let st = storage(&local, None);
    st.write(&k, "v").unwrap();
    assert_eq!(st.keys().unwrap(), vec![k.clone()]);
    let prefix = pack_prefix(&DeliveryScope::named("repo"), "pack-a");
    assert!(!meta.starts_with(&prefix));
}

#[test]
fn 書き出しの経路は_flush_guarded_の_1_か所だけ() {
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/persist.rs"))
        .expect("persist.rs");
    let code: String = src
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        code.matches(".flush(").count(),
        1,
        "flush の呼び出しは 1 か所"
    );
    let start = code.find("fn flush_guarded").expect("flush_guarded");
    let call = code.find(".flush(").expect("flush");
    assert!(call > start, "flush は flush_guarded の中にある");
    assert!(code[call..].contains("Guarded"), "照合の包みを通す");
}

/// 容量の回復を模す kv。`full` が真の間は `set` が容量超過になる。
struct Toggle {
    inner: Rc<FakeLocal>,
    full: Rc<std::cell::Cell<bool>>,
}

impl SyncKv for Toggle {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        self.inner.get(key)
    }
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        if self.full.get() {
            Err(StorageError::QuotaExceeded)
        } else {
            self.inner.set(key, value)
        }
    }
    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.inner.remove(key)
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        self.inner.keys()
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        self.inner.total_utf16()
    }
}

fn toggled(
    local: &Rc<FakeLocal>,
    k: &HistoryKey,
) -> (BootGuard<Toggle>, Rc<std::cell::Cell<bool>>) {
    let full = Rc::new(std::cell::Cell::new(true));
    let kv = Toggle {
        inner: Rc::clone(local),
        full: Rc::clone(&full),
    };
    (BootGuard::claim(kv, k, 1), full)
}

#[test]
fn 容量が戻れば取得をやり直して保存を再開する() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let (g, full) = toggled(&local, &k);
    assert_eq!(g.verify(), Err(StorageError::QuotaExceeded));
    full.set(false);
    assert_eq!(g.verify(), Ok(true));
    let st = storage(&local, None);
    Guarded {
        inner: &st,
        guard: &g,
    }
    .write(&k, "v")
    .unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("v"));
}

#[test]
fn 取得に失敗している間に別のタブが起動していれば再取得せず古いタブになる() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let (g, full) = toggled(&local, &k);
    full.set(false);
    // 後から起動したタブが先に取得する。
    let b = BootGuard::claim(Rc::clone(&local), &k, 2);
    assert_eq!(g.verify(), Ok(false));
    assert!(g.is_stale());
    assert!(b.is_current(), "後から起動したタブを追い越さない");
}

#[test]
fn 取得に失敗している間に_meta_が消えても古いタブとは決めず失敗を返す() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let _earlier = BootGuard::claim(Rc::clone(&local), &k, 2);
    let (g, full) = toggled(&local, &k);
    full.set(false);
    // 容量超過の回復などで保存領域が消えた。
    local.remove(&boot_meta_key(&k)).unwrap();
    assert_eq!(g.verify(), Err(StorageError::Failed));
    assert!(!g.is_stale(), "消失は他のタブの取得ではない");
}

/// 一度だけ走らせる割り込み。
type Hook = Rc<RefCell<Option<Box<dyn FnOnce()>>>>;

/// 照合の窓（SAVE-12 の限界。`docs/known-gaps.md`）を模す kv。meta の読み取りが値を返す直前に 1 回だけ `hook` を走らせる。
/// 照合の後・書き込みの前に別のタブが取得する順序を、同期の試験の中で再現する。
/// `lag` は、割り込みの後もなお割り込みの前の値を返す meta の読み取りの回数（タブ間の伝播の遅れを模す）。
/// 0 なら直後の読み取りから他のタブの値が見える（同期の fake の素の振る舞い）。
struct Interpose {
    inner: Rc<FakeLocal>,
    meta: String,
    hook: Hook,
    lag: Cell<usize>,
    seen: RefCell<Option<Option<String>>>,
}

impl SyncKv for Interpose {
    fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        if key != self.meta {
            return self.inner.get(key);
        }
        if let Some(old) = self.seen.borrow().clone() {
            if self.lag.get() > 0 {
                self.lag.set(self.lag.get() - 1);
                return Ok(old);
            }
        }
        let out = self.inner.get(key);
        let hook = self.hook.borrow_mut().take();
        if let Some(f) = hook {
            f();
            *self.seen.borrow_mut() = Some(out.clone()?);
        }
        out
    }
    fn set(&self, key: &str, value: &str) -> Result<(), StorageError> {
        self.inner.set(key, value)
    }
    fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.inner.remove(key)
    }
    fn keys(&self) -> Result<Vec<String>, StorageError> {
        self.inner.keys()
    }
    fn total_utf16(&self) -> Result<u64, StorageError> {
        self.inner.total_utf16()
    }
}

type Slot = Rc<RefCell<Option<BootGuard<Rc<FakeLocal>>>>>;

/// タブ A を取得し、A の次の照合の読み取りの直後に別のタブ B が取得するよう仕掛ける。B は `Slot` に入る。
/// `lag` は [`Interpose`] の同名の欄。
fn racing_tabs(local: &Rc<FakeLocal>, k: &HistoryKey, lag: usize) -> (BootGuard<Interpose>, Slot) {
    let hook: Hook = Rc::new(RefCell::new(None));
    let a = BootGuard::claim(
        Interpose {
            inner: Rc::clone(local),
            meta: boot_meta_key(k),
            hook: Rc::clone(&hook),
            lag: Cell::new(lag),
            seen: RefCell::new(None),
        },
        k,
        0xa,
    );
    // 取得の読み取りでは発火させない。仕掛けるのは取得の後。
    let slot: Slot = Rc::new(RefCell::new(None));
    let (l, kk, s) = (Rc::clone(local), k.clone(), Rc::clone(&slot));
    *hook.borrow_mut() = Some(Box::new(move || {
        *s.borrow_mut() = Some(BootGuard::claim(l, &kk, 0xb));
    }));
    (a, slot)
}

#[test]
fn 限界_照合の後に別のタブが取得すると_1_回だけ書くが_後のタブの保存が残る() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let (a, slot) = racing_tabs(&local, &k, 0);
    let st = storage(&local, None);
    let ga = Guarded {
        inner: &st,
        guard: &a,
    };

    // A の照合（古い meta を読む）→ B の取得 → A の書き込み。この 1 回は届く（原子的でない限界）。
    ga.write(&k, "from-a").unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-a"));
    // この fake にはタブ間の遅れが無いので、書いた直後の再照合が B の値を読んで古いタブと記憶する。
    // 実ブラウザで同じタスクの中の再照合が B の値を読むか（効くか）は未確認。効かない場合は
    // `限界_書いた直後の再照合が古い値を読んでも次の保存の照合で止まる` が固定する。
    assert!(a.is_stale(), "fake では書いた直後の再照合が効く");
    let b = slot
        .borrow_mut()
        .take()
        .expect("B は A の照合の中で取得した");

    // A の次の保存は届かない（ここでは上の再照合で立てた古いタブの記憶が止める）。
    ga.write(&k, "from-a-2").unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-a"));

    // B が操作して保存すれば、B の値が上書きして残る。
    Guarded {
        inner: &st,
        guard: &b,
    }
    .write(&k, "from-b")
    .unwrap();
    ga.write(&k, "from-a-3").unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-b"));
}

#[test]
fn 交互に_10_回ずつ書いても保存先には新しいタブの最後の書き込みだけが残る() {
    // 合流の規則は足さない（D-56。SAVE-13）。古いタブの書き込みは何度重ねても新しいタブの値を壊さない。
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let a = BootGuard::claim(Rc::clone(&local), &k, 0xa);
    let b = BootGuard::claim(Rc::clone(&local), &k, 0xb);
    let st = storage(&local, None);
    let ga = Guarded {
        inner: &st,
        guard: &a,
    };
    let gb = Guarded {
        inner: &st,
        guard: &b,
    };

    for i in 1..=10 {
        let before = history_raw(&local, &k);
        ga.write(&k, &format!("a-{i}")).unwrap();
        assert_eq!(
            history_raw(&local, &k),
            before,
            "古いタブの書き込みは届かない"
        );
        let from_b = format!("b-{i}");
        gb.write(&k, &from_b).unwrap();
        assert_eq!(
            history_raw(&local, &k).as_deref(),
            Some(from_b.as_str()),
            "新しいタブの書き込みは届く（陽性の対照）"
        );
    }
    assert!(a.is_stale());
    assert!(!b.is_stale());
}

#[test]
fn 限界_書いた直後の再照合が古い値を読んでも次の保存の照合で止まる() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    // 割り込みの後の 1 回（書いた直後の再照合）だけ、B の取得の前の値を読ませる。
    let (a, slot) = racing_tabs(&local, &k, 1);
    let st = storage(&local, None);
    let ga = Guarded {
        inner: &st,
        guard: &a,
    };

    ga.write(&k, "from-a").unwrap();
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-a"));
    assert!(!a.is_stale(), "再照合は古い値を読み、古いとは決めていない");
    assert!(slot.borrow().is_some(), "B は A の照合の中で取得した");

    // 次の保存の照合が B の値を読み、書かない。
    ga.write(&k, "from-a-2").unwrap();
    assert!(a.is_stale());
    assert_eq!(history_raw(&local, &k).as_deref(), Some("from-a"));
}

#[test]
fn 限界_照合の後に取得したタブが未操作なら_先のタブの_1_回分が残る() {
    let local = Rc::new(FakeLocal::default());
    let k = key("pack-a");
    let (a, slot) = racing_tabs(&local, &k, 0);
    let st = storage(&local, None);
    Guarded {
        inner: &st,
        guard: &a,
    }
    .write(&k, "from-a")
    .unwrap();
    let b = slot
        .borrow_mut()
        .take()
        .expect("B は A の照合の中で取得した");
    assert!(b.is_current(), "B は追い越されていない");

    // 主張の本体: B を未操作のまま再読み込みすると、新しい取得のもとで A の最後の 1 回分を読む。
    drop(b);
    let reloaded = BootGuard::claim(Rc::clone(&local), &k, 0xc);
    assert_eq!(st.read(&k).unwrap().as_deref(), Some("from-a"));
    assert!(reloaded.is_current());
    // 付随の確認: A が以後書かないこと。fake では書いた直後の再照合で既に古いと記憶しているため、
    // ここは再照合の効果を見ているだけである（効かない場合は上の `限界_書いた直後の…` が次の保存の照合で止まることを固定する）。
    assert!(!a.is_current(), "A は以後も書かない");
}
