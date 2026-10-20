//! 受講中の履歴の保存先（本番の結線。TASK-319 1/3、SAVE-3・SAVE-9・SAVE-28）。
//!
//! `glue::mount_app` が、起動の前に保存先を開いて保存済みの値を読み、離れる直前（`pagehide`）に書き出すために呼ぶ。
//! 保存先の選択の規則は `storage_select`、保留と同期の書き出しは `save_pending`、`pagehide` のリスナーは `leave_flush` が
//! 持ち、この module はそれらを 1 つの保存先と 1 つの保留に結ぶだけである。
//!
//! 試験用の口（`test_hooks` の `debug_storage_*`・`debug_save_*`）は、別の保存先と保留を持ち続ける。そのため試験の設定では
//! IndexedDB の接続が 2 本になるが、どちらも `versionchange` で閉じる（`storage_idb`）ため、互いの更新を塞がない。
//!
//! 書き出す時機は 2 つある。状態を変える操作のたび（ホストの `update` の末尾が [`save_now`] を呼ぶ。SAVE-10・TASK-323 1/2）と、
//! 離れる直前（`pagehide`）である。下書きだけの変更では書かない（打鍵ごとに履歴の全体を書き出す費用を避け、下書きは離れる直前に書く）。
//! どちらもホストが「保存する値」を持つときだけ保留へ積んで書く。最初の変更まで保存しない（`EngineHost::history_to_save` が
//! `None`）ので、未操作のページを離れても保存先には何も書かれない（SAVE-28）。保存の遅延・畳み込みは置かない（TASK-318）。
//! IndexedDB への書き込みの失敗は確定が非同期で届くため、ホストが次の `update` で probe（[`async_save_failure`]）を引いて表示する（SAVE-11）。
//! 変更のたびの書き込みが容量超過で失敗したときは、ホストが理由を表示する。書けなかった値は保留に残り、次の書き出しで再び試す。
//!
//! 鍵は呼び出し元が `HistoryKey` の構築子で作ったものだけを受ける。値・鍵の文字列は、結果にも記録にも出さない
//! （履歴は下書きを含む受講者のデータである）。

use std::cell::RefCell;

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore, StorageError};
use wasm_bindgen::JsValue;

use crate::host::EngineHost;
use crate::leave_flush::install_pagehide_flush_as;
use crate::save_pending::PendingSave;
use crate::storage_idb::{open_history_storage, OpenedStorage};

thread_local! {
    /// 開いた保存先。準備完了の合図の後に入る。
    static STORAGE: RefCell<Option<OpenedStorage>> = const { RefCell::new(None) };
    /// 離れる直前に書き出す未保存の値の保留。
    static PENDING: PendingSave = PendingSave::new();
}

/// 保存先を非同期に開く。準備ができたら（開けなくても localStorage だけで）`on_ready` を 1 回呼ぶ。
pub fn open(on_ready: impl FnOnce() + 'static) {
    open_history_storage(move |opened| {
        STORAGE.with(|s| {
            if let Ok(mut slot) = s.try_borrow_mut() {
                *slot = Some(opened);
            }
        });
        on_ready();
    });
}

/// 保存済みの値を読む。無ければ `Ok(None)`。保存先が開いていない・借用中は `Unavailable`。
///
/// # Errors
///
/// 読めなかった種類。呼び出し元は、読めなかったまま保存すると保存済みの値を失うため、保存を止める。
pub fn read(key: &HistoryKey) -> Result<Option<String>, StorageError> {
    STORAGE.with(|s| {
        let slot = s.try_borrow().map_err(|_| StorageError::Unavailable)?;
        slot.as_ref().ok_or(StorageError::Unavailable)?.read(key)
    })
}

/// `pagehide` で `value` が返す値を `key` に書き出すリスナーを付ける（登録は 1 回だけ）。
///
/// `value` は離れる直前に呼ばれ、保存しないとき（未操作・開けなかった履歴がある）は `None` を返す。
/// 書けなかった値は保留に残る（次の書き出しで再び試す）。
///
/// # Errors
///
/// `window` が無いなど、リスナーを付けられなかったとき。
pub fn install_save_on_leave(
    key: HistoryKey,
    value: impl Fn() -> Option<String> + 'static,
) -> Result<(), JsValue> {
    install_pagehide_flush_as("history", move || {
        let Some(v) = value() else {
            return;
        };
        PENDING.with(|p| {
            p.stage(key.clone(), v);
            STORAGE.with(|s| {
                if let Ok(slot) = s.try_borrow() {
                    if let Some(st) = slot.as_ref() {
                        p.flush(st);
                    }
                }
            });
        });
    })
}

