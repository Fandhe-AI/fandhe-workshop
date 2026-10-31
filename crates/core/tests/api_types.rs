//! TASK-204 1/3 の型の公開パスと直列化の形の試験。

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid, TickAddr};
use fandhe_workshop_core::event::{Cause, ControlKind, Event, IgnoreReason, RefuseReason};
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use serde::{de::DeserializeOwned, Serialize};

fn stamp() -> Stamp {
    Stamp {
        generation: Generation(3),
        step: StepUid("main/s1".to_string()),
    }
}

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(v: &T) {
    let a = serde_json::to_string(v).expect("serialize");
    let back: T = serde_json::from_str(&a).expect("deserialize");
    assert_eq!(&back, v);
    assert_eq!(a, serde_json::to_string(&back).expect("serialize again"));
}

#[test]
fn step_status_values_and_default() {
    let cases = [
        (StepStatus::NotStarted, "\"not_started\""),
        (StepStatus::Entered, "\"entered\""),
        (StepStatus::Accepted, "\"accepted\""),
        (StepStatus::Achieved, "\"achieved\""),
        (StepStatus::Skipped, "\"skipped\""),
    ];
    for (v, s) in cases {
        assert_eq!(serde_json::to_string(&v).expect("serialize"), s);
        roundtrip(&v);
    }
    assert_eq!(StepStatus::default(), StepStatus::NotStarted);
}

#[test]
fn tick_addr_has_three_elements() {
    let t = TickAddr {
        generation: Generation(1),
        step: StepUid("main/s1".to_string()),
        index: 2,
    };
    let v = serde_json::to_value(&t).expect("serialize");
    let obj = v.as_object().expect("object");
    assert_eq!(obj.len(), 3);
    for k in ["generation", "step", "index"] {
        assert!(obj.contains_key(k), "missing {k}");
    }
    roundtrip(&t);
}

#[test]
fn ops_roundtrip() {
    let ops = [
        Op::Input(Input::Command("run".to_string())),
        Op::Input(Input::Cancel),
        Op::Input(Input::Approval {
            decision: Decision::Revise,
            target: None,
        }),
        Op::Tick(TickAddr {
            generation: Generation(0),
            step: StepUid("m@a/s".to_string()),
            index: 0,
        }),
        Op::Control(Control::Stop),
        Op::Control(Control::GoTo(StepUid("main/s2".to_string()))),
        Op::Control(Control::Save),
        Op::Control(Control::Restore(CheckpointId(1))),
        Op::Control(Control::Begin("start".to_string())),
        Op::Control(Control::OpenHint),
        Op::Control(Control::SetGuidance(GuidanceLevel::C)),
    ];
    for op in &ops {
        roundtrip(op);
    }
}

#[test]
fn events_roundtrip() {
    let mut el = Element::default();
    el.attrs.insert("b".to_string(), "2".to_string());
    el.attrs.insert("a".to_string(), "1".to_string());
    let events = [
        Event::Changed {
            at: stamp(),
            path: "x/y".to_string(),
            before: None,
            after: Some(el),
            cause: Cause::Input,
        },
        Event::Restored {
            at: stamp(),
            id: CheckpointId(1),
            new_generation: Generation(4),
            log_len: 2,
            owner: StepUid("main/s1".to_string()),
        },
        Event::Skipped { at: stamp() },
        Event::Refused {
            at: stamp(),
            op: ControlKind::GoTo,
            reason: RefuseReason::UnknownStep,
            rejection: None,
        },
        Event::Ignored {
            at: stamp(),
            reason: IgnoreReason::Stale,
        },
        Event::Warning {
            at: stamp(),
            message: "m".to_string(),
        },
        Event::Stopped {
            at: stamp(),
            new_generation: Generation(4),
        },
        Event::Moved {
            at: stamp(),
            to: StepUid("main/s2".to_string()),
            new_generation: Generation(4),
        },
        Event::Begun {
            at: stamp(),
            start: "start".to_string(),
            new_generation: Generation(4),
        },
        Event::HintOpened {
            at: stamp(),
            opened: 1,
            total: 2,
        },
        Event::Reloaded {
            at: stamp(),
            new_generation: Generation(4),
        },
        Event::TransitionLimitReached {
            at: stamp(),
            landings: 1000,
        },
        Event::Refused {
            at: stamp(),
            op: ControlKind::OpenHint,
            reason: RefuseReason::NoHint,
            rejection: None,
        },
    ];
    for e in &events {
        roundtrip(e);
    }
    let v = serde_json::to_value(&events[6]).expect("serialize");
    assert!(v["stopped"]["new_generation"].is_number());
    let reloaded = events
        .iter()
        .find(|e| matches!(e, Event::Reloaded { .. }))
        .expect("reloaded");
    let v = serde_json::to_value(reloaded).expect("serialize");
    assert!(v["reloaded"]["new_generation"].is_number());
    let limit = events
        .iter()
        .find(|e| matches!(e, Event::TransitionLimitReached { .. }))
        .expect("transition_limit_reached");
    let v = serde_json::to_value(limit).expect("serialize");
    assert_eq!(v["transition_limit_reached"]["landings"], 1000);
}

