//! TASK-204 2/3 の公開パス・版・書式検査・世代の規則の表・直列化の試験。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_core::addr::{Generation, Root, StepUid, StepUidError, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, GenerationRule, Input, Op};
use fandhe_workshop_core::projection::{
    LogRow, Mark, MarkKind, Projection, ReviewChange, ReviewItem,
};
use fandhe_workshop_core::provider::StateChangeProvider;
use fandhe_workshop_core::session::{
    Busy, Checkpoint, Engine, EntryCopy, RunState, SessionState, Slice,
};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir, Session, CONTRACT_VERSION};

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

#[test]
fn contract_version_is_two() {
    assert_eq!(CONTRACT_VERSION, 3);
}

#[test]
fn step_uid_parse_accepts_valid() {
    for s in [
        "main/st-1",
        "issue-to-merge@round1/st-7-2",
        "impl-run@round1/merge-gate@gate/st-7-4",
        "a@x/b@y/c@z/st",
    ] {
        assert_eq!(StepUid::parse(s), Ok(uid(s)));
    }
}

#[test]
fn step_uid_parse_rejects_with_kind() {
    let long = format!("main/{}", "a".repeat(100_000));
    let cases: Vec<(String, StepUidError)> = vec![
        ("".into(), StepUidError::Empty),
        (long, StepUidError::TooLong),
        ("main/a\u{0}b".into(), StepUidError::InvalidChar),
        ("main/a b".into(), StepUidError::InvalidChar),
        ("main".into(), StepUidError::SlashCount),
        ("main/a/b".into(), StepUidError::BadRoot),
        ("a@x/plain/st".into(), StepUidError::BadRoot),
        ("a@x//st".into(), StepUidError::BadRoot),
        ("/st".into(), StepUidError::EmptyPart),
        ("main/".into(), StepUidError::EmptyPart),
        ("a@b@c/st".into(), StepUidError::BadRoot),
        ("@b/st".into(), StepUidError::BadRoot),
        ("a@/st".into(), StepUidError::BadRoot),
        ("plain/st".into(), StepUidError::BadRoot),
    ];
    for (s, kind) in cases {
        assert_eq!(StepUid::parse(&s), Err(kind), "input len {}", s.len());
    }
}

#[test]
fn step_uid_error_message_has_no_input() {
    let e = StepUid::parse("secret value").expect_err("rejected");
    assert!(!e.to_string().contains("secret"));
}

#[test]
fn step_uid_root() {
    assert_eq!(
        uid("issue-to-merge@round1/st-7-2").root(),
        Root("issue-to-merge@round1".to_string())
    );
    assert_eq!(
        uid("impl-run@round1/merge-gate@gate/st-7-4").root(),
        Root("impl-run@round1/merge-gate@gate".to_string())
    );
    assert_eq!(uid("main/st-1").root(), Root("main".to_string()));
    assert_eq!(uid("noslash").root(), Root("noslash".to_string()));
}

#[test]
fn testdata_uids_pass_parse() {
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let files = [
        "engine-event-consistency/test-data/min-3step.json",
        "input-field-ime/test-data/ch1-pack.json",
        "input-field-ime/test-data/ime-pack.json",
    ];
    for f in files {
        let bytes = std::fs::read(base.join(f)).expect("read testdata");
        let ir = load_ir(&bytes).expect("load ir");
        assert!(!ir.nodes.is_empty());
        for n in &ir.nodes {
            assert!(StepUid::parse(&n.uid).is_ok(), "{f}: bad uid {}", n.uid);
        }
    }
}

