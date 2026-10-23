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
//! 進行の間隔の定数（TASK-229。D-47）は `tick_interval` に、次の進行の宛先を組み立てる純関数（TASK-349）は `tick_schedule` に置く。
//! エンジン（core の `Session`）を持ち、`Runtime::mount` に渡すホストの Component と、表示部品の振り分け（TASK-347）は `host` に、3 つの表示（ログ・左の成果物・復習）の部品（TASK-347 2/2）は `engine_parts` に置く。
//! 本文の木を画面の部品に変える描画と、リンクの許可スキーム（TASK-149）は `doc_view` に置く。
//! 左の成果物ビューの共通の型・変化の印の対応・束縛点・指せる行の属性・変種の口（TASK-467）は `artifact_view` に置く。版は `artifact_view::ARTIFACT_VIEW_CONTRACT_VERSION`。
//! 世界状態とイベント列から左のビューの行の前後の組を導く処理（TASK-468）は `artifact_rows` に、文書のブロックの突き合わせ（TASK-468 2/2。`-` と `+` の行にする）は `artifact_doc_rows` に置く。
//! 項目 1 件を変化の印つきで作る部品（記号・文字・`data-mark` の 3 手段とインデントのクラス。TASK-469 1/2）は `artifact_mark` に置く。スタイルは `static/app.css`（同一オリジンの外部ファイル）である。
//! ツリーの位置の計算と木の属性（TASK-472。ART-9・ART-13）は `tree_attrs` に置く。
//! ファイルツリーのビュー（変種 (a)・(b)。TASK-473）は `file_tree_view` に置く。
//! カードのビュー（変種 (a)・(b)。TASK-477）は `card_view` に置く。
//! チェックの一覧のビュー（変種 (a)・(b)。TASK-478）は `check_view` に置く。
//! タスクツリーの変種 (a)・(b) の本体（TASK-474。ART-1・A11Y-22）は `task_tree` に置く。
//! 差分のビューの変種 (b) の本体（TASK-475 1/2。ART-1・ART-8）は `diff_view` に置く。
//! 文書のビューの変種 (b) の本体（TASK-476 1/2。ART-1・ART-7。本文の木の `Block` を行の中身に結ぶ）と、対照専用の変種 (a)（TASK-476 2/2。製品では選べない）は `document_view` に置く。
//! 左のビューの共通の検査器（TASK-471 2/3）がマウントする代役の部品（試験の設定だけ）は `view_fixture` に置く。
//! 直接リンク `?start=` の値を IR の `starts` と完全一致で照らして開始点を選ぶ純関数（TASK-109 1/2。DLV-9・SAFE-33）は `start_select` に置く。
//! 教材（IR）を読めなかったホストが出す、旧いエンジンの断りの通知文（VER-13。仮置き）は `ir_notice` に置く。
//! 右のログの行のモデルと描画（TASK-434）は `log_lines` に置く。
//! 状態表示（待機中・処理中。TASK-436）の導出は `run_status` に置く。
//! 章の完了後の「次の章へ進む」（SHELL-10。TASK-526）は `next_chapter` に置く。
//! `answer` の選択肢のボタン（表示名で出す。TASK-445 1/2）は `offers` に置く。
//! 入力方式 `choice` の入力待ちの選択肢（TASK-450 1/3）も `offers` が同じ口で出す。
//! 履歴の保存先の実装と選択の規則（localStorage 既定・IndexedDB への一方向の切り替え。TASK-317）は `storage_select` に置く。
//! wasm 側の実体は `storage_local`（localStorage）と `storage_idb`（IndexedDB と非同期に開く入口）である。
//! 未保存の変更の保留と同期の書き出し（TASK-317 2/2。SAVE-9）は `save_pending` に、`pagehide` で書き出すリスナー（wasm のみ）は `leave_flush` に置く。変更の後の遅延保存の判定（純関数。TASK-318）は `save_schedule` に置く。
//! 受講中の履歴の保存先の結線（起動前に保存先を開いて読み、離れる直前に書き出す。wasm のみ。TASK-319）は `persist` に置く。
//! 候補の一覧の開閉の規則とコマンド表（TASK-440）は `candidates` に置く。
//! 履歴の書き出しファイルの名前と種類（TASK-330 1/2。SAVE-16）は `history_export` に、書き出しのバイト列から `blob:` URL を作る処理（wasm のみ）は `history_download` に置く。受講中の履歴を書き出す唯一の入口 `EngineHost::export_live` は、秘密情報の警告の間は書き出さない `ExportGate` を返す（TASK-164 2/3）。
//! 枠の履歴の書き出し・読み込みの操作の描画（TASK-531 1/2。SHELL-15）は `history_ops` に置く。
//! 保存の直前に起動番号を照合して古いタブの保存を止める処理（TASK-324 1/2。SAVE-12）は `boot_guard` に置く。
//! 画面の枠の区画・取り付け点・操作の名前と宛先・通知の口（種類・値・適合の検査器）の契約（TASK-511。版 2 は TASK-516 1/2・2/2）は `shell` に置く。版は `shell::SHELL_CONTRACT_VERSION`。
//! 枠の通知欄（`shell-notice`）の描画（変換して開いたときの版の違いの通知。TASK-535 1/2。SHELL-20）は `shell_notice` に置く。
//! 画面の枠の組み立て（幅 960px 以上の 2 列の骨格・`tablist`・部品の左右の振り分け・960px 未満のタブの切り替え。TASK-517）は `shell_frame` に置く。
//! 上部の教材名・章の ID・Step の位置（M / K）の導出（TASK-524。SHELL-8）は `top_position` に、描画は `shell_frame::top_place_list` に置く。
#![forbid(unsafe_code)]

