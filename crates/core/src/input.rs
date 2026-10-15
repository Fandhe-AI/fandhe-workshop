//! `apply()` が受け取る入力（`Op`）の型（TASK-204 1/3）。
//!
//! 学習者の入力（[`Input`]）、実行環境の操作（[`Control`]）、宛先つきの進行（`Tick`）を [`Op`] にまとめる。
//! 呼び出し元は web（入力の受け口）・保存（履歴の `ops`）。版は 2/3 で付ける。
//!
//! 意図して固定しないもの（版上げで足す）。
//! - 案内レベルとヒントの操作（TASK-290）
//! - 下書きの置き換え・送信・再読み込み（入力欄・保存の契約、SAVE-4）
//! - 早送りの入・切（EVT-27。TASK-15 の決定後）
//!
//! 文字列は利用者由来で信頼できない。長さの上限の判定は各読み込み口の責務で、本型は値を保持するだけである。

use serde::{Deserialize, Serialize};

use crate::addr::{StepUid, TickAddr};

/// 承認の 3 値。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    /// 承認する。
    Approve,
    /// 却下する。
    Reject,
    /// 修正を求める。
    Revise,
}

/// チェックポイントの識別子。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CheckpointId(pub u32);

/// 学習者の入力（教材の語彙の入力方式に対応する）。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Input {
    /// コマンドの 1 行。
    Command(String),
    /// 自由な文章。
    Text(String),
    /// 選択肢の選択（選択肢の ID）。
    Choice(String),
    /// UI 部品の操作。
    Ui {
        /// 表示の名前。
        view: String,
        /// 操作した部品の経路。
        path: String,
    },
    /// 承認の操作。
    Approval {
        /// 承認の 3 値。
        decision: Decision,
        /// 対象（無ければ `None`）。
        target: Option<String>,
    },
    /// 取り消し。処理中でない Esc はこの入力になり、世代を変えない（EVT-10）。
    Cancel,
}

/// 実行環境の操作。教材の語彙ではない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Control {
    /// 処理中の停止。処理中でなければ [`Input::Cancel`] として扱う（EVT-6 の (3)）。
    Stop,
    /// 指定の Step へ移る。
    GoTo(StepUid),
    /// チェックポイントを保存する。
    Save,
    /// チェックポイントへ復帰する。
    Restore(CheckpointId),
    /// 周回を開始する（開始 ID）。
    Begin(String),
}

/// `apply()` が受け取る 1 件。履歴の `ops` の 1 件に当たる。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Op {
    /// 学習者の入力。
    Input(Input),
    /// 宛先つきの進行。
    Tick(TickAddr),
    /// 実行環境の操作。
    Control(Control),
}
