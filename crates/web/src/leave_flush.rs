//! ページを離れる直前の書き出しのリスナー（TASK-317 2/2。SAVE-9）。
//!
//! `window` の `pagehide` で、呼び出し元が渡した書き出しを同期に呼ぶ。localStorage への書き込みは同期のため、
//! ページが破棄される前に終わる（IndexedDB は非同期で確定を保証しない。限界は `save_pending` に書く）。
//! 呼び出し元は、保存先と保留（`save_pending::PendingSave`）を持つ層（試験用の口 `test_hooks`、後続の TASK-318・TASK-319）である。
//!
//! イベントの属性（`persisted` 等）は読まない。再読み込み・タブを閉じる・bfcache のどれでも書き出す。
//! `Closure::forget` は登録が 1 セッションに 1 回のため、リークは定数個に収まる（`glue` と同じ）。

use std::cell::Cell;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

thread_local! {
    /// 登録済みの印。二重に登録しない。
    static INSTALLED: Cell<bool> = const { Cell::new(false) };
}

/// `pagehide` のリスナーを 1 つだけ付ける。2 回目以降は何もしない（先の `f` が残る）。
pub fn install_pagehide_flush(mut f: impl FnMut() + 'static) -> Result<(), JsValue> {
    if INSTALLED.with(|i| i.get()) {
        return Ok(());
    }
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let on_hide = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| f());
    window.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref())?;
    on_hide.forget();
    INSTALLED.with(|i| i.set(true));
    Ok(())
}
