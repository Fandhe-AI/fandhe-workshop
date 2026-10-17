//! Step の状態表示（`step-status`）の導出（TASK-435 1/2。TERM-31）。
//!
//! 呼び出し元は `engine_parts` の `LogPart` で、描画のたびと束縛点の更新のたびに [`step_status`] を引いて
//! 右の操作エリアの Step の状態の語（`受付済み（確認待ち）`・`達成`）を決める。語そのものは
//! `terminal_area::step_status_word`（TASK-431 の契約）が持ち、ここは `Session` からどの値を選ぶかだけを持つ。
//!
//! 表示層は進行管理を持たない（`tests/no_progress.rs` が固定する）ため、`Session` は読み取りだけに使う。
//! 「達成」を表示側で作らない。確認の前に達成を出さない保証は core（EVT-3・EVT-5）にあり、ここはそれを写す。
//!
//! 規則は次の順で、未読込・未知のイベントは「語なし」（`None`）に倒す。
//!
//! 1. 現在の Step の状態が `Accepted` なら `Accepted`。処理中（`busy`）でも同じで、進行の間も受付済みである。
//!    `offers` は「確認待ち = `Accepted` かつ処理中でない」と読むが、あちらは確認の口を出してよいかの判断で
//!    目的が違う。
//! 2. 現在の Step の状態が `Achieved` なら `Achieved`（`next` を持たない最後の Step など）。
//! 3. それ以外はログを末尾から読む。core は達成の同じ操作の中で `next` へ遷移して現在の Step を移すため、
//!    現在の Step だけを見ると、確認を省く Step や確認の直後の「達成」が一瞬も見えない。読み飛ばすのは
//!    `Entered`・`Transitioned`・`Changed`・`Flag`・`ModuleEntered`・`ModuleExited`・`ChapterCompleted`・
//!    `Warning`・`Ignored` の狭い許可リストだけで、最初に当たったイベントが `Achieved` であり、かつ状態の上でも
//!    その Step が `Achieved` のときだけ `Achieved` とする。それ以外（未知のイベントを含む）は `None`。
//!    受講者が次に操作すれば「達成」は消える。
//!
//! 既知の限界: `processing` を持つ確認省略の Step は、進行の間「受付済み（確認待ち）」が出て、最後の進行で
//! 「達成」になる。語は契約の版 1 の固定語であり、`Entered`・`NotStarted`・`Skipped` の語は未決のため語なしのまま。

use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;

/// 表示する Step の状態。`Accepted`・`Achieved` 以外を返さない（語を持つのはこの 2 つだけ）。
pub fn step_status(session: Option<&Session>) -> Option<StepStatus> {
    let s = session?;
    let state = s.state();
    let status_of = |uid: &StepUid| {
        state
            .run
            .slices
            .get(&uid.root())
            .and_then(|sl| sl.status.get(uid))
    };
    let current = state.run.current.as_ref()?;
    match status_of(current) {
        Some(StepStatus::Accepted) => return Some(StepStatus::Accepted),
        Some(StepStatus::Achieved) => return Some(StepStatus::Achieved),
        _ => {}
    }
    for ev in s.log().iter().rev() {
        match ev {
            Event::Entered { .. }
            | Event::Transitioned { .. }
            | Event::Changed { .. }
            | Event::Flag { .. }
            | Event::ModuleEntered { .. }
            | Event::ModuleExited { .. }
            | Event::ChapterCompleted { .. }
            | Event::Warning { .. }
            | Event::Ignored { .. } => {}
            Event::Achieved { at, .. } => {
                return (status_of(&at.step) == Some(&StepStatus::Achieved))
                    .then_some(StepStatus::Achieved);
            }
            _ => return None,
        }
    }
    None
}
