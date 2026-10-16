//! ブラウザ試験が mount するホストの開始状態（試験の設定 `test-hooks` だけ。TASK-347 2/2）。
//!
//! 呼び出し元は `host::EngineHost::for_mount` だけである。`lib.rs` が feature `test-hooks` の背後でだけ
//! 組み込むため、配信する設定の wasm には入らない（`tools/check-delivery.sh` が教材の文字列の不在を確かめる）。
//!
//! 開始（`Control::Begin`）はまだ core が実装していないため、契約の試料（`event-sequence.json`）が持つ
//! 開始直後の状態から始める。状態からの `Session` の再構築は、表示層で他に許されない操作（`tests/no_progress.rs`）で、
//! このファイルの 1 箇所だけを例外にしている。教材と状態は埋め込みの試験データで、外から入らない。

use fandhe_workshop_core::{load_ir, Session, SessionState};

const IR: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/engine-event-consistency/test-data/min-3step.json"
));
const SAMPLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../testdata/contract/event-sequence.json"
));

/// 試験用の教材と開始直後の状態から実行環境を作る。読めなければ `None`（呼び出し側は session なしに倒す）。
pub(crate) fn start_session() -> Option<Session> {
    let ir = load_ir(IR).ok()?;
    let value: serde_json::Value = serde_json::from_slice(SAMPLE).ok()?;
    let state: SessionState = serde_json::from_value(value.get("initial")?.clone()).ok()?;
    Some(Session::from_state(ir, state))
}
