//! 書き出しのバイト列を、ダウンロード用の `blob:` URL にする（TASK-330 1/2、SAVE-16）。wasm のみ。
//!
//! 呼び出し元は、画面の書き出しの操作（TASK-531）と、試験用の口（`test_hooks`）である。
//! `Blob` を作って `URL.createObjectURL` を呼ぶだけで、DOM への書き込みも通信もしない。`data:` URL は履歴の中身を
//! URL に載せるため採らない（SAVE-16。URL に状態を載せない）。`blob:` の URL は文書の中でだけ有効で、
//! 履歴の中身は含まない。
//!
//! 引き継ぎ（TASK-531）。画面は、受講者がクリックする可視のリンク（URL は [`ExportUrl::as_str`]。上流が `blob:` の `href` を落とすため `data-export-url` に載せ `app.js` が `href` へ写す。`download` に
//! [`crate::history_export::file_name`]）を view で描く（結線済み。TASK-531 1/2）。プログラムからの `.click()` と属性の書き込みは
//! `tests/no_hidden_triggers.rs` が禁止しており、ここでは行わない。リンクを使い終えたら [`ExportUrl::revoke`]
//! （または drop）で URL を破棄する。
//!
//! 例外の文（message）は読まず、記録しない（履歴は受講者のデータのため。`storage_local` と同じ方針）。

use wasm_bindgen::JsValue;
use web_sys::js_sys::{Array, Uint8Array};
use web_sys::{Blob, BlobPropertyBag, Url};

use crate::history_export::MIME;

/// 破棄の責務を持つ `blob:` URL。drop で `revokeObjectURL` を呼ぶ。
pub struct ExportUrl {
    url: String,
}

impl ExportUrl {
    /// `blob:` URL の文字列。
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.url
    }

    /// URL を破棄する。以後この URL からは読めない。
    pub fn revoke(self) {
        drop(self);
    }
}

impl Drop for ExportUrl {
    fn drop(&mut self) {
        // 破棄の失敗は握りつぶす（呼び出し側が回復できず、既に無効な URL を指すだけのため）。
        let _ = Url::revoke_object_url(&self.url);
    }
}

/// バイト列から `blob:` URL を作る。
///
/// # Errors
///
/// `Blob` または URL を作れなかったとき（理由の文は読まない）。
pub fn object_url(bytes: &[u8]) -> Result<ExportUrl, JsValue> {
    let array = Uint8Array::from(bytes);
    let parts = Array::of1(&array);
    let options = BlobPropertyBag::new();
    options.set_type(MIME);
    let blob = Blob::new_with_u8_array_sequence_and_options(&parts, &options)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    Ok(ExportUrl { url })
}
