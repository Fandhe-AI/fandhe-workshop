//! 受講者の操作を `ops` に記録する規則の試験（TASK-315 1/2、SAVE-2・SAVE-24）。
//!
//! 送信の経路（Enter・ボタン）で記録が変わらないこと、20,000 文字の送信がそのまま残ること、
//! 畳みと空白の除外、拒否・無視の記録、並びの保持（性質テスト）を確かめる。
//! 送信の本文は長いので、比較は真偽で行い失敗の出力に全文を出さない。呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;

use common::property::{check, DEFAULT_CASES, MAX_OPS};
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::limits::MAX_INPUT_CODE_POINTS;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::{apply_recorded, OpsRecorder, SessionOp};
use fandhe_workshop_core::session::{apply, RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use proptest::prelude::*;

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn stale_tick() -> Op {
    Op::Tick(TickAddr {
        generation: Generation(99),
        step: StepUid("main/st-1".to_string()),
        index: 0,
    })
}

#[test]
fn save24_example_shape_is_exact() {
    let mut rec = OpsRecorder::new();
    rec.draft("/help".to_string());
    assert!(rec.submit("/help".to_string()));
    rec.draft("未送信のメモ".to_string());
    let expected = serde_json::json!([
        {"draft": "/help"},
        {"submit": "/help"},
        {"draft": "未送信のメモ"}
    ]);
    assert_eq!(serde_json::to_value(rec.ops()).unwrap(), expected);
}

#[test]
fn submit_keeps_max_input_code_points_verbatim() {
    for ch in ["a", "あ"] {
        let text = ch.repeat(MAX_INPUT_CODE_POINTS);
        let mut rec = OpsRecorder::new();
        assert!(rec.submit(text.clone()));
        let kept = match rec.ops() {
            [HistoryOp::Submit(s)] => s.clone(),
            _ => panic!("unexpected record shape"),
        };
        assert_eq!(kept.chars().count(), MAX_INPUT_CODE_POINTS);
        assert!(kept == text, "submit text was altered");
        let json = serde_json::to_string(rec.ops()).unwrap();
        let back: Vec<SessionOp> = serde_json::from_str(&json).unwrap();
        assert!(back.as_slice() == rec.ops(), "roundtrip altered the record");
    }
}

#[test]
fn submit_text_is_not_trimmed_or_classified() {
    let texts = [
        "  /help  ",
        "\u{3000}全角空白\u{3000}",
        "/help",
        "help me",
        "\tx\n",
    ];
    let mut rec = OpsRecorder::new();
    for t in texts {
        assert!(rec.submit(t.to_string()));
    }
    let want: Vec<SessionOp> = texts
        .iter()
        .map(|t| HistoryOp::Submit((*t).to_string()))
        .collect();
    assert_eq!(rec.ops(), want.as_slice());
}

#[test]
fn consecutive_drafts_fold_to_last() {
    let mut rec = OpsRecorder::new();
    rec.draft("a".into());
    rec.draft("ab".into());
    rec.draft("abc".into());
    assert_eq!(rec.ops(), [HistoryOp::Draft("abc".to_string())].as_slice());

    rec.submit("abc".into());
    rec.draft("d".into());
    rec.engine(Op::Input(Input::Cancel));
    rec.draft("e".into());
    assert_eq!(rec.len(), 5);
    assert_eq!(rec.ops()[2], HistoryOp::Draft("d".to_string()));
    assert_eq!(rec.ops()[4], HistoryOp::Draft("e".to_string()));
}

#[test]
fn whitespace_only_submit_is_not_recorded() {
    let mut rec = OpsRecorder::new();
    rec.draft("x".into());
    for t in ["", " ", "\u{3000}\n\t"] {
        assert!(!rec.submit(t.to_string()));
    }
    assert_eq!(rec.len(), 1);
    rec.draft("y".into());
    assert_eq!(rec.ops(), [HistoryOp::Draft("y".to_string())].as_slice());
}

/// `HistoryOp` に variant が増えるとここがコンパイルできなくなる（Enter 専用の種類を足せない）。
fn kind_of(op: &SessionOp) -> &'static str {
    match op {
        HistoryOp::Submit(_) => "submit",
        HistoryOp::Draft(_) => "draft",
        HistoryOp::Reload => "reload",
        HistoryOp::Engine(_) => "engine",
    }
}

