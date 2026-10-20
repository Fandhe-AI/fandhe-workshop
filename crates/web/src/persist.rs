//! 受講中の履歴の保存先（本番の結線。TASK-319 1/3、SAVE-3・SAVE-9・SAVE-28）。
//!
//! `glue::mount_app` が、起動の前に保存先を開いて保存済みの値を読み、離れる直前（`pagehide`）に書き出すために呼ぶ。
//! 保存先の選択の規則は `storage_select`、保留と同期の書き出しは `save_pending`、`pagehide` のリスナーは `leave_flush` が
//! 持ち、この module はそれらを 1 つの保存先と 1 つの保留に結ぶだけである。
//!
//! 試験用の口（`test_hooks` の `debug_storage_*`・`debug_save_*`）は、別の保存先と保留を持ち続ける。そのため試験の設定では
//! IndexedDB の接続が 2 本になるが、どちらも `versionchange` で閉じる（`storage_idb`）ため、互いの更新を塞がない。
//!
//! 書き出す時機は 3 つある。(1) 状態を変える操作のたび（ホストの `update` の末尾が `save_now` を呼ぶ。SAVE-10・TASK-323 1/2）、
//! (2) 下書きだけの変更の後の遅延保存（`glue` のタイマーが `save_deferred` を呼ぶ。連続する変更は畳む。判定は `save_schedule`。
//! TASK-318・SAVE-7）、(3) 離れる直前（`pagehide`）である。打鍵ごとに履歴の全体を書き出す費用を避けるため、下書きは (2) で畳む。
//! どれもホストが「保存する値」を持つときだけ保留へ積んで書く。最初の変更まで保存しない（`EngineHost::history_to_save` が
//! `None`）ので、未操作のページを離れても保存先には何も書かれない（SAVE-28）。
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
    /// 変更の後の遅延保存が書く鍵。`boot_and_mount` が結ぶ（鍵を作れない設定では空のままで、遅延保存は何もしない）。
    static SAVE_KEY: RefCell<Option<HistoryKey>> = const { RefCell::new(None) };
    /// 試験の設定だけ: 変更の後の保存（変更のたび・遅延の両方）を試みた回数。値・鍵は持たない。
    #[cfg(feature = "test-hooks")]
    static AUTOSAVES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
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

/// 保存の値を保留へ積み、保存先が開いていれば書き出す。書けなかった値は保留に残る。
///
/// 結果は最初の失敗の種類で、保存先が開いていない・借用中は `Unavailable`。離れる直前の書き出しは結果を使わない。
fn stage_and_flush(key: &HistoryKey, value: String) -> Result<(), StorageError> {
    PENDING.with(|p| {
        p.stage(key.clone(), value);
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

/// 変更の後の遅延保存の書き出し（`glue` のタイマーが呼ぶ。TASK-318。SAVE-7）。
///
/// 保存しない場合（未操作・上限超え・開けなかった履歴がある・未マウント）は何も書かない（`history_to_save` が `None`）。
/// 呼ぶのは別のスタックのタイマーの中だけで、Runtime・`dispatch_action` には触れない（読み取りの `history_to_save` と保留だけ）。
pub(crate) fn save_deferred() {
    let Some(key) = SAVE_KEY.with(|k| k.try_borrow().ok().and_then(|k| k.clone())) else {
        return;
    };
    let Some(v) = crate::glue::history_to_save() else {
        return;
    };
    let result = stage_and_flush(&key, v);
    #[cfg(feature = "test-hooks")]
    AUTOSAVES.with(|c| c.set(c.get().saturating_add(1)));
    // 失敗・成功を保存通知へ反映する（SAVE-10）。配送は別のスタックのタイマーで行う。
    crate::glue::report_save_result(result);
}

/// 本番の保存先の種類（`local` か `idb`）。開いていなければ `None`。試験の設定だけ。
#[cfg(feature = "test-hooks")]
pub(crate) fn storage_kind_name() -> Option<&'static str> {
    use fandhe_workshop_contract::storage::StorageKind;
    STORAGE.with(|s| {
        let slot = s.try_borrow().ok()?;
        Some(match slot.as_ref()?.kind() {
            StorageKind::LocalStorage => "local",
            StorageKind::IndexedDb => "idb",
        })
    })
}

/// 変更の後の保存（変更のたび・遅延の両方）を試みた回数。試験の設定だけ。
#[cfg(feature = "test-hooks")]
pub(crate) fn autosave_count() -> u32 {
    AUTOSAVES.with(std::cell::Cell::get)
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
        let _ = stage_and_flush(&key, v);
    })
}

/// 変更のたびの保存（SAVE-10）。`value` を `key` の保留に積んで書き、最初の失敗の種類を返す。
///
/// 保存先が開いていない・借用中は `Unavailable`。書けなかった値は保留に残る（既存の保存は、保存先の `write` が失敗しても
/// 同じ鍵の旧い値を消さない）。値・鍵の文字列は返さない。
fn save_now(key: &HistoryKey, value: &str) -> Result<(), StorageError> {
    #[cfg(feature = "test-hooks")]
    AUTOSAVES.with(|c| c.set(c.get().saturating_add(1)));
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
    crate::glue::mount_host(root_id, host)?;
    SAVE_KEY.with(|k| {
        if let Ok(mut slot) = k.try_borrow_mut() {
            *slot = Some(key.clone());
        }
    });
    install_save_on_leave(key, crate::glue::history_to_save)
}
