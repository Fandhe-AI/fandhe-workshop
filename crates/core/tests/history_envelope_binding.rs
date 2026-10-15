//! core の `Op`・`SessionState` を `HistoryEnvelope` に束縛したときの形の検査（TASK-309 2/3）。
//!
//! contract は core に依存しないため、束縛は使う側（ここ）で行う。写しが包んでも形を変えないこと、
//! ログの並びと位置（`log_len`）が往復で変わらないこと、`snapshot` の欠落・未知の欄を拒否することを確かめる。

use std::collections::BTreeMap;

use fandhe_workshop_contract::history::{HistoryEnvelope, HistoryOp};
use fandhe_workshop_contract::history_version::HistoryVersions;
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Input, Op};
use fandhe_workshop_core::session::{Checkpoint, EntryCopy, RunState, SessionState};
use serde_json::Value;

type H = HistoryEnvelope<Op, SessionState>;

fn stamp(step: &str) -> Stamp {
    Stamp {
        generation: Generation(1),
        step: StepUid(step.to_string()),
    }
}

fn state() -> SessionState {
    let run = RunState::default();
    let entry = EntryCopy {
        run: run.clone(),
        log_len: 1,
    };
    let cp = Checkpoint {
        run: run.clone(),
        entry: Some(entry.clone()),
        log_len: 2,
        owner: Some(Root("main".into())),
    };
    let mut checkpoints = BTreeMap::new();
    checkpoints.insert(CheckpointId(1), cp);
    let mut drafts = BTreeMap::new();
    drafts.insert(Root("main".into()), "未送信".to_string());
    SessionState {
        run,
        log: vec![
            Event::Entered {
                at: stamp("main/b"),
            },
            Event::Entered {
                at: stamp("main/a"),
            },
            Event::CheckpointSaved {
                at: stamp("main/a"),
                id: CheckpointId(1),
                log_len: 2,
            },
            Event::Entered {
                at: stamp("main/c"),
            },
            Event::Entered {
                at: stamp("main/a"),
            },
        ],
        entry: Some(entry),
        checkpoints,
        next_checkpoint: CheckpointId(2),
        drafts,
        hints_opened: BTreeMap::new(),
    }
}

fn envelope() -> H {
    HistoryEnvelope::new(
        "p".into(),
        HistoryVersions {
            engine: "0.1.0".into(),
            ir_format: 1,
            vocab: "1.0".into(),
            content: "0.1.0".into(),
            content_hash: "0".repeat(64),
            refs: BTreeMap::new(),
        },
        "main".into(),
        vec![
            HistoryOp::Engine(Op::Input(Input::Command("ls".into()))),
            HistoryOp::Draft("未送信".into()),
            HistoryOp::Engine(Op::Control(Control::Save)),
        ],
        state(),
    )
}

#[test]
fn roundtrip_and_snapshot_form_unchanged() {
    let e = envelope();
    let s = serde_json::to_string(&e).unwrap();
    let back: H = serde_json::from_str(&s).unwrap();
    assert_eq!(back, e);
    assert_eq!(serde_json::to_string(&back).unwrap(), s);
    let v: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(v["snapshot"], serde_json::to_value(state()).unwrap());
}

#[test]
fn log_order_and_positions_survive_roundtrip() {
    let e = envelope();
    let back: H = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
    assert_eq!(back.snapshot.log, e.snapshot.log);
    assert!(matches!(
        back.snapshot.log[2],
        Event::CheckpointSaved { log_len: 2, .. }
    ));
    let cp = &back.snapshot.checkpoints[&CheckpointId(1)];
    assert_eq!(cp.log_len, 2);
    assert_eq!(back.snapshot.entry.as_ref().unwrap().log_len, 1);
}

#[test]
fn snapshot_has_the_seven_elements() {
    let v = serde_json::to_value(envelope()).unwrap();
    let snap = v["snapshot"].as_object().unwrap();
    let mut keys: Vec<&str> = snap.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "checkpoints",
            "drafts",
            "entry",
            "hints_opened",
            "log",
            "next_checkpoint",
            "run"
        ]
    );
    assert!(snap["run"].get("generation").is_some());
    assert!(snap["checkpoints"]["1"].get("owner").is_some());
}

#[test]
fn missing_or_unknown_snapshot_is_rejected() {
    let mut v = serde_json::to_value(envelope()).unwrap();
    v["snapshot"]["extra"] = Value::Null;
    assert!(serde_json::from_value::<H>(v).is_err());

    let mut v = serde_json::to_value(envelope()).unwrap();
    v.as_object_mut().unwrap().remove("snapshot");
    assert!(serde_json::from_value::<H>(v).is_err());
}
