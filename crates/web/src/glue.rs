//! wasm の入口。JS（`static/app.js`）が呼ぶ `mount_app` を書き出す。
//!
//! 名前を `mount_app` にするのは、上流の `mount`（wasm-bindgen-exports）と重ねないため。
//! マウントした `Runtime` はイベント配線を生かすために保持し続ける必要があり、
//! `thread_local` に置く（wasm は単一スレッド）。

use std::cell::RefCell;

use fandhe_frontend_wasm_full::Runtime;
use wasm_bindgen::prelude::*;

use crate::hello::HelloView;

thread_local! {
    static RUNTIME: RefCell<Option<Runtime<HelloView>>> = const { RefCell::new(None) };
}

/// マウント済みかどうか。試験用の口が状態を読むために使う。
#[cfg(feature = "test-hooks")]
pub(crate) fn is_mounted() -> bool {
    RUNTIME.with(|r| r.try_borrow().map(|r| r.is_some()).unwrap_or(false))
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

/// `root_id` の要素に Hello をマウントする。2 回目以降は拒否する。
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
        *slot = Some(Runtime::mount(root_id, HelloView::new())?);
        Ok(())
    })
}
