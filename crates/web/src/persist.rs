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
//! 3 つとも、書く直前の値を `EngineHost::secret_gate`（遅延保存と離れる直前は `glue::secret_gate` 経由）で調べ、確認済みでない秘密情報らしい文字列があれば保留へ積まない（SAFE-27。TASK-164 1/3）。
//! 警告の表示は (1)(2) が行い、(3) は積まないだけである。選択（このまま保存する・この入力を残さない）はホストの `update` が持ち（SAFE-28。TASK-164 3/3）、
//! 「このまま保存する」で確認済みにした値は (3) でも書く。「この入力を残さない」の読み直しは、`set_saved_refresh` で保存先の実データを取り直してから、`set_saved_reader` で結んだ `read` を使う。
//! (3) で積まないのは D-44・SAFE-44 で決めた振る舞いで、選ぶまで、その間の進行も含めて保存先へ書かない（放置して閉じても書かれない）。警告の状態も確認済みも保存しないため、開き直した後は直前の無害な保存から復元され、警告は出ない。
//! 書き出しは必ず保存の直前の起動番号の照合（SAVE-12・`boot_guard`。TASK-324 1/2）を通す。他のタブに追い越された古いタブは、
//! 保存先へ書かない（`flush_guarded` が唯一の関所）。試験用の保存先の口（`debug_storage_*`・`debug_save_*`）は照合を通さない。
//! IndexedDB への書き込みの失敗は確定が非同期で届くため、ホストが次の `update` で probe（[`async_save_failure`]）を引いて表示する（SAVE-11）。
//! 変更のたびの書き込みが容量超過で失敗したときは、ホストが理由を表示する。書けなかった値は保留に残り、次の書き出しで再び試す。
//!
//! 鍵は呼び出し元が `HistoryKey` の構築子で作ったものだけを受ける。値・鍵の文字列は、結果にも記録にも出さない
//! （履歴は下書きを含む受講者のデータである）。

use std::cell::RefCell;

use fandhe_workshop_contract::storage::{DeliveryScope, HistoryKey, HistoryStore, StorageError};
use fandhe_workshop_core::learner_os::estimate;
use wasm_bindgen::JsValue;

use crate::boot_guard::{BootGuard, Guarded};
use crate::host::EngineHost;
use crate::leave_flush::install_pagehide_flush_as;
use crate::save_pending::{FlushOutcome, PendingSave};
use crate::storage_idb::{open_history_storage, OpenedStorage};
use crate::storage_local::LocalStore;
use crate::storage_select::SyncKv;

