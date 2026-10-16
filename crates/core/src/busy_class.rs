//! 処理中の入力を `busy`（処理中の拒否）に分類する（JUDGE-5、EVT-7、TASK-209 1/2）。
//!
//! EVT-7 は、処理中の Step へ同じ入力を連打しても、受付は（世代, Step）につき 1 回だけで、2 回目以降は
//! 処理中の拒否（[`RejectCategory::Busy`]）になり状態が変わらないことを求める。本モジュールは、その
//! 「処理中か」の判定を状態と入力の種類から決める純関数 [`classify_busy`] を 1 か所に置く。
//! 呼び出し元は、受付の分岐（`crate::accept`。処理中なら受付・確認の処理に進まず、`InputReceived` と
//! `Judged` だけを返す）と、判定の統合（`crate::judge::judge_input`。拒否 `busy` の `Judged` の材料）、
//! 文面の差し込み（[`crate::text_fill`]、TASK-274）である。両者が同じ関数で決めるので、`Judged` の `busy` と受付の分岐は食い違わない。
//! `command_class` と同じく状態を変えず、イベントも出さない。
//!
//! # 規則（上から順に評価し、最初に当たったものを返す）
//!
//! 1. 入力が [`Input::Cancel`] なら `None`（停止・取り消しは TASK-211・TASK-212 が扱う）
//! 2. 現在の Step が無ければ `None`（`Stamp` が作れず判定しない。`accept` の規則 1 と同じ）
//! 3. 処理中（`RunState.busy` が `Some`）なら [`BusyRejection`]。入力方式・Step の状態・入力の中身には依らない
//!    （コマンド・選択・画面操作・承認・自然文のどれでも同じ。処理中の正しい指し先でも達成にならない）
//! 4. それ以外は `None`
//!
//! # 扱わないこと
//!
//! - 受付済み（`Accepted`）で処理中でない Step への再送。確認の判定であり `busy` ではない
//!   （flow のコマンドは `command_class` の `wrong_order`、残り続けるボタンは規則による拒否で、試験は `tests/button_burst.rs`）
//! - 達成済み・開始前の Step への入力（`not_in_this_step` は [`crate::judge`] が決める）
//! - 現在の Step が IR に無い場合の扱い（`accept` が先に固定文の `Warning` を出す。本関数は IR を読まない）
//! - `Judged` の発行・文面の解決と差し込み（TASK-267・TASK-274）
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。
//! 入力値は保持も複製もしない。計算量は定数である。

use fandhe_workshop_contract::judgment::RejectCategory;

use crate::input::Input;
use crate::session::{Busy, RunState};

/// 処理中の拒否。進み具合（`done`・`total`）を持ち、文面の枠（`remaining`・`total`）の材料になる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusyRejection {
    /// 拒否した時点の処理中の状態の写し。
    pub busy: Busy,
}

impl BusyRejection {
    /// 拒否の分類。常に [`RejectCategory::Busy`]。
    pub fn category(&self) -> RejectCategory {
        RejectCategory::Busy
    }
}

/// 状態と入力から、処理中の拒否かを決める。規則と扱わないことはモジュール文書にある。
pub fn classify_busy(run: &RunState, input: &Input) -> Option<BusyRejection> {
    if matches!(input, Input::Cancel) {
        return None;
    }
    run.current.as_ref()?;
    run.busy.clone().map(|busy| BusyRejection { busy })
}