/// 変更のたびの保存（SAVE-10）。`value` を `key` の保留に積んで書き、最初の失敗の種類を返す。
///
/// 保存先が開いていない・借用中は `Unavailable`。書けなかった値は保留に残る（既存の保存は、保存先の `write` が失敗しても
/// 同じ鍵の旧い値を消さない）。値・鍵の文字列は返さない。
fn save_now(key: &HistoryKey, value: &str) -> Result<(), StorageError> {
    PENDING.with(|p| {
        p.stage(key.clone(), value.to_owned());
        STORAGE.with(|s| {
            let slot = s.try_borrow().map_err(|_| StorageError::Unavailable)?;
            let st = slot.as_ref().ok_or(StorageError::Unavailable)?;
            match p.flush(st).first_error {
                Some(e) => Err(e),
                None => Ok(()),
            }
        })
    })
}

/// IndexedDB の保存失敗の probe の実体（SAVE-11。TASK-323 2/2）。読み取り専用で、ホストが `update` のたびに引く。
///
/// 全件を読めなかった保存先は常に `Unavailable`、それ以外は直近の put の失敗の種類（成功の確定で消える）を返す。
/// IndexedDB の確定は非同期で届くため、ホストへ押し込む経路は無い（`state_paths` の規則）。ホストが次の `update` で引く。
/// 保存先が未オープン・借用中は `None`（パニックしない）。
fn async_save_failure() -> Option<StorageError> {
    STORAGE.with(|s| {
        let slot = s.try_borrow().ok()?;
        let st = slot.as_ref()?;
        if st.idb_unreadable() {
            return Some(StorageError::Unavailable);
        }
        st.last_async_error()
    })
}

/// 配信先の区間。URL のパス名（サブパスの下で配られても鍵が衝突しない）から `DeliveryScope` の構築子で作る。
fn delivery_scope() -> DeliveryScope {
    let path = web_sys::window()
        .and_then(|w| w.location().pathname().ok())
        .unwrap_or_default();
    DeliveryScope::from_pathname(&path)
}

/// 起動の分岐を行ってマウントする Promise を返す（`glue::mount_app` の実体。TASK-319 1/3）。
///
/// ホストが実行環境を持たない（本番の設定。IR の取得は後続のタスク）ときは、保存先を開かずに今までどおり即座にマウントする。
/// 持つとき（試験の設定）は、保存先を開いて保存済みの値を読み、`EngineHost::boot` で新しく始めるか復元するかを決めてから
/// マウントし、離れる直前の書き出しを付ける。保存済みの値を読めなかったときは、上書きして失わないよう保存を止める。
/// 要素が無い・二重のマウント・配線の失敗は Promise の拒否になる（`app.js` が `data-mounted="error"` にする）。
pub fn mount_with_history(root_id: &str) -> js_sys::Promise {
    let host = mount_host_for_page();
    let Some(key) = host.history_key(delivery_scope()) else {
        return match crate::glue::mount_host(root_id, host) {
            Ok(()) => js_sys::Promise::resolve(&JsValue::UNDEFINED),
            Err(e) => js_sys::Promise::reject(&e),
        };
    };
    let mut pending = Some((root_id.to_owned(), host, key));
    js_sys::Promise::new(&mut move |resolve, reject| {
        let Some((root_id, host, key)) = pending.take() else {
            return;
        };
        open(move || {
            let outcome = boot_and_mount(&root_id, host, key);
            let _ = match outcome {
                Ok(()) => resolve.call0(&JsValue::UNDEFINED),
                Err(e) => reject.call1(&JsValue::UNDEFINED, &e),
            };
        });
    })
}

/// mount するホスト。試験の設定だけ、起動の前に URL のクエリ（`?fixture=reload-points`・`?fixture=no-input` の完全一致）で教材を選ぶ（TASK-319 3/3）。
///
/// 起動の分岐（`boot`）と保存先の鍵（`pack.id`）は選んだ教材で決まるため、選択は必ず起動の前に行う。
/// 配信する設定では選択の経路を持たない（クエリを読む処理は試験の口の `test_hooks.rs` にだけ置き、`tick_interval` の試験が配信する経路に URL の読み取りが無いことを固定している）。
/// クエリは選択の名前としてだけ使い、経路・URL には使わない。
fn mount_host_for_page() -> EngineHost {
    #[cfg(feature = "test-hooks")]
    {
        EngineHost::for_mount_with(crate::test_hooks::mount_fixture_from_url())
    }
    #[cfg(not(feature = "test-hooks"))]
    {
        EngineHost::for_mount()
    }
}

/// 保存先から読んで起動の分岐を行い、マウントして離れる直前の書き出しを付ける。
fn boot_and_mount(root_id: &str, mut host: EngineHost, key: HistoryKey) -> Result<(), JsValue> {
    match read(&key) {
        Ok(saved) => {
            host.boot(saved.as_deref().map(str::as_bytes));
        }
        Err(_) => {
            host.boot(None);
            host.block_saving();
        }
    }
    // 変更のたびの保存の口は mount の前に結ぶ（mount の後はホストを `&mut` で触れない）。
    let saver_key = key.clone();
    host.set_history_saver(Box::new(move |v| save_now(&saver_key, v)));
    host.set_save_failure_probe(Box::new(async_save_failure));
    crate::glue::mount_host(root_id, host)?;
    install_save_on_leave(key, crate::glue::history_to_save)
}
