//! wasm の入口。JS（`static/app.js`）が呼ぶ `mount_app` を書き出す。
//!
//! 名前を `mount_app` にするのは、上流の `mount`（wasm-bindgen-exports）と重ねないため。
//! マウントした `Runtime` はイベント配線を生かすために保持し続ける必要があり、
//! `thread_local` に置く（wasm は単一スレッド）。
//!
//! 進行（`tick`）と補完（`complete`）は、ここが `Runtime::dispatch_action` で Component に届ける（FEND-4。TASK-349）。
//! `dispatch_action` は更新の最中・配線のコールバックの中から呼ぶと `Reentrant` で捨てられ、保留もされない。
//! そのため配送は必ず `set_timeout` のコールバック（別のスタック）の中で行い、呼び出しの箇所を `deliver` の 1 つに保つ
//! （`tests/state_paths.rs` が固定する）。タイマーは取り消さない。停止・世代の替わりの後に届いた古い宛先の進行は、
//! core が `Ignored` 1 件として記録して状態を変えない（EVT-8・EVT-9）。

use std::cell::RefCell;

use fandhe_frontend_wasm_full::Runtime;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use fandhe_workshop_contract::key_decide::{decide, KeyInput};

use fandhe_workshop_core::addr::TickAddr;

use crate::action::{dispatch_effect, DispatchEffect, RuntimeAction, Scheduled};
use crate::host::EngineHost;
use crate::input_limit::clip_paste;
use crate::key_route::{effect_for, field_state, outside_root_esc, row_key_effect};
use crate::save_schedule::{Fire, SaveDebounce};
use crate::tick_interval::TICK_INTERVAL_MS;
use fandhe_frontend_wasm_full::events::{KeyModifiers, KeydownInput};

thread_local! {
    static RUNTIME: RefCell<Option<Runtime<EngineHost>>> = const { RefCell::new(None) };
    /// 予約済みの進行の宛先。同じ宛先のタイマーを二重に積まないための印（取り消しのためではない）。
    static SCHEDULED: RefCell<Option<TickAddr>> = const { RefCell::new(None) };
    /// 進行のタイマーの世代。履歴の差し替え（`on_tick_reset`）で進める。古い世代のタイマーは配送の時点で捨てる。
    /// 履歴の読み込みで差し替え前のタイマーが残っても、読み込み後のログへ `Ignored` を足さず、宛先が偶然一致しても進めない。
    static TICK_EPOCH: RefCell<u64> = const { RefCell::new(0) };
    /// 書き戻しの対象の root。送信で下書きを空にしたとき、タイマーのコールバックが `sync_draft` に渡す。
    static DRAFT_ROOT: RefCell<Option<web_sys::Element>> = const { RefCell::new(None) };
    /// 変換中（`compositionstart` から `compositionend` まで）かどうか。`sync_draft` が変換中の書き換えを避けるために読む。
    static COMPOSING: RefCell<bool> = const { RefCell::new(false) };
    /// ホストが頼んだ書き戻しが、まだ欄へ反映されていないかどうか（Issue #1510）。
    /// 送信の直後・書き戻しのタイマーの実行前に変換が始まると、書き戻しは変換の終わりまで保留になる。
    /// `compositionend` の捕捉の段で、上流が欄の値を状態へ届ける前に反映するために読む。
    /// 印の寿命は書き戻し 1 回分に限る。立てるのは `on_draft_sync_request` だけで、下ろすのは
    /// `clear_draft_sync_pending` だけ。残ると、後の通常の変換の終わりに変換前の下書きを欄へ書き、確定した文字を消す。
    static DRAFT_SYNC_PENDING: RefCell<bool> = const { RefCell::new(false) };
    /// 変更の後の遅延保存の畳み込み（TASK-318）。古いタイマーは `clearTimeout` でなく token で捨てる。
    static SAVE_DEBOUNCE: RefCell<SaveDebounce> = const { RefCell::new(SaveDebounce::new()) };
}

/// マウント済みかどうか。試験用の口が状態を読むために使う。
#[cfg(feature = "test-hooks")]
pub(crate) fn is_mounted() -> bool {
    RUNTIME.with(|r| r.try_borrow().map(|r| r.is_some()).unwrap_or(false))
}

