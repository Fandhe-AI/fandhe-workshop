//! `apply()` が返すイベントの型（TASK-204 1/3）。
//!
//! 表示・保存・案内の担当が 1 つの型だけを見ればよいよう、エンジンのイベントと実行環境のイベントを
//! [`Event`] 1 本にまとめる。すべての variant が [`Stamp`]（`at`）を持つ。世代が変わる variant は
//! `new_generation` を持つ（EVT-6 の (3)(4)(6) と再読み込み（SAVE-4）。表は [`crate::input::GenerationRule`]）。契約の版は [`crate::CONTRACT_VERSION`]。
//!
//! 意図して固定しないもの（版上げで足す）。
//! - 案内レベルの切り替えのイベント化と履歴での扱い（GUIDE-16。TASK-307・TASK-308 が決める。`Control::SetGuidance` はイベントを出さない）
//!
//! 文字列（`Warning::message`、`Changed` の値）は教材・利用者由来を含みうる。HTML へ埋め込む際の
//! エスケープは web 層の責務である。`Warning` の文面に利用者の入力値を埋め込まない。

use serde::{Deserialize, Serialize};

use fandhe_workshop_contract::judgment::Judgment;

use crate::addr::{Generation, Stamp, StepUid};
use crate::input::{CheckpointId, Input};
use crate::world::Element;

/// 世界状態の変更の原因。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Cause {
    /// 学習者の入力。
    Input,
    /// 停止。
    Stop,
    /// 移動。
    Move,
    /// 復帰。
    Restore,
    /// 周回の開始。
    Start,
}

/// 実行環境の操作の種類（拒否の対象を示す）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ControlKind {
    /// 停止。
    Stop,
    /// 移動。
    GoTo,
    /// 保存。
    Save,
    /// 復帰。
    Restore,
    /// 周回の開始。
    Begin,
    /// ヒントを開く。
    OpenHint,
}

/// 操作を拒否した理由。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RefuseReason {
    /// 無いチェックポイント。
    UnknownCheckpoint,
    /// 無い Step。
    UnknownStep,
    /// 開始の条件を満たさない。
    StartUnmet,
    /// 無い開始 ID。
    UnknownStart,
    /// 開けるヒントの段が無い、または現在の Step が無い（GUIDE-17）。
    NoHint,
    /// 閉じた周回の Step への移動（MOD-15・D-24。TASK-249）。
    ClosedRound,
}

/// 拒否の後に示す次の行動（TASK-249）。受講者向けの文面は持たず、web が理由から引いて文面に当てる。
///
/// 【案】`ClosedRound` には今の周回を続けることを示す。チェックポイントへの復帰を勧めると閉じた周回へ戻る操作へ誘うことになり、
/// D-24 の趣旨に反するため。決定が出たら [`RefuseReason::next_action`] の腕だけを直す。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum RefuseNextAction {
    /// 今の周回を続ける。
    ContinueCurrentRound,
}

impl RefuseNextAction {
    /// すべての行動。
    pub const ALL: [RefuseNextAction; 1] = [RefuseNextAction::ContinueCurrentRound];

    /// 行動の識別子（ASCII の snake_case）。
    pub const fn as_str(self) -> &'static str {
        match self {
            RefuseNextAction::ContinueCurrentRound => "continue_current_round",
        }
    }
}

impl RefuseReason {
    /// 理由ごとの次の行動。割り当てていない理由は `None`（TASK-249 の範囲外）。
    pub const fn next_action(self) -> Option<RefuseNextAction> {
        match self {
            RefuseReason::ClosedRound => Some(RefuseNextAction::ContinueCurrentRound),
            RefuseReason::UnknownCheckpoint
            | RefuseReason::UnknownStep
            | RefuseReason::StartUnmet
            | RefuseReason::UnknownStart
            | RefuseReason::NoHint => None,
        }
    }
}

/// 入力・進行を無視した理由。宛先の食い違いはコマンドの分類より先に判定する（EVT-8）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum IgnoreReason {
    /// 世代・Step・進行の番号のいずれかが古い（同じ番号の二重の届きを含む）。
    Stale,
    /// 現在の Step が無い。
    NoCurrent,
    /// 処理中でない。
    NoProcessing,
}

