//! ブラウザ向けの表示層の crate の入口。
//!
//! core・contract を呼んで DOM を更新する。現時点は最小の画面（Hello）だけを持ち、
//! 教材の描画は後続のタスクで追加する。
//!
//! wasm 側の入口は `glue`（`mount_app`）で、`tools/build-web.sh` が wasm-bindgen で
//! JS の glue に変換して配る。試験用の口は feature `test-hooks` の背後にだけ置く。
#![forbid(unsafe_code)]

mod hello;

#[cfg(target_arch = "wasm32")]
mod glue;

#[cfg(all(target_arch = "wasm32", feature = "test-hooks"))]
mod test_hooks;
