//! 教材実行の核（状態遷移・完了判定）を置く crate の入口。
//!
//! web・表示基盤に依存しない。web と pack から呼ばれる。
//! 現時点は IR の読み込みの入口（`ir_load`）と、状態変化の提供元の境界（[`provider`]、TASK-187）、
//! 履歴ファイルの大きさの検査（[`history_size`]、TASK-155）、解釈の前の入れ子の深さの検査
//! （[`json_depth`]、TASK-157）、それを含む判定の順序の関門（[`load_gate`]、TASK-157）を持つ。
//! 入力・イベント・Step の状態の型（[`addr`]・[`input`]・[`event`]・[`step_status`]・[`world`]、TASK-204 1/3）と、
//! 実行環境 [`Session`]・遷移関数 [`apply`]・導出 [`Projection`]（[`session`]・[`projection`]、TASK-204 2/3。導出の中身は TASK-215 2/2）も持つ。
//! 履歴の 4 軸の版の記録と欠落の検査（[`version_record`]、TASK-173 2/2）も持つ。
//! 状態変化の語彙（要素・属性・フラグ）の 1 語ごとの適用と、`when` つきの列・進行の順次適用（[`effect`]、TASK-43）も持つ。
//! 条件の語彙（状態・世界状態・論理）の評価と、`when` に差し込む評価器（[`condition`]、TASK-42 1/2）も持つ。
//! 操作 1 件の世界状態の差分から変更前の値つきの `Changed` を作る記録（[`changed`]、TASK-215 1/2）も持つ。
//! 受講者の 1 行をコマンド名と引数に分ける字句化（[`command_lex`]、TASK-262 1/2）も持つ。
//! 字句化の結果を宣言の名前・引数の個数と形で判定する照合（[`command_judge`]、TASK-262 2/2）も持つ。
//! 選択肢・画面操作・承認の判定（[`select_judge`]、TASK-263）も持つ。
//! 承認の「直す」の対象の一覧と対象なしの規則（[`select_judge::revise_targets`]）、拒否の文面の解決と
//! 空でない次の行動（[`rejection_text`]、TASK-264）も持つ。
//! 自然文の受付の判定と、規則の効果の適用（[`text_judge`]、TASK-266）も持つ。
//! 判定結果の統合と `Judged` の材料（[`judge`]、TASK-267）も持つ。
//! 入力の受付の分岐（`accept`、TASK-207 1/3。公開しない）も持つ。
//! 確認の操作による達成と外れた指し先（`confirm`、TASK-207 2/3。公開しない）も持つ。
//! コマンド入力の規則の照合と問い返しの判定（[`command_rule`]、TASK-272）も持つ。
//! 学習目標の確認の記録に当てる、最初の回答の正答だけを自力に数える規則（[`lo_first_answer`]、TASK-305、GUIDE-18）も持つ。
//! コマンド入力の規則による受付の効果の適用（受け付けたうえで止まる場面、[`command_accept`]、TASK-273）も持つ。
//! コマンドの表による未知のコマンド・順序違い・教材未対応・この Step では使わないコマンドの分類（[`command_class`]、TASK-268・TASK-269）も持つ。
//! 処理中の入力の `busy` の分類（[`busy_class`]、TASK-209 1/2）も持つ。
//! 受講者の操作を履歴の `ops` に追記する記録（[`ops_record`]、TASK-315 1/2）も持つ。
//! `ops`・写し・版の欄を履歴の包みに組み立てる処理と下書きの同期（[`history_envelope`]、TASK-315 2/2）も持つ。
//! `ops` を初期状態に順に適用し直す復元と送信の分類（[`history_replay`]、TASK-316 1/3）も持つ。
//! 履歴の読み込みの段 1〜6 の入口（大きさ・深さ・JSON・版の欄・型・版の判定。[`history_load`]、TASK-331 1/3・2/3）も持つ。
//! 履歴の包みを書き出すバイト列にする処理と大きさの報告（[`history_export`]、TASK-330 1/2）も持つ。
//! 保存の前に履歴の包みが読み込みの上限を超えていないかを判定する処理（[`history_limit`]、Issue #1414）も持つ。
//! 履歴を開く側の版と突き合わせて開く可否を判定する処理（[`history_open`]、TASK-174）も持つ。
//! 拒否・問い返し・注記の文面への実行時の値の差し込み（[`text_fill`]、TASK-274）も持つ。
//! 宛先が一致する進行の前進と食い違う進行の無視の記録（`tick`、TASK-210 2/3・3/3。公開しない）も持つ。
//! 処理中の停止と入口の写しへの復帰（`stop`、TASK-211 1/2。公開しない）も持つ。
//! 別の Step への移動（`go_to`、TASK-213 1/2。公開しない）も持つ。
//! チェックポイントの保存と復帰（`checkpoint`、TASK-214 1/3。公開しない）も持つ。
//! 1 回の操作で起きる内部の遷移のたどりと上限 1,000 回での停止と、止めた後に操作の前の状態へ戻すこと（`transit`、TASK-160、SAFE-23・SAFE-35。公開しない）も持つ。
//! 処理中の再読み込みでの世代 +1（`reload`、TASK-320 1/2。公開しない）も持つ。
//! 操作を `ops` に記録しながら進める実行環境（[`recorded_session`]、TASK-319 1/3）と、起動時に保存済みの履歴の有無で新しく始めるか復元するかを分ける処理（[`history_boot`]）も持つ。
//! ほかは後続のタスクで追加する。
#![forbid(unsafe_code)]