#[test]
fn generation_rule_table() {
    let t = TickAddr {
        generation: Generation(1),
        step: uid("main/a"),
        index: 0,
    };
    let cases = [
        (Op::Input(Input::Command("x".into())), GenerationRule::Keep),
        (Op::Input(Input::Cancel), GenerationRule::Keep),
        (Op::Tick(t), GenerationRule::Keep),
        (
            Op::Control(Control::Stop),
            GenerationRule::IncrementIfProcessing,
        ),
        (
            Op::Control(Control::GoTo(uid("main/a"))),
            GenerationRule::IncrementOnSuccess,
        ),
        (Op::Control(Control::Save), GenerationRule::Keep),
        (
            Op::Control(Control::Restore(CheckpointId(1))),
            GenerationRule::IncrementOnSuccess,
        ),
        (
            Op::Control(Control::Begin("s".into())),
            GenerationRule::IncrementOnSuccess,
        ),
        (Op::Control(Control::OpenHint), GenerationRule::Keep),
        (
            Op::Control(Control::SetGuidance(GuidanceLevel::C)),
            GenerationRule::Keep,
        ),
    ];
    for (op, rule) in cases {
        assert_eq!(op.generation_rule(), rule, "{op:?}");
    }
}

#[test]
fn generation_initial_and_next() {
    assert_eq!(Generation::INITIAL, Generation(1));
    assert_eq!(Generation(1).next(), Some(Generation(2)));
    assert_eq!(Generation(u64::MAX).next(), None);
}

#[test]
fn default_state() {
    let s = SessionState::default();
    assert_eq!(s.run.generation, Generation(1));
    assert!(s.run.current.is_none());
    assert!(s.log.is_empty());
    assert!(s.checkpoints.is_empty());
    assert_eq!(s.run.guidance_level, GuidanceLevel::A);
    assert!(s.hints_opened.is_empty());
}

fn populated() -> SessionState {
    let mut slice = Slice::default();
    slice.status.insert(uid("main/a"), StepStatus::Entered);
    slice.choices.insert(uid("main/a"), "c1".into());
    slice.flags.insert("f".into());
    let mut run = RunState {
        current: Some(uid("main/a")),
        guidance_level: GuidanceLevel::C,
        busy: Some(Busy { done: 1, total: 3 }),
        ..RunState::default()
    };
    run.slices.insert(Root("main".into()), slice);
    run.global_flags.insert("g".into());
    let entry = EntryCopy {
        run: run.clone(),
        log_len: 0,
    };
    let cp = Checkpoint {
        run: run.clone(),
        entry: Some(entry.clone()),
        log_len: 0,
        owner: Some(Root("main".into())),
    };
    let mut checkpoints = BTreeMap::new();
    checkpoints.insert(CheckpointId(1), cp);
    let mut drafts = BTreeMap::new();
    drafts.insert(Root("main".into()), "draft".to_string());
    let mut hints_opened = BTreeMap::new();
    hints_opened.insert(uid("main/a"), 2);
    SessionState {
        run,
        log: Vec::new(),
        entry: Some(entry),
        checkpoints,
        next_checkpoint: CheckpointId(2),
        drafts,
        hints_opened,
    }
}

#[test]
fn state_json_roundtrip_and_unknown_field() {
    let s = populated();
    let json = serde_json::to_string(&s).expect("serialize");
    let back: SessionState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, s);
    assert_eq!(json, serde_json::to_string(&back).expect("again"));

    let mut v: serde_json::Value = serde_json::from_str(&json).expect("value");
    v.as_object_mut()
        .expect("object")
        .insert("extra".into(), serde_json::Value::Null);
    assert!(serde_json::from_value::<SessionState>(v).is_err());
}

fn assert_provider<P: StateChangeProvider<State = SessionState, Input = Op, Event = Event>>() {}

#[test]
fn engine_is_a_provider() {
    assert_provider::<Engine<'static>>();
}

#[test]
fn projection_and_types_are_constructible() {
    let p = Projection::new();
    assert_eq!(p.consumed(), 0);
    assert!(p.log().is_empty() && p.marks().is_empty() && p.review().is_empty());
    let _ = Mark {
        path: "p".into(),
        kind: MarkKind::Put,
    };
    let _ = ReviewItem {
        step: uid("main/a"),
        changes: vec![ReviewChange {
            path: "p".into(),
            before: None,
            after: None,
        }],
        achieved: false,
        skipped: false,
    };
    let _: Option<LogRow> = None;
    let _: BTreeSet<String> = BTreeSet::new();
    let _ = (apply as fn(_, _, _) -> _, Session::new);
}