#[test]
fn unknown_field_and_variant_are_rejected() {
    let bad_field = r#"{"generation":1,"step":"s","index":0,"extra":1}"#;
    assert!(serde_json::from_str::<TickAddr>(bad_field).is_err());
    assert!(serde_json::from_str::<StepStatus>("\"unknown\"").is_err());
    assert!(serde_json::from_str::<Op>(r#"{"nope":1}"#).is_err());
    assert!(serde_json::from_str::<Event>(r#"{"nope":{}}"#).is_err());
    // 構造体 variant の余分なキーも拒否する（履歴の ops・保存イベントは信頼できない入力）。
    let bad_event =
        r#"{"stopped":{"at":{"generation":1,"step":"s"},"new_generation":1,"extra":1}}"#;
    assert!(serde_json::from_str::<Event>(bad_event).is_err());
    let ok_event = bad_event.replace(r#","extra":1"#, "");
    assert!(serde_json::from_str::<Event>(&ok_event).is_ok());
    let bad_reloaded =
        r#"{"reloaded":{"at":{"generation":1,"step":"s"},"new_generation":2,"extra":1}}"#;
    assert!(serde_json::from_str::<Event>(bad_reloaded).is_err());
    let bad_limit = r#"{"transition_limit_reached":{"at":{"generation":1,"step":"s"},"landings":1000,"extra":1}}"#;
    assert!(serde_json::from_str::<Event>(bad_limit).is_err());
    let ok_limit = bad_limit.replace(r#","extra":1"#, "");
    assert!(serde_json::from_str::<Event>(&ok_limit).is_ok());
    let bad_ui = r#"{"ui":{"view":"v","path":"p","extra":1}}"#;
    assert!(serde_json::from_str::<Input>(bad_ui).is_err());
    let bad_approval = r#"{"approval":{"decision":"approve","target":null,"extra":1}}"#;
    assert!(serde_json::from_str::<Input>(bad_approval).is_err());
}

#[test]
fn judged_event_shape_and_roundtrip_for_all_six_kinds() {
    use fandhe_workshop_contract::judgment::{
        Clarification, Judgment, JudgmentKind, NextAction, RejectCategory, Rejection,
    };
    let na = |s: &str| NextAction::new(s).expect("non-empty");
    let judgments = [
        Judgment::Accepted(None),
        Judgment::Rejected(Rejection {
            category: RejectCategory::Busy,
            text: "rj-default-busy".to_string(),
            next_action: na("待つ"),
        }),
        Judgment::Clarified(Clarification {
            text: "cl-x".to_string(),
            next_action: na("答える"),
        }),
        Judgment::Confirmed,
        Judgment::Progressed,
        Judgment::Ignored,
    ];
    let kinds: Vec<JudgmentKind> = judgments.iter().map(Judgment::kind).collect();
    assert_eq!(kinds, JudgmentKind::ALL);
    for judgment in judgments {
        roundtrip(&Event::Judged {
            at: stamp(),
            judgment,
        });
    }
    let v = serde_json::to_value(Event::Judged {
        at: stamp(),
        judgment: Judgment::Confirmed,
    })
    .expect("serialize");
    assert_eq!(
        v,
        serde_json::json!({"judged": {"at": {"generation": 3, "step": "main/s1"}, "judgment": "confirmed"}})
    );
    // 未知の欄は拒否する。
    let bad = r#"{"judged":{"at":{"generation":3,"step":"main/s1"},"judgment":"confirmed","x":1}}"#;
    assert!(serde_json::from_str::<Event>(bad).is_err());
}

#[test]
fn closed_round_reason_serializes_and_has_a_next_action() {
    use fandhe_workshop_core::event::RefuseNextAction;
    let v = serde_json::to_value(RefuseReason::ClosedRound).expect("serialize");
    assert_eq!(v, serde_json::json!("closed_round"));
    assert_eq!(
        RefuseReason::ClosedRound.next_action(),
        Some(RefuseNextAction::ContinueCurrentRound)
    );
    assert_eq!(RefuseReason::NoHint.next_action(), None);
    for a in RefuseNextAction::ALL {
        assert!(!a.as_str().is_empty() && a.as_str().is_ascii());
    }
}
