//! ブラウザ向けの表示層の crate の入口。
//!
//! core・contract を呼んで DOM を更新する。入力欄（Hello）を内包するホスト（`host`）を mount する。
//! 教材の描画は後続のタスクで追加する。
//!
//! wasm 側の入口は `glue`（`mount_app`）で、`tools/build-web.sh` が wasm-bindgen で
//! JS の glue に変換して配る。試験用の口は feature `test-hooks` の背後にだけ置く。
//!
//! アクセシビリティの構造の契約（TASK-366）は `a11y` に、受講者の文字列を非解釈で描く関数（SAFE-11）は
//! `learner_text` に置く。
//! 右のターミナル風エリアの語（ログの行・状態表示・Step の状態・入力の口の識別子。TASK-431）と適合の検査は `terminal_area` に置く。
//! 表示部品をホストに載せる口の契約（TASK-343）は `parts` に、action の名前と宛先は `action` に、
//! 操作プロファイル・外観・左エリアの設定の渡し方は `profile` に置く。
//! glue の許される操作は `glue_ops` に、keyed list の親の属性と項目数の方針は `list_policy` に置く。
//! これら TASK-343 の契約の版は `parts::PARTS_CONTRACT_VERSION` が持つ。
//! keydown の判定結果を glue の効果と Esc の行き先（停止・候補を閉じる・取り消し）へ変える純関数（TASK-275・TASK-277）、Esc の後の状態遷移と補完に使う候補の選択（TASK-282）は `key_route` に置く。
//! 進行の間隔の定数（TASK-229。D-47）は `tick_interval` に置く。
//! エンジン（core の `Session`）を持ち、`Runtime::mount` に渡すホストの Component と、表示部品の振り分け（TASK-347）は `host` に、3 つの表示（ログ・左の成果物・復習）の部品（TASK-347 2/2）は `engine_parts` に置く。
//! 本文の木を画面の部品に変える描画と、リンクの許可スキーム（TASK-149）は `doc_view` に置く。
//! 左の成果物ビューの共通の型・変化の印の対応・束縛点・指せる行の属性・変種の口（TASK-467）は `artifact_view` に置く。版は `artifact_view::ARTIFACT_VIEW_CONTRACT_VERSION`。
#![forbid(unsafe_code)]

pub mod a11y;
pub mod action;
pub mod artifact_view;
pub mod doc_view;
pub mod engine_parts;
pub mod glue_ops;
mod hello;
pub mod host;

pub mod input_limit;
pub mod key_route;
pub mod learner_text;
pub mod list_policy;
pub mod parts;
pub mod profile;
pub mod terminal_area;
pub mod tick_interval;

#[cfg(target_arch = "wasm32")]
mod glue;

#[cfg(all(target_arch = "wasm32", feature = "test-hooks"))]
mod test_hooks;

#[cfg(feature = "test-hooks")]
mod test_fixture;