/// App（`EngineHost`）を読み取り専用で借りて `f` を呼ぶ。glue が App に触れる唯一の口。
///
/// 状態を変える経路（`update`・`dispatch_action`）をここから作らせないため、閉じ込めは共有参照だけにする。
/// `tests/state_paths.rs` が、`component()` の呼び出しがこの関数の本体の 1 箇所だけであることを固定する。
/// `Runtime::component()` は `RefCell` の借用を取る。update は同期で完了するため、JS から呼ばれる時点で
/// 可変借用は残らない。念のため外側の借用は `try_borrow` で確かめ、未マウント・借用中は `None`（安全側）を返す。
fn with_app<T>(f: impl FnOnce(&EngineHost) -> T) -> Option<T> {
    RUNTIME.with(|cell| {
        let slot = cell.try_borrow().ok()?;
        let rt = slot.as_ref()?;
        let app = rt.component();
        Some(f(&app))
    })
}

/// action を Runtime の口（`dispatch_action`）で Component に届ける。glue が状態を動かす唯一の口。
///
/// 呼び出し元は `schedule_action` が予約した `set_timeout` のコールバックだけである。そこは別のスタックで、
/// Runtime の更新の最中ではないため、再入にならない。借用は共有参照の `try_borrow` で取り、未マウントは `None`。
/// `Reentrant` は `Dropped` として返す。再送もキューイングもしない（古い宛先の再送は EVT-8 で無視になるだけで、
/// 進行は次の更新が新しい宛先で予約し直す）。
fn deliver(action: &Scheduled) -> Option<DispatchEffect> {
    RUNTIME.with(|cell| {
        let slot = cell.try_borrow().ok()?;
        let rt = slot.as_ref()?;
        let (name, payload) = action.encode();
        Some(dispatch_effect(rt.dispatch_action(name, &payload)))
    })
}

/// action の配送を `delay_ms` 後に予約する。配送は必ず `set_timeout` のコールバックの中で行う（再入を避けるため）。
///
/// 戻りのタイマー ID は捨て、`clearTimeout` は使わない。進行は予約時の世代（`TICK_EPOCH`）を持ち、配送の時点で
/// 世代が進んでいれば捨てる（履歴の差し替え前のタイマー）。それ以外の古い宛先は届いても core が無視する。同じ世代で届いた進行の宛先は、
/// その宛先の予約の印を外す。
pub(crate) fn schedule_action(action: impl Into<Scheduled>, delay_ms: i32) {
    let action: Scheduled = action.into();
    let Some(window) = web_sys::window() else {
        return;
    };
    let epoch = TICK_EPOCH.with(|e| e.try_borrow().map(|e| *e).unwrap_or(0));
    let callback = Closure::once_into_js(move || {
        if let Scheduled::Runtime(RuntimeAction::Tick(addr)) = &action {
            if TICK_EPOCH.with(|e| e.try_borrow().map(|e| *e).unwrap_or(0)) != epoch {
                return;
            }
            SCHEDULED.with(|s| {
                if let Ok(mut slot) = s.try_borrow_mut() {
                    if slot.as_ref() == Some(addr) {
                        *slot = None;
                    }
                }
            });
        }
        let _ = deliver(&action);
    });
    let _ = window
        .set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), delay_ms);
}

/// 進行の間隔（ミリ秒）。定数は `i32` に収まる（`tick_interval` の const assert）。
fn tick_delay_ms() -> i32 {
    i32::try_from(TICK_INTERVAL_MS).unwrap_or(i32::MAX)
}

/// 履歴の差し替えで、これまでの進行のタイマーを全部無効にする（`EngineHost::set_tick_reset` の通知先）。
///
/// 世代を進め、予約の印も消す。差し替え後の宛先が差し替え前と同じでも、新しいタイマーを予約し直せる。
/// 停止・移動のような通常の更新では呼ばれない。古いタイマーは届いて core の `Ignored` になる（EVT-8）。
fn on_tick_reset() {
    TICK_EPOCH.with(|e| {
        if let Ok(mut e) = e.try_borrow_mut() {
            // u64 は実用上あふれない。あふれても世代が 0 に戻るだけで、配送の可否が変わるのはその一周に限る。
            *e = e.wrapping_add(1);
        }
    });
    SCHEDULED.with(|s| {
        if let Ok(mut slot) = s.try_borrow_mut() {
            *slot = None;
        }
    });
}

/// ホストが通知する次の進行の宛先を受け、タイマーを予約する（`EngineHost::set_tick_request` の通知先）。
///
/// `Component::update` の中から呼ばれるが、Runtime にも App にも触れず `set_timeout` を予約するだけなので、
/// 再入は起きない。`None` は予約の印を消す。同じ宛先がすでに予約済みなら何もしない（古い進行が `Ignored` を
/// 出して更新が走っても、同じ宛先のタイマーを積み増さない）。
fn on_tick_request(next: Option<TickAddr>) {
    let Some(addr) = next else {
        SCHEDULED.with(|s| {
            if let Ok(mut slot) = s.try_borrow_mut() {
                *slot = None;
            }
        });
        return;
    };
    let fresh = SCHEDULED.with(|s| match s.try_borrow_mut() {
        Ok(mut slot) if slot.as_ref() != Some(&addr) => {
            *slot = Some(addr.clone());
            true
        }
        _ => false,
    });
    if fresh {
        schedule_action(RuntimeAction::Tick(addr), tick_delay_ms());
    }
}

