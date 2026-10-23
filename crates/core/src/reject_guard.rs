//! 拒否は状態を変えないことの関門（TASK-270、JUDGE-5）。
//!
//! 拒否された入力（`Event::Judged` が `Judgment::Rejected`）では、Step が受付済みにならず、世界状態も変わらない。
//! 現在の実装は、`accept` の拒否の分岐が `state.run` を書く前に返ることで、この性質を構造として満たす。
//! 本モジュールは、後続の変更がその構造を崩したときに、誤った状態を残さない fail-closed の関門である。
//! 呼び出し元は [`crate::session::apply`] だけで、`step` の直後・上限の戻し（SAFE-35）・世代の強制・`Changed` の導出より前に当てる。
//! 公開 API には含めない（契約の対象外）。
//!
//! # 見える形にする理由
//!
//! 違反を黙って操作前の状態へ戻すと、`accept` の誤りが状態から消え、性質テストも変異注入も検出できなくなる。
//! そのため、違反のときは状態を戻したうえで、イベント列を `InputReceived` と拒否の `Judged` だけに絞り、固定文の
//! `Warning` を 1 件足す。正しい実装では発火せず、イベント列・状態・ログの形は変わらない。
//!
//! # 戻す欄と戻さない欄
//!
//! 戻すのは `run` と `entry` である。`checkpoints`・`next_checkpoint`・`drafts`・`hints_opened` は `Op::Input` の
//! 経路が書かない欄のため、操作前の値の控えを持たず、戻さない。これらの一致は `tests/reject_invariant.rs` が確かめる。
//!
//! 不変条件: 全域（`unwrap`・`expect`・`panic!` を使わない）で決定的。I/O・時刻・乱数を持たない。
//! `Warning` の文面は固定文で、入力値・教材の値を埋め込まない。

use fandhe_workshop_contract::judgment::Judgment;

use crate::event::Event;
use crate::session::{EntryCopy, RunState, SessionState};

/// 違反を見つけたときの固定文。入力値・教材の値を含めない。
const REJECTION_CHANGED_STATE: &str = "rejected input must not change the state; changes discarded";

/// 拒否の `Judged` を含む操作の結果が不変条件を守っているかを確かめ、破っていれば戻して見える形にする。
///
/// 拒否を含まない `events` と、不変条件を守る拒否は、`events` をそのまま返して `state` に触れない。
/// 破っている（状態が操作前と違う、または `InputReceived`・`Judged` 以外のイベントがある）ときは、
/// `state.run`・`state.entry` を操作前の値へ戻し、`events` を拒否の `InputReceived`・`Judged` と固定文の `Warning` だけにする。
pub(crate) fn guard_rejection(
    events: Vec<Event>,
    state: &mut SessionState,
    run_before: &RunState,
    entry_before: &Option<EntryCopy>,
) -> Vec<Event> {
    let rejected_at = events.iter().find_map(|e| match e {
        Event::Judged {
            at,
            judgment: Judgment::Rejected(_),
        } => Some(at.clone()),
        _ => None,
    });
    let Some(at) = rejected_at else {
        return events;
    };
    let only_expected = events
        .iter()
        .all(|e| matches!(e, Event::InputReceived { .. } | Event::Judged { .. }));
    if only_expected && state.run == *run_before && state.entry == *entry_before {
        return events;
    }
    state.run = run_before.clone();
    state.entry = entry_before.clone();
    let mut kept: Vec<Event> = events
        .into_iter()
        .filter(|e| matches!(e, Event::InputReceived { .. } | Event::Judged { .. }))
        .collect();
    kept.push(Event::Warning {
        at,
        message: REJECTION_CHANGED_STATE.to_string(),
    });
    kept
}

#[cfg(test)]
mod tests {
    use fandhe_workshop_contract::judgment::{NextAction, RejectCategory, Rejection};

    use super::*;
    use crate::addr::{Generation, Stamp, StepUid};
    use crate::input::Input;
    use crate::step_status::StepStatus;

    fn stamp() -> Stamp {
        Stamp {
            generation: Generation(1),
            step: StepUid("main/st-1".to_string()),
        }
    }

    fn received() -> Event {
        Event::InputReceived {
            at: stamp(),
            input: Input::Cancel,
        }
    }

    fn rejected() -> Event {
        Event::Judged {
            at: stamp(),
            judgment: Judgment::Rejected(Rejection {
                category: RejectCategory::NotInThisStep,
                text: "r-1".to_string(),
                next_action: NextAction::new("do something").expect("non-empty"),
            }),
        }
    }

    fn state_with_status(status: StepStatus) -> SessionState {
        let mut state = SessionState::default();
        let uid = StepUid("main/st-1".to_string());
        state.run.current = Some(uid.clone());
        state
            .run
            .slices
            .entry(crate::instance::root_of(&uid))
            .or_default()
            .status
            .insert(uid, status);
        state
    }

    #[test]
    fn rejection_that_changes_nothing_passes_through() {
        let before = state_with_status(StepStatus::Entered);
        let mut state = before.clone();
        let events = vec![received(), rejected()];
        let out = guard_rejection(events.clone(), &mut state, &before.run, &before.entry);
        assert_eq!(out, events);
        assert_eq!(state, before);
    }

    #[test]
    fn events_without_a_rejection_are_not_touched() {
        let before = state_with_status(StepStatus::Entered);
        let mut state = state_with_status(StepStatus::Accepted);
        let events = vec![received(), Event::Accepted { at: stamp() }];
        let out = guard_rejection(events.clone(), &mut state, &before.run, &before.entry);
        assert_eq!(out, events);
        assert_eq!(state, state_with_status(StepStatus::Accepted));
    }

    #[test]
    fn extra_event_after_a_rejection_is_discarded_with_a_warning() {
        let before = state_with_status(StepStatus::Entered);
        let mut state = before.clone();
        let events = vec![received(), rejected(), Event::Accepted { at: stamp() }];
        let out = guard_rejection(events, &mut state, &before.run, &before.entry);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[0], Event::InputReceived { .. }));
        assert!(matches!(out[1], Event::Judged { .. }));
        assert!(
            matches!(&out[2], Event::Warning { message, .. } if message == REJECTION_CHANGED_STATE)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn state_change_after_a_rejection_is_restored_with_a_warning() {
        let before = state_with_status(StepStatus::Entered);
        let mut state = state_with_status(StepStatus::Accepted);
        let events = vec![received(), rejected()];
        let out = guard_rejection(events, &mut state, &before.run, &before.entry);
        assert_eq!(out.len(), 3);
        assert!(matches!(out[2], Event::Warning { .. }));
        assert_eq!(state.run, before.run);
        assert_eq!(state.entry, before.entry);
    }
}
