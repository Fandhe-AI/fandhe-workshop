//! 履歴の包みの組み立てと下書きの同期の試験（TASK-315 2/2、SAVE-2・SAVE-18・SAVE-24、MOD-4）。
//!
//! サンプル（`testdata/contract/history/`）を `build_envelope_with` で組み立て直してバイト一致を見ること、
//! 下書き・送信の記録と写しの同期、`start` の検査と失敗の伝播、チェックポイントの持ち主の保持を確かめる。
//! 変異注入 `mutations/SAVE24-5`〜`SAVE24-9` が検出する。呼び出し元は CI の `make test` のみである。
//! 本文は長いので、比較は真偽で行い失敗の出力に全文を出さない。

mod common;

use std::collections::BTreeMap;
use std::path::PathBuf;

use common::history_sample::{
    check_log_positions, check_roundtrip, load, reference_export, reference_import, SAMPLES,
};
use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::history_version::Version3;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::MAX_INPUT_CODE_POINTS;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::history_envelope::{
    build_envelope, build_envelope_with, record_draft, record_submit, BuildError,
};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::{apply_recorded, OpsRecorder};
use fandhe_workshop_core::session::{Checkpoint, RunState, SessionState};
use fandhe_workshop_core::version_record::{record_versions_with, RecordError, ENGINE_VERSION};
use proptest::collection::vec;
use proptest::prelude::*;

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn import(bytes: &[u8]) -> Result<common::history_sample::H, String> {
    reference_import(bytes).map_err(|e| e.to_string())
}

fn at(step: &str) -> SessionState {
    let mut s = SessionState::default();
    s.run.current = Some(StepUid(step.to_string()));
    s
}

fn main_root() -> Root {
    Root("main".to_string())
}

#[test]
fn samples_are_rebuilt_byte_for_byte() {
    let ir = base_ir();
    let hash = content_hash(&ir).unwrap();
    for name in SAMPLES {
        let golden = common::history_sample::read_bytes(name);
        let h = load(name);
        let engine = Version3::parse(&h.versions.engine).unwrap();
        let versions = record_versions_with(&ir, engine, &hash).unwrap();
        let rebuilt = build_envelope_with(&ir, versions, &h.start, h.ops, h.snapshot).unwrap();
        assert!(reference_export(&rebuilt) == golden, "{name}");
    }
}

#[test]
fn build_envelope_writes_the_current_engine_version() {
    let ir = base_ir();
    let env = build_envelope(&ir, "s-1", Vec::new(), SessionState::default()).unwrap();
    assert_eq!(env.versions.engine, ENGINE_VERSION);
    assert_eq!(env.pack_id, ir.pack.id);
    assert_eq!(env.start, "s-1");
    assert_eq!(
        env.versions.content_hash,
        content_hash(&ir).unwrap().as_str()
    );
}

#[test]
fn save24_example_shape_is_in_the_envelope() {
    let ir = base_ir();
    let mut state = at("main/st-1");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "/help".into());
    assert!(record_submit(&mut state, &mut rec, "/help".into()));
    record_draft(&mut state, &mut rec, "未送信のメモ".into());
    let env = build_envelope(&ir, "s-1", rec.into_ops(), state).unwrap();
    let json = serde_json::to_value(&env.ops).unwrap();
    assert_eq!(
        json,
        serde_json::json!([{"draft": "/help"}, {"submit": "/help"}, {"draft": "未送信のメモ"}])
    );
    assert_eq!(
        env.snapshot.drafts.get(&main_root()).map(String::as_str),
        Some("未送信のメモ")
    );
}

#[test]
fn max_length_submit_stays_whole_in_the_envelope() {
    let ir = base_ir();
    let text = "あ".repeat(MAX_INPUT_CODE_POINTS);
    let mut state = at("main/st-1");
    let mut rec = OpsRecorder::new();
    assert!(record_submit(&mut state, &mut rec, text.clone()));
    let env = build_envelope(&ir, "s-1", rec.into_ops(), state).unwrap();
    assert_eq!(env.ops.len(), 1);
    let kept = matches!(&env.ops[0], HistoryOp::Submit(t) if *t == text);
    assert!(kept, "submit body must stay whole");
}