/// ホストが通知する「履歴が変わった」を受け、遅延保存のタイマーを予約する（`EngineHost::set_save_request` の通知先。TASK-318。SAVE-7）。
///
/// `Component::update` の中から呼ばれるため、Runtime にも App にも触れず予約するだけにする（再入を避ける）。
/// 連続する変更は最後の 1 回に畳み、最初の変更から最大の待ちを超えない（`save_schedule`）。
/// 書き出しは別のスタックのタイマーの中で `persist::save_deferred` が行い、書けない・保存しない値は書かない（fail-closed）。
fn on_save_request() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(plan) = SAVE_DEBOUNCE.with(|d| {
        d.try_borrow_mut()
            .ok()
            .map(|mut d| d.on_change(js_sys::Date::now()))
    }) else {
        return;
    };
    let callback = Closure::once_into_js(move || {
        let fire =
            SAVE_DEBOUNCE.with(|d| d.try_borrow_mut().ok().map(|mut d| d.on_fire(plan.token)));
        if fire == Some(Fire::Save) {
            crate::persist::save_deferred();
        }
    });
    let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        callback.unchecked_ref(),
        plan.delay_ms as i32,
    );
}

/// ホストが通知する「即時の保存が済んだ」を受け、予約済みの遅延保存を無効にする（`EngineHost::set_save_cancel` の通知先）。
///
/// 畳み込みの状態だけを触る。古いタイマーは token の不一致で何もせず、同じ履歴を二重に書かない。
fn on_save_cancel() {
    SAVE_DEBOUNCE.with(|d| {
        if let Ok(mut d) = d.try_borrow_mut() {
            d.cancel();
        }
    });
}

/// 遅延保存の結果をホストへ届ける（`persist::save_deferred` が呼ぶ。SAVE-10）。
///
/// 呼ぶのは遅延保存のタイマーの中（別のスタック）だけだが、配送は他の action と同じく `schedule_action` の `set_timeout(0)` に回す。
///
/// `epoch` は書き込んだ時点の保存世代。配送までに即時の保存が走っていれば、ホストが古い結果として捨てる。
pub(crate) fn report_save_result(
    epoch: u64,
    result: Result<(), fandhe_workshop_contract::storage::StorageError>,
) {
    schedule_action(Scheduled::SaveResult(epoch, result), 0);
}

/// 今の保存世代（即時の保存のたびに進む）。未マウント・借用中は `None`。
pub(crate) fn save_epoch() -> Option<u64> {
    with_app(EngineHost::save_epoch)
}

/// 遅延保存を待っている変更があるか。試験用の口が読む。
#[cfg(feature = "test-hooks")]
pub(crate) fn save_scheduled() -> bool {
    SAVE_DEBOUNCE.with(|d| d.try_borrow().is_ok_and(|d| d.is_scheduled()))
}

/// ホストが通知する下書きの書き戻しの要求を受け、`set_timeout(0)` で `sync_draft` を予約する
/// （`EngineHost::set_draft_sync_request` の通知先。TASK-438）。
///
/// `Component::update` の中から呼ばれるため、Runtime にも App にも触れず予約するだけにする（再入を避ける）。
/// 書き戻しを予約に回すのは、送信の click が上流の更新の中で走り、その最中に App を借りられないため。
/// 送信の直後・タイマーの実行前に変換が始まった場合、`sync_draft` が変換中を見て書き換えない。
/// 保留した書き戻しは、`compositionend` の捕捉の段（上流が欄の値を状態へ届ける前）で反映する（`wire_draft_sync`）。
/// そのため、タイマーを予約する時点で保留の印を立て、変換中でない `sync_draft` が（書けたかどうかに関わらず）印を下ろす。
/// 書き戻しの対象の root が無いときも印を下ろす（書く先が無い印を残すと、後の変換の確定文字を消すため）。
fn on_draft_sync_request() {
    let Some(window) = web_sys::window() else {
        return;
    };
    mark_draft_sync_pending();
    let callback = Closure::once_into_js(move || {
        let root = DRAFT_ROOT.with(|r| r.try_borrow().ok().and_then(|slot| slot.clone()));
        match root {
            Some(root) => sync_draft(&root),
            None => clear_draft_sync_pending(),
        }
    });
    let _ =
        window.set_timeout_with_callback_and_timeout_and_arguments_0(callback.unchecked_ref(), 0);
}

