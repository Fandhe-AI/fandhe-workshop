//! 履歴の包みの契約適合試験（TASK-309 3/3、`HISTORY_CONTRACT_VERSION` = 1）。
//!
//! 呼び出し元は CI の `make test` で、サンプルは `testdata/contract/history/`（説明は同じ場所の `README.md`）。
//! 書き出し→読み込みの往復でログの並びと位置（`log_len`）が変わらないこと（SAVE-2）、Enter の送信が
//! `submit` だけで残ること（SAVE-24）、下書きが書き出しに入ること（SAVE-18）を確かめる。
//! `apply()` が骨格の間は、`ops` を適用し直した結果と `snapshot` の一致は確かめない（TASK-174・TASK-310・TASK-315 の責務）。
//!
//! 変異注入 `mutations/SAVE2-1.mutation`（PoC-6 の X-6 に当たる、読み込みでログの並びを変える誤り）は、
//! この試験の往復・バイト一致・性質テストが検出する。比較器そのものは `comparator_self_check` で自己確認する。

mod common;

use std::collections::BTreeMap;

use common::history_sample::{
    check_log_positions, check_roundtrip, load, read_bytes, reference_export, reference_import, H,
    SAMPLES,
};
use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::content_hash::content_hash;
use fandhe_workshop_contract::history::{
    HistoryOp, EXPORT_FORM, EXPORT_INCLUDES_DRAFT, HISTORY_CONTRACT_VERSION, HISTORY_FORMAT_NAME,
    OPS_RECORDS_FAST_FORWARD,
};
use fandhe_workshop_core::addr::{Generation, Root, Stamp, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{Checkpoint, EntryCopy, RunState, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;

fn import(bytes: &[u8]) -> Result<H, String> {
    reference_import(bytes).map_err(|e| e.to_string())
}

fn roundtrip_ok(h: &H) -> Result<(), String> {
    check_roundtrip(reference_export, import, h)
}

fn stamp(i: usize) -> Stamp {
    Stamp {
        generation: Generation(1),
        step: StepUid(format!("main/st-{i}")),
    }
}

#[test]
fn contract_version_is_1() {
    assert_eq!(HISTORY_CONTRACT_VERSION, 1);
    assert_eq!(HISTORY_FORMAT_NAME, "fandhe-workshop-history");
}

#[test]
fn samples_parse_with_core_types() {
    let ir_path = common::contract_sample::testdata_dir()
        .join("engine-event-consistency/test-data/min-3step.json");
    let ir = load_ir(&std::fs::read(ir_path).unwrap()).unwrap();
    let hash = content_hash(&ir).unwrap();
    for name in SAMPLES {
        let h = load(name);
        assert_eq!(h.format, HISTORY_FORMAT_NAME, "{name}");
        assert_eq!(h.history_format, HISTORY_CONTRACT_VERSION, "{name}");
        h.versions
            .parse()
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(h.versions.content_hash, hash.as_str(), "{name}");
        assert_eq!(h.versions.ir_format, 1, "{name}");
        assert_eq!(h.pack_id, "min-3step", "{name}");
    }
}

#[test]
fn samples_are_byte_identical_after_roundtrip() {
    for name in SAMPLES {
        let golden = read_bytes(name);
        let h = load(name);
        assert_eq!(reference_export(&h), golden, "{name}");
    }
}

#[test]
fn samples_roundtrip_preserves_log_order_and_positions() {
    for name in SAMPLES {
        let h = load(name);
        roundtrip_ok(&h).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn samples_satisfy_log_position_invariants() {
    for name in SAMPLES {
        check_log_positions(&load(name).snapshot).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn sample_kinds_are_as_named() {
    let empty = load("empty");
    assert!(empty.ops.is_empty());
    assert!(empty.snapshot.log.is_empty());

    let two = load("two-rounds");
    let count = |f: fn(&Event) -> bool| two.snapshot.log.iter().filter(|e| f(e)).count();
    assert_eq!(count(|e| matches!(e, Event::ChapterCompleted { .. })), 2);
    assert!(count(|e| matches!(e, Event::Begun { .. })) >= 1);
    assert!(count(|e| matches!(e, Event::Restored { .. })) >= 1);
    assert!(two.ops.iter().any(|o| matches!(o, HistoryOp::Submit(_))));
    assert!(two.ops.iter().any(|o| matches!(o, HistoryOp::Reload)));

    let dr = load("draft-rejected");
    let Some(HistoryOp::Draft(last)) = dr.ops.last() else {
        panic!("the last op must be an unsent draft");
    };
    assert_eq!(dr.snapshot.drafts.get(&Root("main".into())), Some(last));
    const { assert!(EXPORT_INCLUDES_DRAFT) };
    let log = &dr.snapshot.log;
    let rejected_by_form = log.windows(2).any(|w| {
        matches!(w[0], Event::InputReceived { .. }) && !matches!(w[1], Event::Accepted { .. })
    }) || matches!(log.last(), Some(Event::InputReceived { .. }));
    assert!(rejected_by_form, "an InputReceived without Accepted");
    assert!(log.iter().any(|e| matches!(e, Event::Refused { .. })));
}

#[test]
fn submit_is_the_only_send_record() {
    const TAGS: [&str; 6] = ["submit", "draft", "reload", "input", "tick", "control"];
    const { assert!(!OPS_RECORDS_FAST_FORWARD) };
    for name in SAMPLES {
        let v: serde_json::Value = serde_json::from_slice(&read_bytes(name)).unwrap();
        for op in v["ops"].as_array().unwrap() {
            let tag = match op {
                serde_json::Value::String(s) => s.as_str(),
                serde_json::Value::Object(m) => m.keys().next().unwrap().as_str(),
                other => panic!("{name}: unexpected op {other}"),
            };
            assert!(TAGS.contains(&tag), "{name}: tag {tag}");
        }
        assert!(v.get("snapshot").is_some(), "{name}: snapshot is required");
    }
    assert_eq!(format!("{EXPORT_FORM:?}"), "OpsAndSnapshot");
}

/// 整合の取れたログと `ops` を作る。`kinds` の各値で、保存・復帰・その他のイベントを選ぶ。
/// イベントはどれも別の Step 名を持ち、並びを入れ替えると必ず違いが出る。
fn build(kinds: &[u8]) -> H {
    let mut log: Vec<Event> = Vec::new();
    let mut ops: Vec<HistoryOp<Op>> = Vec::new();
    let mut checkpoints: BTreeMap<CheckpointId, Checkpoint> = BTreeMap::new();
    let run = RunState::default();
    let mut next = 1u32;
    for (i, k) in kinds.iter().enumerate() {
        let at = stamp(i);
        match k % 4 {
            0 => {
                log.push(Event::Entered { at });
                ops.push(HistoryOp::Draft(format!("d{i}")));
            }
            1 => {
                log.push(Event::InputReceived {
                    at,
                    input: fandhe_workshop_core::input::Input::Command(format!("c{i}")),
                });
                ops.push(HistoryOp::Submit(format!("c{i}")));
            }
            2 => {
                let id = CheckpointId(next);
                next += 1;
                let len = log.len() as u32;
                checkpoints.insert(
                    id,
                    Checkpoint {
                        run: run.clone(),
                        entry: None,
                        log_len: len,
                        owner: Some(Root("main".into())),
                    },
                );
                log.push(Event::CheckpointSaved {
                    at,
                    id,
                    log_len: len,
                });
                ops.push(HistoryOp::Engine(Op::Control(Control::Save)));
            }
            _ => {
                if let Some((&id, cp)) = checkpoints.iter().next() {
                    log.push(Event::Restored {
                        at,
                        id,
                        new_generation: Generation(2),
                        log_len: cp.log_len,
                        owner: StepUid("main/st-0".into()),
                    });
                    ops.push(HistoryOp::Engine(Op::Control(Control::Restore(id))));
                } else {
                    log.push(Event::Entered { at });
                    ops.push(HistoryOp::Reload);
                }
            }
        }
    }
    let snapshot = SessionState {
        run: run.clone(),
        entry: Some(EntryCopy {
            run,
            log_len: log.len() as u32,
        }),
        log,
        checkpoints,
        next_checkpoint: CheckpointId(next),
        drafts: BTreeMap::new(),
    };
    let base = load("empty");
    H::new(base.pack_id, base.versions, base.start, ops, snapshot)
}

#[test]
fn property_roundtrip_preserves_log_order() {
    check(
        "history-roundtrip-log-order",
        DEFAULT_CASES,
        vec(any::<u8>(), 2..24),
        |kinds| {
            let h = build(&kinds);
            check_log_positions(&h.snapshot).map_err(TestCaseError::fail)?;
            roundtrip_ok(&h).map_err(TestCaseError::fail)?;
            Ok(())
        },
    );
}

#[test]
fn comparator_self_check() {
    let h = load("two-rounds");
    roundtrip_ok(&h).unwrap();

    // ログの隣接する 2 件を入れ替える読み込み。
    let swap = |bytes: &[u8]| {
        let mut b = import(bytes)?;
        b.snapshot.log.swap(0, 1);
        Ok(b)
    };
    assert!(check_roundtrip(reference_export, swap, &h).is_err());

    // Restored.log_len を 1 ずらす読み込み（PoC-6 の X-6）。
    let shift_restored = |bytes: &[u8]| {
        let mut b = import(bytes)?;
        for e in &mut b.snapshot.log {
            if let Event::Restored { log_len, .. } = e {
                *log_len += 1;
            }
        }
        Ok(b)
    };
    assert!(check_roundtrip(reference_export, shift_restored, &h).is_err());

    // チェックポイントの log_len を 1 ずらす読み込み。
    let shift_checkpoint = |bytes: &[u8]| {
        let mut b = import(bytes)?;
        for cp in b.snapshot.checkpoints.values_mut() {
            cp.log_len += 1;
        }
        Ok(b)
    };
    assert!(check_roundtrip(reference_export, shift_checkpoint, &h).is_err());

    // 位置の不変条件だけが壊れた包みは、往復が一致しても断る。
    let mut broken = h.clone();
    for e in &mut broken.snapshot.log {
        if let Event::Restored { log_len, .. } = e {
            *log_len += 1;
        }
    }
    assert!(check_log_positions(&broken.snapshot).is_err());
    assert!(roundtrip_ok(&broken).is_err());
}