mod accept;
pub mod addr;
pub mod busy_class;
pub mod changed;
mod checkpoint;
pub mod command_accept;
pub mod command_class;
pub mod command_judge;
pub mod command_lex;
pub mod command_rule;
pub mod condition;
mod confirm;
pub mod effect;
pub mod event;
mod go_to;
pub mod history_boot;
pub mod history_envelope;
pub mod history_export;
pub mod history_limit;
pub mod history_load;
pub mod history_open;
pub mod history_replay;
pub mod history_size;
pub mod input;
pub mod ir_load;
pub mod json_depth;
pub mod judge;
pub mod lo_first_answer;
pub mod load_gate;
pub mod ops_record;
pub mod projection;
pub mod provider;
pub mod recorded_session;
pub mod rejection_text;
mod reload;
pub mod select_judge;
pub mod session;
pub mod step_status;
mod stop;
pub mod text_fill;
pub mod text_judge;
mod tick;
mod transit;
pub mod version_record;
pub mod world;

pub use ir_load::{check_ir_limits, load_ir, IrLoadError};
pub use projection::Projection;
pub use session::{apply, Session, SessionState};

/// 公開 API の契約の版（TASK-204・TASK-290・TASK-267・TASK-320・TASK-160、版 6）。
///
/// 覆う範囲は [`addr`]・[`input`]・[`event`]・[`step_status`]・[`world`]・[`session`]・[`projection`] の
/// 公開の型と署名、および [`input::GenerationRule`] の表である。次のいずれかを変えたら上げる。
/// - 欄・variant の追加・削除・改名、直列化の形
/// - 関数・メソッドの署名
/// - 世代の規則の表
///
/// 版 2 で足したもの: 案内レベルとヒントの操作（`Control::OpenHint`・`Control::SetGuidance`）、イベント
/// （`Event::HintOpened`・`RefuseReason::NoHint`・`ControlKind::OpenHint`）、`RunState.guidance_level`、
/// `SessionState.hints_opened`。案内レベルの値の型は `fandhe_workshop_contract::guidance` にある。
///
/// 版 3 で足したもの: 判定結果のイベント `Event::Judged`（JUDGE-3）。`Op::Input` では `InputReceived` の直後に 1 件置く。
///
/// 版 4 で足したもの: 再読み込み（SAVE-4）の `Event::Reloaded`・`session::reload`・`Session::reload`。
/// `Op` と世代の規則の表は変えない（再読み込みは `apply()` を経由しない）。
///
/// 版 5 で足したもの: 問い返しの待ちの欄 `session::Slice::clarifying`（EP-4）。旧データは既定で読め、`None` は書き出さない。
///
/// 版 6 で足したもの: 遷移の上限で止めたことの専用のイベント `Event::TransitionLimitReached`（SAFE-34・D-45）。
/// 作成者向けの `Warning`（SAFE-23）は残し、その直後に 1 件置く。
///
/// 意図して含めないもの（版上げで足す）。
/// - 早送りの入・切（EVT-27）
/// - 下書きの操作
///
/// 状態変化の提供元の境界の版は [`provider::PROVIDER_CONTRACT_VERSION`] で別に持つ。
pub const CONTRACT_VERSION: u32 = 6;