/// 予約済みの進行の宛先。試験用の口が読む。
#[cfg(feature = "test-hooks")]
pub(crate) fn scheduled_tick() -> Option<TickAddr> {
    SCHEDULED.with(|s| s.try_borrow().ok().and_then(|slot| slot.clone()))
}

/// 補完の action を、実際の配送の口（`schedule_action`）で届ける。試験用の口が、画面の Tab と同じ経路を確かめるために使う。
#[cfg(feature = "test-hooks")]
pub(crate) fn request_complete() {
    schedule_action(RuntimeAction::Complete, 0);
}

/// 補完の action の受信回数。未マウント・借用中は `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn complete_count() -> Option<u32> {
    with_app(|app| app.complete_count())
}

/// keydown の probe の受信回数。未マウント・借用中は `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn keydown_probe_count() -> Option<u32> {
    with_app(|app| app.keydown_probe())
}

/// root の中の Esc の受信回数。未マウント・借用中は `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn esc_count() -> Option<u32> {
    with_app(|app| app.esc_count())
}

/// 3 つの表示の照合用の状態（JSON 文字列）。未マウント・借用中は `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn engine_snapshot() -> Option<String> {
    with_app(|app| app.engine_snapshot_json())
}

/// 直近に開いた履歴の、開いた直後の状態の JSON。開いていない・未マウント・借用中は `None`。
#[cfg(feature = "test-hooks")]
pub(crate) fn last_import_state() -> Option<String> {
    with_app(|app| app.last_import_state_json().map(str::to_owned)).flatten()
}

/// 入力欄の値の取得。未マウント・借用中は `None`。試験用の口が欄の状態を読むために使う。
///
/// 戻り値は（コードポイント数、超過の印、本文）。
#[cfg(feature = "test-hooks")]
pub(crate) fn draft_snapshot() -> Option<(usize, bool, String)> {
    with_app(|app| {
        (
            app.draft().chars().count(),
            app.over_limit(),
            app.draft().to_owned(),
        )
    })
}

/// 下書きの欄・送信ボタンのセレクタ（`hello.rs` の `data-testid` と対応する）。
const DRAFT_SELECTOR: &str = "[data-testid=\"draft-input\"]";
const SUBMIT_SELECTOR: &str = "[data-testid=\"submit-input\"]";

/// root 配下の要素を引く。要素の取得の呼び出し箇所をここ 1 つに保つ（`glue_contract` が箇所数を固定する）。
/// 見つからない場合・不正なセレクタは `None`（安全側）。
fn find_in_root(root: &web_sys::Element, selector: &str) -> Option<web_sys::Element> {
    root.query_selector(selector).ok().flatten()
}