#[test]
fn enter_and_button_share_one_record_kind() {
    // Enter の経路もボタンの経路も、入口は submit の 1 つ（経路を区別する引数が API に無い）。
    // 以下は、記録の種類のタグに経路の語が現れず、既知の種類に限られることを検査する。
    let mut rec = OpsRecorder::new();
    rec.draft("d".into());
    rec.submit("s".into());
    rec.engine(Op::Input(Input::Command("ls".into())));
    rec.engine(stale_tick());
    rec.engine(Op::Control(Control::Save));
    let tags: Vec<String> = rec
        .ops()
        .iter()
        .map(|o| {
            let v = serde_json::to_value(o).unwrap();
            v.as_object().unwrap().keys().next().unwrap().clone()
        })
        .collect();
    for t in &tags {
        assert!(
            ["submit", "draft", "input", "tick", "control"].contains(&t.as_str()),
            "unexpected tag: {t}"
        );
        assert!(!t.contains("enter") && !t.contains("key"));
    }
    assert!(rec.ops().iter().all(|o| kind_of(o) != "reload"));
}

/// st-1 が `Entered`・処理中でない・世代 1 の状態。
fn entered() -> SessionState {
    let u = StepUid("main/st-1".to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

#[test]
fn refused_and_ignored_engine_ops_are_recorded() {
    let ir = base_ir();
    let mut rec = OpsRecorder::new();
    let mut state = entered();
    // 受け付けられない入力（拒否の判定が出る）・古い世代の進行（無視される）・状態を変えない操作。
    let ops = [
        Op::Input(Input::Command("whatever".to_string())),
        stale_tick(),
        Op::Control(Control::Save),
    ];
    for (i, op) in ops.iter().enumerate() {
        let before = state.log.len();
        let (next, _) = apply_recorded(&ir, state, &mut rec, op.clone());
        state = next;
        assert_eq!(rec.len(), i + 1);
        assert_eq!(rec.ops().last(), Some(&HistoryOp::Engine(op.clone())));
        if i == 0 {
            let added = &state.log[before..];
            assert!(
                added.iter().any(|e| matches!(e, Event::Judged { .. })),
                "the input was not judged: {added:?}"
            );
        }
    }
}

#[test]
fn apply_recorded_matches_apply() {
    let ir = base_ir();
    let ops = [
        Op::Control(Control::Begin("main/st-1".to_string())),
        Op::Input(Input::Command("ls".to_string())),
        stale_tick(),
        Op::Input(Input::Cancel),
    ];
    let mut rec = OpsRecorder::new();
    let mut a = SessionState::default();
    let mut b = SessionState::default();
    for op in ops {
        let (na, ea) = apply_recorded(&ir, a, &mut rec, op.clone());
        let (nb, eb) = apply(&ir, b, op);
        assert!(na == nb, "state differs");
        assert!(ea == eb, "events differ");
        a = na;
        b = nb;
    }
    assert_eq!(rec.len(), 4);
}

#[derive(Debug, Clone)]
enum Act {
    Draft(String),
    Submit(String),
    Engine(u8),
}

fn engine_op(n: u8, tag: usize) -> Op {
    match n % 3 {
        0 => Op::Input(Input::Command(format!("c{tag}"))),
        1 => Op::Input(Input::Text(format!("t{tag}"))),
        _ => Op::Tick(TickAddr {
            generation: Generation(1),
            step: StepUid(format!("s{tag}")),
            index: 0,
        }),
    }
}

fn act() -> impl Strategy<Value = Act> {
    prop_oneof![
        "[a-z ]{0,4}".prop_map(Act::Draft),
        prop_oneof![Just(" ".to_string()), "[ a-z\u{3000}]{0,5}"].prop_map(Act::Submit),
        any::<u8>().prop_map(Act::Engine),
    ]
}

#[test]
fn property_ops_follow_call_order() {
    check(
        "ops_record_follows_call_order",
        DEFAULT_CASES,
        proptest::collection::vec(act(), 1..=MAX_OPS),
        |acts| {
            let mut rec = OpsRecorder::new();
            let mut model: Vec<SessionOp> = Vec::new();
            for (i, a) in acts.into_iter().enumerate() {
                match a {
                    Act::Draft(t) => {
                        // 種別ごとに異なる文字列にして、入れ替えが差になるようにする
                        let t = format!("{t}#{i}");
                        rec.draft(t.clone());
                        if let Some(HistoryOp::Draft(p)) = model.last_mut() {
                            *p = t;
                        } else {
                            model.push(HistoryOp::Draft(t));
                        }
                    }
                    Act::Submit(t) => {
                        let recorded = rec.submit(t.clone());
                        prop_assert_eq!(recorded, !t.trim().is_empty());
                        if recorded {
                            model.push(HistoryOp::Submit(t));
                        }
                    }
                    Act::Engine(n) => {
                        let op = engine_op(n, i);
                        rec.engine(op.clone());
                        model.push(HistoryOp::Engine(op));
                    }
                }
            }
            prop_assert_eq!(rec.ops(), model.as_slice());
            Ok(())
        },
    );
}
