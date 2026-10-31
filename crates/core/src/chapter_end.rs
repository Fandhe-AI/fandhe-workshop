//! 章の終わりの判定と、章の終わりのチェックポイントの自動保存の要否（SAVE-31・SHELL-10・TASK-339 1/2）。
//!
//! 呼び出し元は [`crate::session::apply`]（自動保存の要否 `auto_save_due`）と web の `export_notice`
//! （書き出しの案内の時機。SAVE-29）である。両者が同じ判定を使うことで、案内と自動保存が同じイベントで起きる。
//! `Event::ChapterCompleted` は core がまだ出さない（TASK-233 以降）ため、章の終わりは
//! 「章（`main` の根）の最後の Step（`is_final`）の達成、またはモジュールからその Step への復帰（`ModuleExited`）」でも判定する。
//!
//! 試験は `tests/chapter_end_autosave.rs`（core のソースは `std::fs` を使えない。`ir_only`）。
//!
//! 不変条件: 純関数で全域（panic しない）。IR に無い Step は章の終わりではない（fail-closed）。

use fandhe_workshop_contract::ir::Ir;

use crate::event::Event;

/// `uid` が `main` の根の最後の Step（`is_final`）か。IR に無い Step は偽（fail-closed）。
pub fn is_main_final(ir: &Ir, uid: &str) -> bool {
    uid.starts_with("main/")
        && ir
            .nodes
            .iter()
            .find(|n| n.uid == uid)
            .is_some_and(|n| n.step.is_final)
}

/// 1 件のイベントが章の終わりか。`ChapterCompleted`、`main` の根の最後の Step の `Achieved`、または
/// 学習モジュールから戻った先がその Step の `ModuleExited`。
/// モジュールからの復帰では core が呼び出し元の `Achieved` を出さず `ModuleExited` だけを出すため、
/// 最後の Step がモジュールを呼ぶ章でも章の終わりを検出できるようにする。
/// IR に無い Step は偽（fail-closed）。学習モジュールの最後の Step は呼び出し元へ戻るため数えない。
pub fn is_chapter_end(ir: &Ir, ev: &Event) -> bool {
    match ev {
        Event::ChapterCompleted { .. } => true,
        Event::Achieved { at, .. } | Event::ModuleExited { at, .. } => {
            is_main_final(ir, at.step.0.as_str())
        }
        _ => false,
    }
}

/// イベントの列のどれかが章の終わりか。
pub fn chapter_end_in(ir: &Ir, events: &[Event]) -> bool {
    events.iter().any(|e| is_chapter_end(ir, e))
}

/// 操作 1 件のイベント列に対して、章の終わりのチェックポイントを自動で保存するか。
///
/// 章の終わりを含み、かつ上限で止めて前の状態へ戻した操作（`TransitionLimitReached`）ではないときだけ真。
/// 戻した操作では章の終わりの状態が残っていないため、保存しない。
pub(crate) fn auto_save_due(ir: &Ir, events: &[Event]) -> bool {
    chapter_end_in(ir, events)
        && !events
            .iter()
            .any(|e| matches!(e, Event::TransitionLimitReached { .. }))
}