/// 状態の下書きを `<textarea>` へ書き戻す。値が同じなら書かない（書くとキャレットが末尾へ動くため）。
///
/// 上流の `wasm-client` の `value` 束縛は `HtmlInputElement` しか同期しない（PoC-2 の F-1）。
/// 状態側で 20,000 コードポイントに切った後の値を欄に反映するには、この書き戻しが要る。
/// 上流が `<textarea>` の同期に対応したら外す。要素が無い・借用中は何もしない（安全側）。
///
/// 変換中（`COMPOSING`）は、どの経路から呼ばれても書かない（変換中の書き換えは入力を壊す）。
/// 保留した分は `compositionend` の捕捉の段が処理する。そのため変換中に限って保留の印を残す。
///
/// 変換中でなければ、書けたかどうかに関わらず保留の印を下ろす（印を書き戻し 1 回分より長く残さない）。
/// 欄が無い・`<textarea>` でないときは、書く先が無い。App を借りられない（`with_app` が `None`）ときは
/// 書き戻しを 1 回落とす。印を残しても、この後に再試行する経路は無く、次に印を読むのは後の通常の変換の
/// `compositionend` である。そこで変換前の下書きを書くと確定した文字を消すため、下ろす側を選ぶ。落とした場合は
/// 送信済みの本文が欄に残るが、次の入力で上流が欄の値を状態へ届け、欄と状態は揃う（文字は消えない）。
/// `with_app` はタイマー・リスナーの中では借用中にならない（`mount_app` の外）ため、実際には通らない安全側の分岐である。
fn sync_draft(root: &web_sys::Element) {
    if is_composing() {
        return;
    }
    if let Some(field) = find_in_root(root, DRAFT_SELECTOR)
        .and_then(|el| el.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
    {
        let _ = with_app(|app| {
            let draft = app.draft();
            if field.value() != draft {
                field.set_value(draft);
            }
        });
    }
    clear_draft_sync_pending();
}

/// 書き戻しの保留があるかどうか。借用中は `false`（変換の終わりに書かない側＝確定した入力を消さない側）。
fn draft_sync_pending() -> bool {
    DRAFT_SYNC_PENDING.with(|p| p.try_borrow().map(|b| *b).unwrap_or(false))
}

/// 書き戻しの保留の印を立てる。立てるのは `on_draft_sync_request` だけ。借用中は何もしない（立てない側＝書かない側）。
fn mark_draft_sync_pending() {
    DRAFT_SYNC_PENDING.with(|p| {
        if let Ok(mut b) = p.try_borrow_mut() {
            *b = true;
        }
    });
}

/// 書き戻しの保留の印を下ろす。印を下ろす唯一の口（`sync_draft` の変換中でない出口と、root が無いタイマー）。
/// 印の借用は各関数の中で閉じるため、実際には借用中にならない（なっても panic させず何もしない）。
fn clear_draft_sync_pending() {
    DRAFT_SYNC_PENDING.with(|p| {
        if let Ok(mut b) = p.try_borrow_mut() {
            *b = false;
        }
    });
}

/// 変換中かどうか。借用中は `true`（書かない側＝安全側）。
fn is_composing() -> bool {
    COMPOSING.with(|c| c.try_borrow().map(|b| *b).unwrap_or(true))
}

/// 変換中の印を更新する。借用中は何もしない。
fn mark_composing(on: bool) {
    COMPOSING.with(|c| {
        if let Ok(mut b) = c.try_borrow_mut() {
            *b = on;
        }
    });
}

/// 書き戻しのリスナーを root に登録する。`mount_app` が成功した直後に 1 回だけ呼ぶ。
///
/// 上流の `input` の配線（`Runtime::mount` が先に root へ登録済み）の後に走るため、状態は更新済みである。
/// 変換中（`InputEvent.isComposing`）は書き戻さず、`compositionend` の次のタスクで同期する（変換中の書き換えは入力を壊す）。
/// `compositionend` だけは捕捉の段（capture）で登録する。上流も root の `compositionend`（泡立ちの段）で欄の値を
/// 確定値として状態へ届けるため、保留した書き戻し（送信で空にした下書き）をそれより先に欄へ反映しないと、
/// 送信済みの本文が欄から状態へ戻り、次の Enter で同じ本文を再び送ってしまう（Issue #1510）。
/// `Closure::forget` は mount が 1 回限りのため、リークは定数個に収まる。
fn wire_draft_sync(root: &web_sys::Element) -> Result<(), JsValue> {
    let target = root.clone();
    let on_input = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        if ev
            .dyn_ref::<web_sys::InputEvent>()
            .is_some_and(|i| i.is_composing())
        {
            return;
        }
        sync_draft(&target);
    });
    let on_start = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        mark_composing(true);
    });
    root.add_event_listener_with_callback("compositionstart", on_start.as_ref().unchecked_ref())?;
    on_start.forget();
    root.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())?;
    on_input.forget();
    // 保留した書き戻しがあるときだけ、捕捉の段で同期に反映する。保留が無いときは書かない。
    // 変換の印を先に下ろすため、ここの `sync_draft` は書けたかどうかに関わらず保留の印を下ろす（印は変換 1 回分で尽きる）。
    // 変換の間、上流は `input` を延期するため、状態は変換前の下書きのままであり、ここで書くと確定した文字を消す。
    // Safari は確定の `input` より前に `compositionend` を発火する。その時点の状態は古いため、
    // 保留の無い同期はタスクを 1 つ送り、続く `input`（と上流の確定値の配送）が状態を更新した後に行う
    // （Chrome のように `compositionend` が後でも、状態は最新なので害はない）。
    let target = root.clone();
    let on_end = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        mark_composing(false);
        if draft_sync_pending() {
            sync_draft(&target);
        }
        let Some(window) = web_sys::window() else {
            return;
        };
        let target = target.clone();
        let deferred = Closure::once_into_js(move || sync_draft(&target));
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(deferred.unchecked_ref(), 0);
    });
    root.add_event_listener_with_callback_and_bool(
        "compositionend",
        on_end.as_ref().unchecked_ref(),
        true,
    )?;
    on_end.forget();
    Ok(())
}

