//! 教材データ・イベント・成果物の契約型を置く crate の入口。
//!
//! core と web が共有する型の置き場で、表示基盤にも core にも依存しない。
//! 現時点は IR の型と `ir_format` 1・2 の契約（`ir`、TASK-41・TASK-60）、本文と練習データの木の型（`doc`、TASK-60）、未信頼入力の上限の契約（[`limits`]、TASK-199）、履歴の版と判定結果の契約（[`history_version`]、TASK-173）、`content_hash` の求め方（[`content_hash`]、TASK-180）、履歴の入力の列・書き出しの形・履歴の包み（[`history`]、TASK-309）、案内レベルとヒントの値の型（[`guidance`]、TASK-290）、入力の判定結果・拒否の分類・次の行動・字句化の型（[`judgment`]、TASK-256）、入力方式 5 種の宣言の型と `text` の判定（[`input_spec`]、TASK-44）、学習目標の確認の記録の型と導出（[`lo_check`]、TASK-291）、操作プロファイルの接頭辞の規則と入力の分け（[`profile`]、TASK-52）、確認 4 種の宣言の型と受付・達成の分け方（[`confirm_spec`]、TASK-47）、外観の閉じた集合と読み取り（[`appearance`]、TASK-53）、左エリアの設定の閉じた集合と読み取り（[`left_pane`]、TASK-53 2/2）を持つ。
//! 状態変化の語彙（要素・属性・フラグ）の効果の型と `Value` からの解釈（[`effect`]、TASK-43）と、進行（`processing`）の解釈も持つ。
//! 文面の差し込みの枠の閉じた集合・構文の読み取り・差し込み（[`slot`]、TASK-46）も持つ。
//! 保存先の口と履歴の鍵の生成規則（[`storage`]、TASK-310）も持つ。
//! キー判定表 `decide` の入出力の型と行の判定（[`key_decide`]、TASK-257 版 1、TASK-281 版 2）も持つ。
//! 読み込みの拒否理由コードの一覧と判定の順序（[`load_reason`]、TASK-310）も持つ。
//! ほかの型は後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod appearance;
pub mod confirm_spec;
pub mod content_hash;
pub mod doc;
pub mod effect;
pub mod guidance;
pub mod history;
pub mod history_version;
pub mod input_spec;
pub mod ir;
pub mod judgment;
pub mod key_decide;
pub mod left_pane;
pub mod limits;
pub mod lo_check;
pub mod load_reason;
pub mod profile;
mod sha256;
pub mod slot;
pub mod storage;
