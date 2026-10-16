//! コマンドの表と Step の状態による、コマンドの拒否の分類（JUDGE-4・JUDGE-5、TASK-268 1/2・TASK-269）。
//!
//! JUDGE-4 は、受け付けない入力にエンジンが持つ分類 1 つを付けることを求める。本モジュールは
//! `unknown_command`・`wrong_order`・`not_supported`・`not_in_this_step`（コマンドの表と Step の状態で決まる
//! 4 場面）を決める純関数を置く。呼び出し元は、`Judged` の発行（[`crate::judge`]、TASK-267）と教材テスト
//! （TASK-404、pack）である。引数不足と引数の形（TASK-268 2/2）は [`crate::command_judge`] が返し、4 場面の教材テストは
//! pack の結合試験 `reject_four_scenes` が持つ。文面の解決と空でない次の行動は [`crate::rejection_text`] が持つ。
//!
//! # 規則（上から順に評価し、最初に当たったものを返す）
//!
//! 出所は PoC-3 の語彙 v1 の 3 節と、試作の `classify_command` である。
//!
//! 1. 名前が `commands` のキーに無ければ [`CommandClass::Unknown`]（大文字小文字を区別する完全一致）
//! 2. 表の値を [`CommandKind`]（閉じた 4 語の文字列）として読む。読めなければ [`CommandClassError::InvalidKind`]
//!    （受付にも別の分類にも倒さない。引いた 1 件だけを読み、他の行の不正には影響されない）
//! 3. 種類が `unsupported` なら [`CommandClass::NotSupported`]。判定順は「この Step が受け付けるか」より前に置き、
//!    `unsupported` が順序違いや受付に落ちないようにする（Step の段階に関係なく返す）
//! 4. 状態が `Entered` で、`Step.input` がコマンドの宣言で、その名前が入力と一致すれば [`CommandClass::Accepting`]。
//!    宣言は `Entered` のときだけ読む。宣言が読めなければ [`CommandClassError::InvalidStep`]
//!    （`Step.input` がコマンドでない場合は「受け付けない」でありエラーにしない）
//! 5. 種類が `flow` なら [`CommandClass::WrongOrder`]（確認待ちの `Accepted` での再送も同じ）
//! 6. 残り（`builtin`・`skill`）は [`CommandClass::NotInThisStep`]
//!
//! コマンドの種類の 4 語は、現行の `docs/vocabulary/vocabulary.md` に無い。語彙への取り込みは別の作業で扱う。
//!
//! # 扱わないこと
//!
//! - `busy`・`missing_arg`・`bad_arg_shape` の分類（`busy` は [`crate::busy_class`]、後者 2 つは
//!   [`crate::command_judge`] が持つ）
//! - 接頭辞の無い行と字句の不正（[`crate::command_lex`] のエラー）の分類。呼び出し側（[`crate::judge`]）が
//!   `not_in_this_step` にする
//! - 文面の解決と差し込み（[`crate::rejection_text`]・TASK-274）、状態の変更、`Judged` の発行
//!
//! 外部への呼び出し・正規表現の評価は持たない。全域（panic しない）で決定的で、計算量は表を 1 件引くことと
//! Step の宣言を 1 回読むことに限られる。エラーの `Display` は固定の英文で、名前・表の値・教材の値を含めない。

use std::fmt;

use fandhe_workshop_contract::ir::{Ir, Step};
use fandhe_workshop_contract::judgment::{LexedCommand, RejectCategory};
use serde_json::Value;

use crate::command_rule::{command_spec, CommandRuleError};
use crate::step_status::StepStatus;

/// IR の `commands` の値（コマンドの種類）。閉じた 4 語。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandKind {
    /// 組み込み（`builtin`）。
    Builtin,
    /// スキル（`skill`）。
    Skill,
    /// 流れを進めるコマンド（`flow`）。
    Flow,
    /// 教材が扱わないコマンド（`unsupported`）。
    Unsupported,
}

impl CommandKind {
    /// 正準の文字列だけを受ける。文字列以外・未知の語・大文字違いは `None`。
    pub fn from_value(v: &Value) -> Option<Self> {
        match v.as_str()? {
            "builtin" => Some(Self::Builtin),
            "skill" => Some(Self::Skill),
            "flow" => Some(Self::Flow),
            "unsupported" => Some(Self::Unsupported),
            _ => None,
        }
    }
}

/// 分類の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandClass {
    /// 表に無い名前（`unknown_command`）。
    Unknown,
    /// `flow` で、この Step が今受け付けるコマンドでない（`wrong_order`）。
    WrongOrder,
    /// この Step が今受け付けるコマンド。宣言との照合へ進む。
    Accepting,
    /// 種類が `unsupported`（`not_supported`）。Step の段階に関係なく、受付の判定より先に決まる。
    NotSupported,
    /// 種類が `builtin`・`skill` で、この Step・この段階では受け付けない（`not_in_this_step`）。
    /// 種類は試験と記録のために持つ。
    NotInThisStep(CommandKind),
}

impl CommandClass {
    /// 拒否の分類。[`Self::Accepting`] だけが `None`（受付の判定へ進む）。
    pub fn category(&self) -> Option<RejectCategory> {
        match self {
            Self::Unknown => Some(RejectCategory::UnknownCommand),
            Self::WrongOrder => Some(RejectCategory::WrongOrder),
            Self::NotSupported => Some(RejectCategory::NotSupported),
            Self::NotInThisStep(_) => Some(RejectCategory::NotInThisStep),
            Self::Accepting => None,
        }
    }
}

/// 判定できない理由。`Display` は固定の英文で、名前・表の値・教材の値を埋め込まない。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandClassError {
    /// `commands` の値が 4 語の文字列でない。
    InvalidKind,
    /// 状態が `Entered` で、`Step.input` がコマンドの宣言だが読めない。
    InvalidStep,
}

impl fmt::Display for CommandClassError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKind => f.write_str("command table entry cannot be read"),
            Self::InvalidStep => f.write_str("step command declaration cannot be read"),
        }
    }
}

impl std::error::Error for CommandClassError {}

/// この Step が、この状態で、この名前のコマンドを受け付けるか。
///
/// 宣言は `Entered` のときだけ読む。それ以外の状態では受け付けられないので、壊れた宣言も見ない。
fn accepts(step: &Step, status: StepStatus, name: &str) -> Result<bool, CommandClassError> {
    if status != StepStatus::Entered {
        return Ok(false);
    }
    match command_spec(step) {
        Ok(spec) => Ok(spec.name == name),
        Err(CommandRuleError::NotCommand) => Ok(false),
        Err(_) => Err(CommandClassError::InvalidStep),
    }
}

/// コマンドの表と Step の状態で、字句化済みのコマンドを分類する。純関数・全域・決定的。
pub fn classify_command(
    ir: &Ir,
    step: &Step,
    status: StepStatus,
    lexed: &LexedCommand,
) -> Result<CommandClass, CommandClassError> {
    let Some(entry) = ir.commands.get(&lexed.name) else {
        return Ok(CommandClass::Unknown);
    };
    let kind = CommandKind::from_value(entry).ok_or(CommandClassError::InvalidKind)?;
    if kind == CommandKind::Unsupported {
        return Ok(CommandClass::NotSupported);
    }
    if accepts(step, status, &lexed.name)? {
        return Ok(CommandClass::Accepting);
    }
    if kind == CommandKind::Flow {
        return Ok(CommandClass::WrongOrder);
    }
    Ok(CommandClass::NotInThisStep(kind))
}