#[test]
fn submit_clears_only_the_current_root_draft() {
    let mut state = at("main/st-1");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "a".into());
    state.run.current = Some(StepUid("other/st-1".into()));
    record_draft(&mut state, &mut rec, "b".into());
    assert!(record_submit(&mut state, &mut rec, "b".into()));
    assert_eq!(
        state.drafts.get(&main_root()).map(String::as_str),
        Some("a")
    );
    assert!(!state.drafts.contains_key(&Root("other".into())));
}

#[test]
fn nested_instance_draft_root_is_its_instance_id() {
    // 期待値の根は文字列で直に書く（`StepUid::root` で作ると自己参照になる）。
    let mut state = at("impl-run@round1/merge-gate@gate/st-7-4");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "child".into());
    state.run.current = Some(StepUid("impl-run@round1/st-7-3".into()));
    record_draft(&mut state, &mut rec, "parent".into());
    state.run.current = Some(StepUid("impl-run@round2/merge-gate@gate/st-7-4".into()));
    record_draft(&mut state, &mut rec, "round2".into());
    let get = |r: &str| state.drafts.get(&Root(r.to_string())).map(String::as_str);
    assert_eq!(state.drafts.len(), 3);
    assert_eq!(get("impl-run@round1/merge-gate@gate"), Some("child"));
    assert_eq!(get("impl-run@round1"), Some("parent"));
    assert_eq!(get("impl-run@round2/merge-gate@gate"), Some("round2"));
}

#[test]
fn nested_submit_clears_only_its_own_root() {
    let mut state = at("impl-run@round1/st-7-3");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "parent".into());
    state.run.current = Some(StepUid("impl-run@round2/merge-gate@gate/st-7-4".into()));
    record_draft(&mut state, &mut rec, "round2".into());
    state.run.current = Some(StepUid("impl-run@round1/merge-gate@gate/st-7-4".into()));
    record_draft(&mut state, &mut rec, "child".into());
    assert!(record_submit(&mut state, &mut rec, "child".into()));
    let get = |r: &str| state.drafts.get(&Root(r.to_string())).map(String::as_str);
    assert_eq!(get("impl-run@round1/merge-gate@gate"), None);
    assert_eq!(get("impl-run@round1"), Some("parent"));
    assert_eq!(get("impl-run@round2/merge-gate@gate"), Some("round2"));
}

#[test]
fn no_current_step_records_ops_only() {
    let mut state = SessionState::default();
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "x".into());
    assert!(record_submit(&mut state, &mut rec, "x".into()));
    assert_eq!(rec.len(), 2);
    assert!(state.drafts.is_empty());
}

#[test]
fn whitespace_submit_changes_nothing() {
    let mut state = at("main/st-1");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, "keep".into());
    assert!(!record_submit(&mut state, &mut rec, "  \u{3000}".into()));
    assert_eq!(rec.len(), 1);
    assert_eq!(
        state.drafts.get(&main_root()).map(String::as_str),
        Some("keep")
    );
}

#[test]
fn empty_draft_is_kept_as_empty_string() {
    let mut state = at("main/st-1");
    let mut rec = OpsRecorder::new();
    record_draft(&mut state, &mut rec, String::new());
    assert_eq!(state.drafts.get(&main_root()).map(String::as_str), Some(""));
}

#[test]
fn unknown_start_is_rejected() {
    let ir = base_ir();
    let r = build_envelope(&ir, "no-such-start", Vec::new(), SessionState::default());
    assert_eq!(r.unwrap_err(), BuildError::UnknownStart);
}