/// 上限を超える貼り付けを、貼った分だけで切る（TASK-283。JUDGE-24・D-60）。`mount_host` が 1 回だけ呼ぶ。
///
/// `beforeinput` の `insertFromPaste` で、貼り付けの前に挿入を止める。介入するのは、下書きの欄で、変換中でなく、
/// 欄と状態が揃っていて、途中への貼り付けが上限を超えるときだけである。それ以外は何もせず、既定の挿入と
/// 状態側の末尾で切る安全網（`clamp_draft`）に任せる。計画は純粋関数 `clip_paste` が持つ。
/// 欄の今の値は読まず、状態の下書きから取る。借用は計画を求めた時点で解く（上流の同期の `update` を妨げないため）。
/// 欄へ書いた後は泡立つ `input` を合成し、上流の通常の入力の経路で `SetDraft` を届ける。glue は状態を直接変えない
/// （`wire_enter_submit` が `click()` で上流へ届けるのと同じ扱い）。キャレットは書き戻しの後に置く。
/// 欄の値をプログラムで書くため、Ctrl+Z では貼り付けの前に戻らない（`docs/testing.md` 5.189）。
fn wire_paste_clip(root: &web_sys::Element) -> Result<(), JsValue> {
    let on_before = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(iev) = ev.dyn_ref::<web_sys::InputEvent>() else {
            return;
        };
        if iev.is_composing() || iev.input_type() != "insertFromPaste" || draft_sync_pending() {
            return;
        }
        let Some(field) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlTextAreaElement>().ok())
        else {
            return;
        };
        let Some(pasted) = iev.data() else {
            return;
        };
        let (Ok(Some(start)), Ok(Some(end))) = (field.selection_start(), field.selection_end())
        else {
            return;
        };
        let Some(Some(plan)) = with_app(|app| clip_paste(app.draft(), start, end, &pasted)) else {
            return;
        };
        ev.prevent_default();
        field.set_value(&plan.value);
        let init = web_sys::EventInit::new();
        init.set_bubbles(true);
        if let Ok(input) = web_sys::Event::new_with_event_init_dict("input", &init) {
            let _ = field.dispatch_event(&input);
        }
        let _ = field.set_selection_range(plan.caret_utf16, plan.caret_utf16);
    });
    root.add_event_listener_with_callback("beforeinput", on_before.as_ref().unchecked_ref())?;
    on_before.forget();
    Ok(())
}

/// 入力欄の Enter を、送信ボタンへの click に結ぶ（TASK-275。JUDGE-12・FEND-14）。`mount_app` が 1 回だけ呼ぶ。
///
/// root に登録するのは、再描画で欄が作り直されてもリスナーが外れないため。対象は下書きの `<textarea>` だけで（左の行は `wire_row_keys` が受け持つ）、
/// 他の要素（probe のボタン等）の Enter には反応しない。判定は `contract` の `decide`、効果への変換は
/// `key_route` が持つ。`with_app` の借用は効果を求めた時点で解く。click の中で上流の Runtime が同期に
/// `update` を呼ぶため、借用を持ったまま click すると更新が黙って失敗する。
/// Tab の補完の `preventDefault`（JUDGE-19・D-42）も、同じリスナーが効果に従って結ぶ。
/// 入力欄の Enter・Tab・Space の状態に依存する `preventDefault` は、属性化も上流の口も採らず glue が受け持つ
/// （D-42。FEND-7。TASK-352 1/3）。Space は判定が `Nothing` で `preventDefault` せず、textarea の既定動作の空白の挿入に任せる。
/// `Closure::forget` は mount が 1 回限りのため、リークは定数個に収まる。
fn wire_enter_submit(root: &web_sys::Element) -> Result<(), JsValue> {
    let target = root.clone();
    let on_keydown = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        let from_field = ev
            .target()
            .is_some_and(|t| t.dyn_ref::<web_sys::HtmlTextAreaElement>().is_some());
        if !from_field {
            return;
        }
        // keyCode は IME の変換中の判定（JUDGE-14。229）に使うため読む。
        let key_code = kev.key_code();
        let input = KeyInput {
            key: kev.key(),
            key_code,
            is_composing: kev.is_composing(),
            shift: kev.shift_key(),
            ctrl: kev.ctrl_key(),
            alt: kev.alt_key(),
            meta: kev.meta_key(),
        };
        let Some((has_text, candidates_open)) =
            with_app(|app| (!app.draft().is_empty(), app.candidates_open()))
        else {
            return;
        };
        let effect = effect_for(decide(&input, &field_state(has_text, candidates_open)));
        if effect.prevent_default {
            ev.prevent_default();
        }
        // 補完はリスナーの中で直接届けず、タイマーの予約を通す（リスナーの中では再入で捨てられるため）。
        // 一覧が開いていて文字があるときだけ判定が補完になり、ここへ届く（JUDGE-19）。挿入はホストが行う。
        if effect.deliver_complete {
            schedule_action(RuntimeAction::Complete, 0);
        }
        // 押しっぱなしの自動リピートは送信しない（下書きが残るため、同じ行が繰り返し足されるのを防ぐ）。
        // 改行の挿入の抑止（prevent_default）は通常の Enter と同じにする。
        if effect.click_submit && !kev.repeat() {
            if let Some(button) = find_in_root(&target, SUBMIT_SELECTOR)
                .and_then(|el| el.dyn_into::<web_sys::HtmlElement>().ok())
            {
                button.click();
            }
        }
    });
    root.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())?;
    on_keydown.forget();
    Ok(())
}

