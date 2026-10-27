//! `line_kind_from_event`（複製をしない判定）が `line_from_event` の種類と必ず一致することの試験（TASK-232 2/3。FEND-22）。
//!
//! 右のログは畳まれる行の `LogLine` を作らず、この判定で行を数える。判定がずれると、畳みの計画と実際の行が食い違うため、
//! 行にならないイベント・文面の有無・進行の境界を含めて、2 つの関数の結果を突き合わせる。
//! 試験では `unwrap` を使ってよい。

use fandhe_workshop_contract::judgment::{
    Clarification, Judgment, NextAction, RejectCategory, Rejection,
};
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input};
use fandhe_workshop_web::log_lines::{line_from_event, line_kind_from_event};

fn stamp() -> Stamp {
    Stamp {
        generation: Generation(1),
        step: StepUid("main/st-1".to_owned()),
    }
}

fn na() -> NextAction {
    NextAction::new("ls を打つ").unwrap()
}

fn events() -> Vec<Event> {
    let at = stamp;
    let mut v = vec![
        Event::Judged {
            at: at(),
            judgment: Judgment::Rejected(Rejection {
                category: RejectCategory::UnknownCommand,
                text: "r-1".into(),
                next_action: na(),
            }),
        },
        Event::Judged {
            at: at(),
            judgment: Judgment::Clarified(Clarification {
                text: "c-1".into(),
                next_action: na(),
            }),
        },
        Event::Stopped {
            at: at(),
            new_generation: Generation(2),
        },
        Event::Reloaded {
            at: at(),
            new_generation: Generation(2),
        },
        Event::TransitionLimitReached {
            at: at(),
            landings: 1000,
        },
        Event::Accepted { at: at() },
        Event::Entered { at: at() },
    ];
    for (index, total) in [(0, 2), (1, 2), (2, 2), (3, 2), (0, 0)] {
        v.push(Event::Progress {
            at: at(),
            name: "n".into(),
            index,
            total,
        });
    }
    for input in [
        Input::Command("ls".into()),
        Input::Text("t".into()),
        Input::Choice("c".into()),
        Input::Cancel,
        Input::Ui {
            view: "v".into(),
            path: "p".into(),
        },
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
        Input::Approval {
            decision: Decision::Reject,
            target: Some("x".into()),
        },
    ] {
        v.push(Event::InputReceived { at: at(), input });
    }
    v
}

#[test]
fn kind_matches_line_for_every_event_and_text() {
    let texts: [Option<&str>; 5] = [None, Some(""), Some("  \t"), Some("理由"), Some(" 理 ")];
    let mut some = 0;
    for e in events() {
        for t in texts {
            let expected = line_from_event(&e, t).map(|l| l.kind());
            assert_eq!(line_kind_from_event(&e, t), expected, "{e:?} / {t:?}");
            some += usize::from(expected.is_some());
        }
    }
    assert!(some > 20, "行になる組が少なすぎる: {some}");
}