#[test]
fn version_failure_is_returned_not_defaulted() {
    let mut ir = base_ir();
    ir.pack.ir_format += 1;
    let r = build_envelope(&ir, "s-1", Vec::new(), SessionState::default());
    assert_eq!(
        r.unwrap_err(),
        BuildError::Versions(RecordError::IrFormatMismatch)
    );
}

#[test]
fn checkpoint_owner_is_kept() {
    let ir = base_ir();
    let mut state = SessionState::default();
    for (n, owner) in [(1u32, Some(main_root())), (2, None)] {
        state.checkpoints.insert(
            CheckpointId(n),
            Checkpoint {
                run: RunState::default(),
                entry: None,
                log_len: 0,
                owner,
            },
        );
    }
    state.next_checkpoint = CheckpointId(3);
    let env = build_envelope(&ir, "s-1", Vec::new(), state.clone()).unwrap();
    assert!(env.snapshot.checkpoints == state.checkpoints);
    assert_eq!(
        env.snapshot.checkpoints[&CheckpointId(1)].owner,
        Some(main_root())
    );
    assert_eq!(env.snapshot.checkpoints[&CheckpointId(2)].owner, None);
}

#[test]
fn sample_rebuild_roundtrips_and_keeps_positions() {
    let ir = base_ir();
    let hash = content_hash(&ir).unwrap();
    for name in SAMPLES {
        let h = load(name);
        let engine = Version3::parse(&h.versions.engine).unwrap();
        let versions = record_versions_with(&ir, engine, &hash).unwrap();
        let rebuilt =
            build_envelope_with(&ir, versions, &h.start, h.ops.clone(), h.snapshot.clone())
                .unwrap();
        check_roundtrip(reference_export, import, &rebuilt)
            .unwrap_or_else(|e| panic!("{name}: {e}"));
        check_log_positions(&rebuilt.snapshot).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(rebuilt.snapshot.log == h.snapshot.log, "{name}");
        assert!(rebuilt == h, "{name}");
    }
}

#[derive(Debug, Clone)]
enum Step {
    Draft(String),
    Submit(String),
    Save,
    Move(bool),
}

fn step_strategy() -> impl Strategy<Value = Step> {
    prop_oneof![
        "[a-z ]{0,6}".prop_map(Step::Draft),
        "[a-z ]{0,6}".prop_map(Step::Submit),
        Just(Step::Save),
        any::<bool>().prop_map(Step::Move),
    ]
}

#[test]
fn property_ops_and_drafts_stay_in_sync() {
    let ir = base_ir();
    check(
        "history-envelope-drafts-sync",
        DEFAULT_CASES,
        vec(step_strategy(), 0..MAX_OPS),
        |steps| {
            let mut state = at("main/st-1");
            let mut rec = OpsRecorder::new();
            let mut model: BTreeMap<Root, String> = BTreeMap::new();
            for s in steps {
                let root = state.run.current.as_ref().map(StepUid::root).unwrap();
                match s {
                    Step::Draft(t) => {
                        model.insert(root, t.clone());
                        record_draft(&mut state, &mut rec, t);
                    }
                    Step::Submit(t) => {
                        if !t.trim().is_empty() {
                            model.remove(&root);
                        }
                        record_submit(&mut state, &mut rec, t);
                    }
                    Step::Save => {
                        let (next, _) =
                            apply_recorded(&ir, state, &mut rec, Op::Control(Control::Save));
                        state = next;
                    }
                    Step::Move(b) => {
                        let step = if b { "main/st-1" } else { "other/st-1" };
                        state.run.current = Some(StepUid(step.to_string()));
                    }
                }
            }
            let expected = rec.ops().to_vec();
            let env = build_envelope(&ir, "s-1", rec.into_ops(), state).unwrap();
            prop_assert!(env.ops == expected, "ops differ from the recorder");
            prop_assert!(env.snapshot.drafts == model, "drafts differ from the model");
            Ok(())
        },
    );
}