/// イベント。`apply()` の戻りのイベント列の 1 件である。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum Event {
    /// Step に入った。
    Entered {
        /// その時点の世代と Step。
        at: Stamp,
    },
    /// モジュールに入った。
    ModuleEntered {
        /// その時点の世代と Step。
        at: Stamp,
        /// モジュールの実体名。
        instance: String,
    },
    /// モジュールを出た。
    ModuleExited {
        /// その時点の世代と Step。
        at: Stamp,
        /// モジュールの実体名。
        instance: String,
    },
    /// 学習者の入力を受け取った。
    InputReceived {
        /// その時点の世代と Step。
        at: Stamp,
        /// 受け取った入力。
        input: Input,
    },
    /// 進行が 1 つ進んだ。
    Progress {
        /// その時点の世代と Step。
        at: Stamp,
        /// IR の進行の名前。
        name: String,
        /// 何番目の進行か（0 始まり）。
        index: u32,
        /// 進行の総数。
        total: u32,
    },
    /// 入力の判定結果（JUDGE-3、TASK-267）。受付・拒否・問い返し・確認・進行・無視の 6 種のどれか 1 つで、
    /// `Op::Input` では `InputReceived` の直後（`Accepted`・効果・`Achieved` より前）に 1 件だけ置く。
    ///
    /// 結果の型は contract の `judgment::Judgment` が正である。文面の ID・次の行動は教材由来の未信頼の
    /// 文字列で、core は保持するだけである。HTML へのエスケープは web 層の責務とする。
    Judged {
        /// その時点の世代と Step。
        at: Stamp,
        /// 判定結果。
        judgment: Judgment,
    },
    /// 入力を受け付けた。
    Accepted {
        /// その時点の世代と Step。
        at: Stamp,
    },
    /// 完了条件を満たした。
    Achieved {
        /// その時点の世代と Step。
        at: Stamp,
        /// 学習者が明示的に省略して達成としたか。
        explicit_skip: bool,
    },
    /// Step を省略した。
    Skipped {
        /// その時点の世代と Step。
        at: Stamp,
    },
    /// 次の Step へ遷移した。
    Transitioned {
        /// その時点の世代と Step。
        at: Stamp,
        /// 遷移先。
        to: StepUid,
    },
    /// 章を完了した。
    ChapterCompleted {
        /// その時点の世代と Step。
        at: Stamp,
    },
    /// フラグが変わった。
    Flag {
        /// その時点の世代と Step。
        at: Stamp,
        /// フラグの範囲。
        scope: String,
        /// フラグの名前。
        name: String,
        /// 入か切か。
        on: bool,
    },
    /// 世界状態が変わった（1 経路 1 件。EVT-13）。
    Changed {
        /// その時点の世代と Step。
        at: Stamp,
        /// 変わった経路。
        path: String,
        /// 変更前（無ければ `None`）。
        before: Option<Element>,
        /// 変更後（無くなれば `None`）。
        after: Option<Element>,
        /// 変更の原因。
        cause: Cause,
    },
    /// 停止した。
    Stopped {
        /// その時点の世代と Step。
        at: Stamp,
        /// 停止後の世代。
        new_generation: Generation,
    },
    /// 指定の Step へ移った。
    Moved {
        /// その時点の世代と Step。
        at: Stamp,
        /// 移動先。
        to: StepUid,
        /// 移動後の世代。
        new_generation: Generation,
    },
    /// チェックポイントを保存した。
    CheckpointSaved {
        /// その時点の世代と Step。
        at: Stamp,
        /// 保存したチェックポイント。
        id: CheckpointId,
        /// 保存時点のログの長さ。
        log_len: u32,
    },
    /// チェックポイントへ復帰した（EVT-12・EVT-14）。
    Restored {
        /// その時点の世代と Step。
        at: Stamp,
        /// 復帰したチェックポイント。
        id: CheckpointId,
        /// 復帰後の世代。
        new_generation: Generation,
        /// 復帰したチェックポイントの保存の時点のログの長さ（`CheckpointSaved.log_len` と同じ。EVT-14）。
        log_len: u32,
        /// 復帰後の現在の Step。
        owner: StepUid,
    },
    /// 周回を開始した。
    Begun {
        /// その時点の世代と Step。
        at: Stamp,
        /// 開始 ID。
        start: String,
        /// 開始後の世代。
        new_generation: Generation,
    },
    /// 再読み込みした（SAVE-4）。処理中のまま続け、世代だけを +1 する。`apply()` の `Op` ではなく、
    /// `session::reload` が出す。
    Reloaded {
        /// 操作前の世代と現在の Step。
        at: Stamp,
        /// 再読み込み後の世代（保存した世代 +1）。
        new_generation: Generation,
    },
    /// ヒントを 1 段開いた（GUIDE-17）。世代は変わらない。
    HintOpened {
        /// その時点の世代と Step。
        at: Stamp,
        /// 開いた後の段数。
        opened: u32,
        /// その時点で出せる段数（`fandhe_workshop_contract::guidance::visible_hints` の数）。
        total: u32,
    },
    /// 操作を拒否した。
    Refused {
        /// その時点の世代と Step。
        at: Stamp,
        /// 拒否した操作の種類。
        op: ControlKind,
        /// 拒否の理由。
        reason: RefuseReason,
        /// 拒否の文面の識別子。`Begin` の開始条件の拒否では、解決できた `unmet` の文面 ID を入れる（TASK-240）。
        /// 無い・解決できないときは `None`（TASK-256 の型ができるまでの識別子）。
        rejection: Option<String>,
    },
    /// 入力・進行を無視した。状態は変えず 1 件だけ記録する。
    Ignored {
        /// その時点の世代と Step。
        at: Stamp,
        /// 無視した理由。
        reason: IgnoreReason,
    },
    /// 1 回の操作で起きる内部の遷移が上限に達し、実行を止めた（SAFE-34・D-45）。教材の定義の誤りではなく、
    /// 実行時の上限である。作成者向けの [`Event::Warning`]（SAFE-23）の直後に 1 件だけ置く。
    ///
    /// 受講者向けの見出し・案内・まとめ表示は、この variant だけから web が作る。文面は web が固定文で持つので、
    /// 本 variant は文字列の欄を持たない（教材の値が入り込む経路を作らない）。
    TransitionLimitReached {
        /// 止めた地点の世代と Step。
        at: Stamp,
        /// 止めるまでに数えた着地の回数（現行の上限では 1,000）。
        landings: u32,
    },
    /// 教材の定義の誤りの診断。作成者向けで、状態・イベントの決定には使わない。
    Warning {
        /// その時点の世代と Step。
        at: Stamp,
        /// 診断の文面。
        message: String,
    },
}
