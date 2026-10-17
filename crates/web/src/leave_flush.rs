//! ページを離れる直前の書き出しのリスナー（TASK-317 2/2。SAVE-9）。
//!
//! `window` の `pagehide` で、呼び出し元が渡した書き出しを同期に呼ぶ。localStorage への書き込みは同期のため、
//! ページが破棄される前に終わる（IndexedDB は非同期で確定を保証しない。限界は `save_pending` に書く）。
//! 呼び出し元は、保存先と保留（`save_pending::PendingSave`）を持つ層（試験用の口 `test_hooks`、受講中の保存の結線 `persist`。TASK-319）である。
//!
//! イベントの属性（`persisted` 等）は読まない。再読み込み・タブを閉じる・bfcache のどれでも書き出す。
//! 登録は名前ごとに 1 回だけである。試験用の口と受講中の結線は別の名前で登録し、片方の登録がもう片方を塞がない。
//! `Closure::forget` は登録が名前ごとに 1 セッションに 1 回のため、リークは定数個に収まる（`glue` と同じ）。

use std::cell::RefCell;
use wasm_bindgen::{closure::Closure, JsCast, JsValue};

/// 名前を省略した登録の名前（試験用の口が使う）。
const DEFAULT_NAME: &str = "default";

thread_local! {
    /// 登録済みの名前。同じ名前を二重に登録しない。
    static INSTALLED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

/// `pagehide` のリスナーを 1 つだけ付ける。2 回目以降は何もしない（先の `f` が残る）。
///
/// # Errors
///
/// `window` が無い、またはリスナーを付けられなかったとき。
pub fn install_pagehide_flush(f: impl FnMut() + 'static) -> Result<(), JsValue> {
    install_pagehide_flush_as(DEFAULT_NAME, f)
}

/// 名前つきで `pagehide` のリスナーを付ける。同じ名前の 2 回目以降は何もしない（先の `f` が残る）。
///
/// # Errors
///
/// `window` が無い、またはリスナーを付けられなかったとき。
pub fn install_pagehide_flush_as(
    name: &'static str,
    mut f: impl FnMut() + 'static,
) -> Result<(), JsValue> {
    if INSTALLED.with(|i| i.try_borrow().map(|v| v.contains(&name)).unwrap_or(true)) {
        return Ok(());
    }
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let on_hide = Closure::<dyn FnMut(web_sys::Event)>::new(move |_ev: web_sys::Event| f());
    window.add_event_listener_with_callback("pagehide", on_hide.as_ref().unchecked_ref())?;
    on_hide.forget();
    INSTALLED.with(|i| {
        if let Ok(mut v) = i.try_borrow_mut() {
            v.push(name);
        }
    });
    Ok(())
}