/// 左の行（指せる行。`li`）の Enter・Space を、その行への `click()` に結ぶ（TASK-358 1/2。A11Y-18・FEND-13・D-42）。
///
/// 呼び出し元は [`mount_host`] と、試験用の代役の部品のマウント（`test_hooks::debug_view_fixture_mount`）である。
/// 指せる行は標準の操作部品ではないため、ブラウザは Enter・Space で何もしない。ここで `click()` に結べば、
/// マウスの click と同じく上流の委譲（`closest("[data-action]")`）から `decode_action` を通って下流に届き、
/// 宛先を受け付けるかの判断は下流（`PointTarget::decode`・core・`is_pointable_now`）が持つ。glue は payload を読まない。
///
/// `pointable` は属性を読まず、target が `li` かどうかで決める（glue は DOM の値を読めない）。`tabindex` を持つ `li` は
/// 指せる行だけで、持たない `li` はフォーカスを受けず keydown の target にならない。内側のボタンや入力欄の上では
/// target がその要素になり、ここは何もしないため二重の click は起きない。
/// `with_app` は呼ばない。借用を持ったまま `click()` すると、上流の同期の `update` が黙って失敗するためである。
/// 判定は `key_route::row_key_effect` が持つ。`Closure::forget` は mount が 1 回限りのため、リークは定数個に収まる。
///
/// # Errors
///
/// リスナーの登録に失敗した場合に `Err` を返す。
pub(crate) fn wire_row_keys(root: &web_sys::Element) -> Result<(), JsValue> {
    let on_keydown = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        let Some(row) = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::HtmlLiElement>().ok())
        else {
            return;
        };
        let input = KeyInput {
            key: kev.key(),
            key_code: kev.key_code(),
            is_composing: kev.is_composing(),
            shift: kev.shift_key(),
            ctrl: kev.ctrl_key(),
            alt: kev.alt_key(),
            meta: kev.meta_key(),
        };
        let effect = row_key_effect(&input, kev.repeat(), true);
        if effect.prevent_default {
            ev.prevent_default();
        }
        if effect.click_row {
            row.click();
        }
    });
    root.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref())?;
    on_keydown.forget();
    Ok(())
}

/// root の外の Esc（フォーカスなし・root の外の部品）を、`document` の keydown で受けて Component に届ける（TASK-353 1/2。FEND-8・D-42）。
///
/// 呼び出し元は [`mount_host`] である。root の中の Esc は上流の属性（`action::ESC_KEYDOWN_ATTRS`。`main` と `workshop-root` に付く）が届けるため、
/// target が root の中なら何もしない（二重に届けない。FEND-6）。target が無い・Node でないときは root の外として届ける。
/// 判定は `key_route::outside_root_esc`（上流と同じ照合）、停止か取り消しかの判断は届いた後の `esc_route` が持つ。
/// 届けるのは `schedule_action` 経由に限る（リスナーの中で直接 `dispatch_action` すると `Reentrant` で捨てられる）。
/// `preventDefault` は呼ばない（root の中の属性も付けていない）。`root` の clone を持ち込むのは、要素の取得の箇所を増やさないため。
/// `Closure::forget` は mount が 1 回限りのため、リークは定数個に収まる。
///
/// # Errors
///
/// `window`・`document` が無い場合、リスナーの登録に失敗した場合に `Err` を返す。
fn wire_outside_esc(root: &web_sys::Element) -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let root = root.clone();
    let on_keydown = Closure::<dyn FnMut(web_sys::Event)>::new(move |ev: web_sys::Event| {
        let Some(kev) = ev.dyn_ref::<web_sys::KeyboardEvent>() else {
            return;
        };
        let in_root = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Node>().ok())
            .is_some_and(|node| root.contains(Some(&node)));
        let key = kev.key();
        let input = KeydownInput {
            key: &key,
            modifiers: KeyModifiers {
                ctrl: kev.ctrl_key(),
                alt: kev.alt_key(),
                shift: kev.shift_key(),
                meta: kev.meta_key(),
            },
            is_composing: kev.is_composing(),
            key_code: kev.key_code(),
            repeat: kev.repeat(),
        };
        if let Some(action) = outside_root_esc(&input, in_root) {
            schedule_action(action, 0);
        }
    });
    // 捕捉の段で受ける。泡立ちの段で受けると、root の中の Esc を先に処理した上流の配送が画面を作り直し、
    // 押された要素（左の行など）が DOM から外れて `contains` が偽になり、root の外として 2 回目が届く（TASK-353 2/2）。
    document.add_event_listener_with_callback_and_bool(
        "keydown",
        on_keydown.as_ref().unchecked_ref(),
        true,
    )?;
    on_keydown.forget();
    Ok(())
}

