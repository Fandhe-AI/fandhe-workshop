//! wasm の入口。JS（`static/app.js`）が呼ぶ `mount_app` を書き出す。
//!
//! 名前を `mount_app` にするのは、上流の `mount`（wasm-bindgen-exports）と重ねないため。
//! マウントした `Runtime` はイベント配線を生かすために保持し続ける必要があり、
//! `thread_local` に置く（wasm は単一スレッド）。

use std::cell::RefCell;

use fandhe_frontend_wasm_full::Runtime;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use fandhe_workshop_contract::key_decide::{decide, KeyInput};

use crate::host::EngineHost;
use crate::key_route::{effect_for, field_state};

thread_local! {
    static RUNTIME: RefCell<Option<Runtime<EngineHost>>> = const { RefCell::new(None) };
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
fn sync_draft(root: &web_sys::Element) {
    let Some(el) = find_in_root(root, DRAFT_SELECTOR) else {
        return;
    };
    let Ok(field) = el.dyn_into::<web_sys::HtmlTextAreaElement>() else {
        return;
    };
    with_app(|app| {
        let draft = app.draft();
        if field.value() != draft {
            field.set_value(draft);
        }
    });
}

/// 書き戻しのリスナーを root に登録する。`mount_app` が成功した直後に 1 回だけ呼ぶ。
///
/// 上流の `input` の配線（`Runtime::mount` が先に root へ登録済み）の後に走るため、状態は更新済みである。
/// 変換中（`InputEvent.isComposing`）は書き戻さず、`compositionend` の次のタスクで同期する（変換中の書き換えは入力を壊す）。
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
    root.add_event_listener_with_callback("input", on_input.as_ref().unchecked_ref())?;
    on_input.forget();
    // Safari は確定の `input` より前に `compositionend` を発火する。その時点の状態は古いため、
    // 同期で書き戻すと変換結果を消す。タスクを 1 つ送り、続く `input` が状態を更新した後に同期する
    // （Chrome のように `compositionend` が後でも、状態は最新なので害はない）。
    let target = root.clone();
    let on_end = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| {
        let Some(window) = web_sys::window() else {
            return;
        };
        let target = target.clone();
        let deferred = Closure::once_into_js(move || sync_draft(&target));
        let _ = window
            .set_timeout_with_callback_and_timeout_and_arguments_0(deferred.unchecked_ref(), 0);
    });
    root.add_event_listener_with_callback("compositionend", on_end.as_ref().unchecked_ref())?;
    on_end.forget();
    Ok(())
}

/// 入力欄の Enter を、送信ボタンへの click に結ぶ（TASK-275。JUDGE-12・FEND-14）。`mount_app` が 1 回だけ呼ぶ。
///
/// root に登録するのは、再描画で欄が作り直されてもリスナーが外れないため。対象は下書きの `<textarea>` だけで、
/// 他の要素（probe のボタン等）の Enter には反応しない。判定は `contract` の `decide`、効果への変換は
/// `key_route` が持つ。`with_app` の借用は効果を求めた時点で解く。click の中で上流の Runtime が同期に
/// `update` を呼ぶため、借用を持ったまま click すると更新が黙って失敗する。
/// Tab の補完の `preventDefault`（JUDGE-19・D-42）も、同じリスナーが効果に従って結ぶ。
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
        let Some(has_text) = with_app(|app| !app.draft().is_empty()) else {
            return;
        };
        let effect = effect_for(decide(&input, &field_state(has_text)));
        if effect.prevent_default {
            ev.prevent_default();
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
/// # Errors
///
/// 要素が無い場合、イベント配線に失敗した場合、すでにマウント済みの場合に `Err` を返す。
#[wasm_bindgen]
pub fn mount_app(root_id: &str) -> Result<(), JsValue> {
    RUNTIME.with(|cell| {
        let mut slot = cell
            .try_borrow_mut()
            .map_err(|_| JsValue::from_str("runtime busy"))?;
        if slot.is_some() {
            return Err(JsValue::from_str("already mounted"));
        }
        let runtime = Runtime::mount(root_id, EngineHost::new())?;
        let root = runtime.root();
        wire_draft_sync(root)?;
        wire_enter_submit(root)?;
        *slot = Some(runtime);
        Ok(())
    })
}
