//! 処理中の進行（`Tick`）の間隔の定数（TASK-229）。
//!
//! 出所は決定 D-47・D-43、EVT-18、PACK-44。進行はイベントの列で進み時刻を持たない（EVT-16）ため、間隔は
//! 表示側のタイマー（TASK-349。`glue::tick_delay_ms`）がこの値を読んで与える。core・contract には時間の値を置かない。
//!
//! 変えられるのはビルド（このソースの変更）のときだけである。URL のクエリ・教材・実行時の設定から
//! 上書きする口は持たず、引数で値を受け取る関数も作らない。この性質は `tests/tick_interval.rs` が固定する。
//! Esc を押す余裕は進行の回数で補う（`docs/pack-authoring.md` の処理中の進行の節）。
//! 早送り（EVT-27）はこの定数の範囲外である。

/// 進行の間隔（ミリ秒）。
pub const TICK_INTERVAL_MS: u32 = 300;

// `setTimeout` の遅延は `i32` で渡すため、収まることを定数の段階で保証する。
const _: () = assert!(TICK_INTERVAL_MS <= i32::MAX as u32);