pub mod a11y;
pub mod action;
pub mod app_demo_view;
pub mod artifact_doc_rows;
pub mod artifact_mark;
pub mod artifact_rows;
pub mod artifact_view;
pub mod boot_guard;
pub mod browser_notice;
pub mod candidates;
pub mod card_view;
pub mod check_view;
pub mod checkpoint_ops;
pub mod diff_view;
pub mod doc_view;
pub mod document_view;
pub mod engine_parts;
pub mod export_notice;
pub mod file_tree_view;
pub mod glue_ops;
mod hello;
pub mod history_export;
pub mod history_ops;
pub mod host;
pub mod stale_notice;
pub mod view_only_notice;

pub mod input_limit;
pub mod ir_notice;
pub mod key_route;
pub mod learner_text;
pub mod limit_notice;
pub mod list_policy;
pub mod log_lines;
pub mod modifier_key;
pub mod next_chapter;
pub mod offers;
pub mod parts;
pub mod profile;
pub mod run_status;
pub mod save_notice;
pub mod save_pending;
pub mod save_schedule;
pub mod secret_notice;
pub mod shell;
pub mod shell_frame;
pub mod shell_notice;
pub mod start_select;
pub mod step_status;
pub mod storage_select;
pub mod task_tree;
pub mod terminal_area;
pub mod tick_interval;
pub mod tick_schedule;
pub mod top_position;
pub mod tree_attrs;
pub mod view_select;

#[cfg(target_arch = "wasm32")]
mod glue;

#[cfg(target_arch = "wasm32")]
pub mod history_download;
#[cfg(target_arch = "wasm32")]
pub mod leave_flush;
#[cfg(target_arch = "wasm32")]
pub mod persist;
#[cfg(target_arch = "wasm32")]
pub mod storage_idb;
#[cfg(target_arch = "wasm32")]
pub mod storage_local;

#[cfg(all(target_arch = "wasm32", feature = "test-hooks"))]
mod test_hooks;

#[cfg(feature = "test-hooks")]
mod test_fixture;

#[cfg(any(test, feature = "test-hooks"))]
mod view_fixture;
