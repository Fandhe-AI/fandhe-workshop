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
//! 操作 1 件の世界状態の差分から変更前の値つきの `Changed` を作る記録（[`changed`]、TASK-215 1/2）も持つ。
//! 受講者の 1 行をコマンド名と引数に分ける字句化（[`command_lex`]、TASK-262 1/2）も持つ。
//! 字句化の結果を宣言の名前・引数の個数と形で判定する照合（[`command_judge`]、TASK-262 2/2）も持つ。
//! ほかは後続のタスクで追加する。
#![forbid(unsafe_code)]

pub mod addr;
pub mod changed;
pub mod command_judge;
pub mod command_lex;
pub mod effect;
pub mod event;
pub mod history_size;
pub mod input;
pub mod ir_load;
pub mod json_depth;
pub mod load_gate;
pub mod projection;
pub mod provider;
pub mod session;
pub mod step_status;
pub mod version_record;
pub mod world;

pub use ir_load::{check_ir_limits, load_ir, IrLoadError};
pub use projection::Projection;
pub use session::{apply, Session, SessionState};

/// 公開 API の契約の版（TASK-204・TASK-290、版 2）。
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
/// 意図して含めないもの（版上げで足す）。
/// - 早送りの入・切（EVT-27）、判定結果（`Judged`、TASK-256）
/// - 下書きの操作と再読み込み（SAVE-4）
///
/// 状態変化の提供元の境界の版は [`provider::PROVIDER_CONTRACT_VERSION`] で別に持つ。
pub const CONTRACT_VERSION: u32 = 2;