thread_local! {
    /// 開いた保存先。準備完了の合図の後に入る。
    static STORAGE: RefCell<Option<OpenedStorage>> = const { RefCell::new(None) };
    /// 離れる直前に書き出す未保存の値の保留。
    static PENDING: PendingSave = PendingSave::new();
    /// このページの起動番号（SAVE-12）。鍵が決まった直後に 1 回だけ取得する。取得前は書かない。
    static GUARD: RefCell<Option<BootGuard<LocalStore>>> = const { RefCell::new(None) };
    /// 変更の後の遅延保存が書く鍵。`boot_and_mount` が結ぶ（鍵を作れない設定では空のままで、遅延保存は何もしない）。
    static SAVE_KEY: RefCell<Option<HistoryKey>> = const { RefCell::new(None) };
    /// 試験の設定だけ: 変更の後の保存（変更のたび・遅延の両方）を試みた回数。値・鍵は持たない。
    #[cfg(feature = "test-hooks")]
    static AUTOSAVES: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    /// 試験の設定だけ: 秘密情報らしい文字列を見つけて保存を止めた回数（遅延・離れる直前の両方）。値・形は持たない。
    #[cfg(feature = "test-hooks")]
    static SECRET_HOLDS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
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

/// 保存先の実データから写しを取り直し、終わったら `done` を呼ぶ（「この入力を残さない」の読み直し用。SAFE-28）。
///
/// 保存先が開いていない・借用中は `done(Err(Unavailable))`。IndexedDB の取得は非同期で、`done` は後から届く。
pub fn refetch(key: &HistoryKey, done: Box<dyn FnOnce(Result<(), StorageError>)>) {
    let mut done = Some(done);
    STORAGE.with(|s| {
        if let Ok(slot) = s.try_borrow() {
            if let Some(storage) = slot.as_ref() {
                if let Some(d) = done.take() {
                    storage.refetch(key, d);
                }
            }
        }
    });
    if let Some(d) = done {
        d(Err(StorageError::Unavailable));
    }
}

/// 保存の値を保留へ積み、保存先が開いていれば書き出す。書けなかった値は保留に残る。
///
/// 結果は最初の失敗の種類で、保存先が開いていない・借用中は `Unavailable`。離れる直前の書き出しは結果を使わない。
fn stage_and_flush(key: &HistoryKey, value: String) -> Result<(), StorageError> {
    PENDING.with(|p| {
        p.stage(key.clone(), value);
        match flush_guarded(p) {
            Some(outcome) => match outcome.first_error {
                Some(e) => Err(e),
                None => Ok(()),
            },
            None => Err(StorageError::Unavailable),
        }
    })
}

/// 保存済みの履歴の削除（SAVE-32。TASK-341）。指定した配信先・教材の鍵を保存先から消し、全 delete の確定の後に `done` へ件数を渡す。
///
/// 先に未保存の保留を捨て（離れる直前・遅延保存の書き出しが消した鍵を作り直さないため）、その後で保存先を消す。
/// 受講者の意思による削除なので、書き込みの関所の起動番号の照合（[`flush_guarded`]）は通さない。
/// 保存先が開いていない・借用中は `done(Err(Unavailable))`（成功扱いにしない）。試験用の別の保存先には触れない。
/// 呼び出し元は `boot_and_mount` が結ぶ `set_history_deleter` の閉包だけで、対象は payload ではなくホストが読んだ IR の `pack.id` で決める。
/// `done` は同期でも非同期でも 1 回だけ呼ばれ、保存先を借りてはならない（配送のタイマーに積むだけにする）。
pub fn delete_pack(
    scope: &DeliveryScope,
    pack_id: &str,
    done: Box<dyn FnOnce(Result<usize, StorageError>)>,
) {
    PENDING.with(|p| p.discard_pack(scope, pack_id));
    let mut done = Some(done);
    STORAGE.with(|s| {
        if let Ok(slot) = s.try_borrow() {
            if let Some(storage) = slot.as_ref() {
                if let Some(d) = done.take() {
                    storage.remove_pack(scope, pack_id, d);
                }
            }
        }
    });
    if let Some(d) = done {
        d(Err(StorageError::Unavailable));
    }
}

/// 壊れた履歴の鍵 1 つを捨てる（SAVE-21・TASK-333 2/2）。保留からこの鍵だけを外してから、保存先の同じ鍵を消す。
///
/// 教材の全開始点を消す [`delete_pack`] とは別で、他の開始点・教材・配信先の履歴には触れない。受講者の明示の意思による操作なので、
/// 起動番号の照合（[`flush_guarded`]）は通さない。保存先が開いていない・借用中は `Err(Unavailable)`。
/// 呼び出し元は `boot_and_mount` が結ぶ `set_history_discarder` の閉包だけで、対象の鍵は結ぶ側が閉じ込める。
pub fn discard_key(key: &HistoryKey) -> Result<(), StorageError> {
    PENDING.with(|p| p.discard_key(key));
    STORAGE.with(|s| {
        let slot = s.try_borrow().map_err(|_| StorageError::Unavailable)?;
        // 関連関数の形で呼ぶ。glue の契約の検査は `.remove(` を DOM の構造操作として数えるため、保存先の削除と区別する。
        HistoryStore::remove(slot.as_ref().ok_or(StorageError::Unavailable)?, key)
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
    // 検出・書き込みの直前の保存世代。配送が遅れる間に即時の保存や履歴の差し替えが走れば、この世代は古くなる。
    let Some(epoch) = crate::glue::save_epoch() else {
        return;
    };
    // 秘密情報らしい文字列は保存先へ積まない（D-44 の失敗側）。世代と形だけをホストへ配り、警告を出す（SAFE-27）。
    // 解除は、次に書けた保存の `save-result`（成功）がホストで行う。
    let held = crate::glue::secret_gate(&v);
    if !held.is_empty() {
        #[cfg(feature = "test-hooks")]
        SECRET_HOLDS.with(|c| c.set(c.get().saturating_add(1)));
        crate::glue::schedule_action(crate::action::Scheduled::SecretHold(epoch, held), 0);
        return;
    }
    let result = stage_and_flush(&key, v);
    #[cfg(feature = "test-hooks")]
    AUTOSAVES.with(|c| c.set(c.get().saturating_add(1)));
    // 失敗・成功を保存通知へ反映する（SAVE-10）。配送は別のスタックのタイマーで行う。
    crate::glue::report_save_result(epoch, result);
}

/// 秘密情報らしい文字列を見つけて保存を止めた回数。試験の設定だけ。
#[cfg(feature = "test-hooks")]
pub(crate) fn secret_hold_count() -> u32 {
    SECRET_HOLDS.with(std::cell::Cell::get)
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
        // 秘密情報らしい文字列は、離れる直前にも書かない。警告は出せない（離れる最中のため）。
        if !crate::glue::secret_gate(&v).is_empty() {
            #[cfg(feature = "test-hooks")]
            SECRET_HOLDS.with(|c| c.set(c.get().saturating_add(1)));
            return;
        }
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
    stage_and_flush(key, value.to_owned())
}

/// 保留を保存先へ書き出す唯一の関所（SAVE-12）。呼び出し元は `stage_and_flush`（`save_now`・`save_deferred`・`pagehide` のリスナーが通る）である。
///
/// 書き込みの直前に起動番号を照合し（[`Guarded`]）、古いタブの値は保存先へ届けない。起動番号の取得前（ガードが無い）も書かない。
/// 保存先が開いていない・借用中は `None`。古いタブは `Ok` の結果（失敗 0 件）を返す。保存先へ書く経路を増やすときも、この関数を通す。
fn flush_guarded(p: &PendingSave) -> Option<FlushOutcome> {
    STORAGE.with(|s| {
        let slot = s.try_borrow().ok()?;
        let st = slot.as_ref()?;
        GUARD.with(|g| {
            let guard_slot = g.try_borrow().ok()?;
            let guard = guard_slot.as_ref()?;
            Some(p.flush(&Guarded { inner: st, guard }))
        })
    })
}

/// このタブが他のタブに追い越された（古いタブ）か。ホストの probe（`stale-notice`。SAVE-13）と、
/// 試験の設定の口 `debug_save_stale` が読む。真偽だけを返し、保存領域は読まない。
pub fn stale_tab() -> bool {
    GUARD.with(|g| {
        g.try_borrow()
            .ok()
            .and_then(|slot| slot.as_ref().map(BootGuard::is_stale))
            .unwrap_or(false)
    })
}

/// 起動番号の `tag`。同時に起動したタブを区別するための一意化で、秘密・認証には使わない。
fn boot_tag() -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "Math.random() は 0 以上 1 未満で、4294967296 を掛けた値は u32 に収まる"
    )]
    let tag = (js_sys::Math::random() * 4_294_967_296.0) as u32;
    tag
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
///
/// `start` は直接リンク `?start=` の値（`static/app.js` が読んで渡す。指定なしは `None`。TASK-109 1/2。DLV-9・D-68）で、
/// 開始点の照合だけに使い、鍵・経路・URL の組み立てには使わない。
pub fn mount_with_history(
    root_id: &str,
    user_agent: &str,
    start: Option<String>,
    ir: Option<Vec<u8>>,
    reduced_motion: bool,
) -> js_sys::Promise {
    let mut host = mount_host_for_page(start.as_deref(), ir.as_deref());
    // 動きの抑制の設定。表示だけの値で、履歴・鍵には使わない（A11Y-11）。
    host.set_reduced_motion(reduced_motion);
    // User-Agent は OS の推定とブラウザの案内の判定にだけ使い、保持・記録しない（鍵・保存先・履歴には使わない。D-48・JUDGE-28）。
    host.set_learner_os(estimate(user_agent));
    // 閉じた記録が読めない（保存領域なし）ときは、記録が無いものとして出す（TASK-544。SHELL-30）。
    let dismissed = matches!(
        LocalStore::new().get(crate::browser_notice::BROWSER_NOTICE_KEY),
        Ok(Some(v)) if v == crate::browser_notice::DISMISSED
    );
    host.set_browser_notice(
        crate::browser_notice::needs_notice(user_agent) && !dismissed,
        Some(Box::new(LocalStore::new())),
    );
    let Some(key) = host.history_key(delivery_scope()) else {
        // 復元の経路が無いため、`boot` の後という順序の制約は無い（試験の設定の代役の受け口。TASK-191 2/2）。
        #[cfg(feature = "test-hooks")]
        let host = crate::test_hooks::apply_receptor_probe(host);
        return match crate::glue::mount_host(root_id, host) {
            Ok(()) => js_sys::Promise::resolve(&JsValue::UNDEFINED),
            Err(e) => js_sys::Promise::reject(&e),
        };
    };
    // 起動番号は鍵が決まった直後（保存先を開く前。ページを開いた時点に最も近い）に 1 回だけ取得する（SAVE-12）。
    GUARD.with(|g| {
        if let Ok(mut slot) = g.try_borrow_mut() {
            *slot = Some(BootGuard::claim(LocalStore::new(), &key, boot_tag()));
        }
    });
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
///
/// 開始点の指定（`start`）は、試験の設定では選んだ教材の、配信する設定では同梱の IR（`ir`。`static/app.js` が固定の `./ir/main.json` を
/// 取得して渡す。印の meta が無い配信物では `None` で、挙動は変わらない）の `starts` と照合して開始点を選ぶ（TASK-112 2/2。`docs/testing.md` 5.239）。
fn mount_host_for_page(start: Option<&str>, ir: Option<&[u8]>) -> EngineHost {
    #[cfg(feature = "test-hooks")]
    {
        // 試験の設定は試験データの教材を埋め込んで使う。同梱の IR は無視する。
        let _ = ir;
        EngineHost::for_mount_with(crate::test_hooks::mount_fixture_from_url(), start)
    }
    #[cfg(not(feature = "test-hooks"))]
    {
        EngineHost::for_mount(ir, start)
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
    // 「この入力を残さない」の読み直しは、起動と同じ保存先を同じ鍵で読む（SAFE-28）。
    let reader_key = key.clone();
    host.set_saved_reader(Box::new(move || read(&reader_key)));
    // 読み直しの前に、保存先の実データを取り直す。完了は配送のタイマーでホストへ届ける（コールバックからホストへは触れない）。
    let refresh_key = key.clone();
    host.set_saved_refresh(Box::new(move |generation| {
        refetch(
            &refresh_key,
            Box::new(move |r| {
                crate::glue::schedule_action(
                    crate::action::Scheduled::SecretDiscardRead(r.is_ok(), generation),
                    0,
                );
            }),
        );
    }));
    // 壊れた履歴を捨てる口（SAVE-21）。消すのは今の鍵 1 つだけ。
    let discard_key_ = key.clone();
    host.set_history_discarder(Box::new(move || discard_key(&discard_key_)));
    // 保存済みの履歴の削除（SAVE-32）。対象はホストが読んだ IR の教材と、この配信の区間で決める。
    if let Some(pack_id) = host.pack_id() {
        let scope = delivery_scope();
        host.set_history_deleter(Box::new(move |generation| {
            delete_pack(
                &scope,
                &pack_id,
                Box::new(move |r| {
                    crate::glue::schedule_action(
                        crate::action::Scheduled::DeleteResult(generation, r),
                        0,
                    );
                }),
            );
        }));
    }
    host.set_save_failure_probe(Box::new(async_save_failure));
    host.set_stale_probe(Box::new(stale_tab));
    // 同意は保存しない（D-53）ため、履歴の復元（`boot`）の後に与える。試験の設定の代役の受け口だけ（TASK-191 2/2）。
    #[cfg(feature = "test-hooks")]
    let host = crate::test_hooks::apply_receptor_probe(host);
    crate::glue::mount_host(root_id, host)?;
    SAVE_KEY.with(|k| {
        if let Ok(mut slot) = k.try_borrow_mut() {
            *slot = Some(key.clone());
        }
    });
    install_save_on_leave(key, crate::glue::history_to_save)
}