/// 画面を root 配下ごと再構築する。再描画の時間の測定（tools/measure/redraw.sh）が試験用の口経由で呼ぶ。
///
/// 戻り値は、再構築を実行したかどうか。未マウント、または他の借用中は何もせず `false` を返す
/// （`Runtime::rerender` は借用に失敗すると黙って何もしないため、呼ぶ前に自分で借用を確かめる）。
#[cfg(feature = "test-hooks")]
pub(crate) fn rerender() -> bool {
    RUNTIME.with(|cell| match cell.try_borrow() {
        Ok(slot) => match slot.as_ref() {
            Some(rt) => {
                rt.rerender();
                true
            }
            None => false,
        },
        Err(_) => false,
    })
}

/// `root_id` の要素にホストをマウントする。2 回目以降は拒否する。
///
/// 呼び出し元は [`mount_app`]（保存先の準備を待った後）と、保存済みの履歴を持たない配信する設定の同期の入口である。
/// 起動の分岐（`EngineHost::boot`）は呼び出し元が済ませてから渡す。進行・下書き・補完の通知先はここで結ぶ。
///
/// # Errors
///
/// 要素が無い場合、イベント配線に失敗した場合、すでにマウント済みの場合に `Err` を返す。
pub(crate) fn mount_host(root_id: &str, mut host: EngineHost) -> Result<(), JsValue> {
    RUNTIME.with(|cell| {
        let mut slot = cell
            .try_borrow_mut()
            .map_err(|_| JsValue::from_str("runtime busy"))?;
        if slot.is_some() {
            return Err(JsValue::from_str("already mounted"));
        }
        host.set_tick_request(Box::new(on_tick_request));
        host.set_tick_reset(Box::new(on_tick_reset));
        host.set_draft_sync_request(Box::new(on_draft_sync_request));
        host.set_save_request(Box::new(on_save_request));
        host.set_save_cancel(Box::new(on_save_cancel));
        let runtime = Runtime::mount(root_id, host)?;
        let root = runtime.root();
        DRAFT_ROOT.with(|r| {
            if let Ok(mut slot) = r.try_borrow_mut() {
                *slot = Some(root.clone());
            }
        });
        wire_draft_sync(root)?;
        wire_paste_clip(root)?;
        wire_enter_submit(root)?;
        wire_row_keys(root)?;
        wire_outside_esc(root)?;
        *slot = Some(runtime);
        Ok(())
    })
}

/// 離れる直前に保存する値。保存しないとき（未操作・開けなかった履歴がある・未マウント）は `None`（TASK-319）。
///
/// `persist` の `pagehide` のリスナーが呼ぶ。読み取りだけで、状態は変えない。
pub(crate) fn history_to_save() -> Option<String> {
    with_app(EngineHost::history_to_save).flatten()
}

/// 保存の直前の判定（確認済みの検出を除いた形。SAFE-27・SAFE-28）。`persist` の遅延保存と離れる直前の保存が呼ぶ。
///
/// 読み取りだけで、状態は変えない。ホストを借りられないとき（未マウント・借用中）は、確認済みを除かない厳しい側
/// （`secret_hold`）に倒す（fail-closed）。
pub(crate) fn secret_gate(text: &str) -> Vec<fandhe_workshop_core::secret_scan::SecretShape> {
    with_app(|h| h.secret_gate(text)).unwrap_or_else(|| crate::secret_notice::secret_hold(text))
}

/// `root_id` の要素にホストをマウントする Promise を返す。2 回目以降は拒否する。
///
/// 保存済みの履歴の読み込みは非同期のため、保存先が準備できてから起動の分岐を行いマウントする（`persist`、TASK-319）。
/// `static/app.js` が `await` し、`data-mounted` はその完了後に立てる。要素が無い場合、イベント配線に失敗した場合、
/// すでにマウント済みの場合は拒否する。
#[wasm_bindgen]
pub fn mount_app(root_id: &str) -> js_sys::Promise {
    crate::persist::mount_with_history(root_id)
}
