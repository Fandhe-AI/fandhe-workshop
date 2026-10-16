//! `ops` の適用し直しによる復元（`history_replay`、TASK-316 1/3、SAVE-1）の試験。
//!
//! 「記録の経路」（下書きと送信を記録しながら `apply` で進めた状態）で得た写しと、その `ops` を初期状態に
//! 適用し直した状態が一致することを、固定の列と性質テスト（固定・誘導つきの生成を各 1,000 通り）で確かめる。
//! 写しとの照合の関数・書き出し→読み込みの一致・再読み込みの意味は TASK-316 2/3 と TASK-320 の範囲。
//!
//! 性質テストの成功条件は一致だけにする。統計は出力するだけで assert しない（`docs/testing.md` 第 2 節）。
//! 生成器の自然文（`TEXT`）は接頭辞 `/` で始まらないので自然文に、`CMDS` はすべて `/` で始まるのでコマンドに
//! 分かれる。この前提で、記録の経路は解決済みの `Op` をそのまま使い、分類し直さない（分類の誤りを検出するため）。

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;

use common::contract_sample::{load, testdata_dir};
use common::history_sample;
use common::op_gen::{guided_op_specs, op_specs, Harness, OpSpec, Which};
use common::property::{check, MAX_OPS};
use fandhe_workshop_contract::history::HistoryOp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::{build_envelope, record_draft, record_submit};
use fandhe_workshop_core::history_replay::{replay, submit_input, ReplayError};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::ops_record::{apply_recorded, OpsRecorder, SessionOp};
use fandhe_workshop_core::session::{apply, SessionState};
use proptest::collection::vec;
use proptest::prelude::*;

/// 教材（IR）と最初の状態を読む。ケースごとではなく、試験の先頭で 1 回だけ呼ぶ。
fn load_min_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 下書きの種類。`None` は下書きを挟まない。
fn draft_text(kind: u8) -> Option<&'static str> {
    match kind % 5 {
        0 => None,
        1 => Some(""),
        2 => Some(" \u{3000}"),
        3 => Some("ノートを作りたい"),
        _ => Some("/make-note first-note"),
    }
}

/// 記録の経路。下書きと送信を記録しながら `apply` で状態を進める（実行時の入力欄が通る道に当たる）。
/// 操作の解決（進行の宛先など）は、同じ列を流す `Harness` に任せる。
struct RecordPath<'a> {
    ir: &'a Ir,
    harness: Harness<'a>,
    state: SessionState,
    rec: OpsRecorder,
}

impl<'a> RecordPath<'a> {
    fn new(ir: &'a Ir, initial: &SessionState) -> Self {
        RecordPath {
            ir,
            harness: Harness::pure(ir, initial.clone()),
            state: initial.clone(),
            rec: OpsRecorder::new(),
        }
    }

    fn draft(&mut self, text: &str) {
        record_draft(&mut self.state, &mut self.rec, text.to_owned());
    }

    /// 抽象の操作を解決して記録しながら進める。入力は下書き→送信→適用の順にする。
    fn spec(&mut self, spec: &OpSpec) {
        let applied = self.harness.step(spec);
        self.op(applied.op);
    }

    /// 解決済みの操作を記録しながら進める。解決済みの `Op` をそのまま使い、分類し直さない。
    fn op(&mut self, op: Op) {
        match &op {
            Op::Input(Input::Command(s) | Input::Text(s)) => {
                record_draft(&mut self.state, &mut self.rec, s.clone());
                assert!(record_submit(&mut self.state, &mut self.rec, s.clone()));
                self.state = apply(self.ir, self.state.clone(), op).0;
            }
            _ => {
                let (state, _) = apply_recorded(self.ir, self.state.clone(), &mut self.rec, op);
                self.state = state;
            }
        }
    }
}

/// 食い違いを欄の名前で返す（全文は出さない）。
fn diff_field(a: &SessionState, b: &SessionState) -> &'static str {
    if a.run != b.run {
        "run"
    } else if a.log != b.log {
        "log"
    } else if a.entry != b.entry {
        "entry"
    } else if a.checkpoints != b.checkpoints {
        "checkpoints"
    } else if a.next_checkpoint != b.next_checkpoint {
        "next_checkpoint"
    } else if a.drafts != b.drafts {
        "drafts"
    } else if a.hints_opened != b.hints_opened {
        "hints_opened"
    } else {
        "other"
    }
}

/// 記録の経路の `ops` を包み、適用し直した結果が写しと一致するかを返す。
fn replay_matches_snapshot(
    ir: &Ir,
    initial: &SessionState,
    path: RecordPath<'_>,
) -> Result<(), String> {
    let RecordPath { state, rec, .. } = path;
    let env =
        build_envelope(ir, "s-1", rec.into_ops(), state).map_err(|e| format!("build: {e}"))?;
    match replay(ir, initial.clone(), &env.ops) {
        Ok(replayed) if replayed == env.snapshot => Ok(()),
        Ok(replayed) => Err(format!(
            "replayed state differs from the snapshot in {}",
            diff_field(&replayed, &env.snapshot)
        )),
        Err(e) => Err(format!("replay failed: {e}")),
    }
}

#[test]
fn empty_ops_replay_to_initial() {
    let (ir, initial) = load_min_ir();
    assert_eq!(replay(&ir, initial.clone(), &[]), Ok(initial.clone()));
    assert_eq!(initial, history_sample::load("empty").snapshot);
}

#[test]
fn scripted_history_replays_to_snapshot() {
    let (ir, initial) = load_min_ir();
    let mut path = RecordPath::new(&ir, &initial);
    path.draft("/make");
    path.draft("/make-note first-note");
    path.spec(&OpSpec::Cmd(0));
    path.spec(&OpSpec::Tick(Which::Current));
    path.spec(&OpSpec::Tick(Which::Current));
    path.spec(&OpSpec::Guided(0));
    path.spec(&OpSpec::Guided(0));
    path.draft("ノートを作りたい");
    path.spec(&OpSpec::Text);
    path.spec(&OpSpec::Stop);
    path.spec(&OpSpec::GoTo(1));
    path.spec(&OpSpec::Save);
    path.spec(&OpSpec::Restore(9));
    path.spec(&OpSpec::Restore(1));
    path.op(Op::Control(Control::OpenHint));
    path.draft("");
    path.draft("未送信の下書き");
    let ops = path.rec.ops().to_vec();
    let has = |f: fn(&SessionOp) -> bool| ops.iter().any(f);
    assert!(has(|o| matches!(o, HistoryOp::Submit(_))));
    assert!(has(|o| matches!(o, HistoryOp::Draft(_))));
    assert!(has(|o| matches!(o, HistoryOp::Engine(_))));
    replay_matches_snapshot(&ir, &initial, path).expect("replay matches the snapshot");
}

#[test]
fn submit_is_classified_by_the_profile_and_kept_verbatim() {
    let (ir, initial) = load_min_ir();
    let text = " /make-note first-note ".to_owned();
    let ops = vec![HistoryOp::Submit(text.clone())];
    let state = replay(&ir, initial.clone(), &ops).expect("replay");
    let received: Vec<&Input> = state
        .log
        .iter()
        .filter_map(|e| match e {
            Event::InputReceived { input, .. } => Some(input),
            _ => None,
        })
        .collect();
    assert_eq!(received, [&Input::Command(text)]);

    // 自然文は既定の接頭辞ではコマンドにならず、接頭辞が空なら何でもコマンドになる。
    let natural = vec![HistoryOp::Submit("ノートを作りたい".to_owned())];
    let state = replay(&ir, initial.clone(), &natural).expect("replay");
    assert!(state.log.iter().any(|e| matches!(
        e,
        Event::InputReceived { input: Input::Text(t), .. } if t == "ノートを作りたい"
    )));
    let mut open = ir.clone();
    open.pack.profile = Some(serde_json::json!({ "command_prefix": "" }));
    let state = replay(&open, initial, &natural).expect("replay");
    assert!(state.log.iter().any(|e| matches!(
        e,
        Event::InputReceived { input: Input::Command(t), .. } if t == "ノートを作りたい"
    )));

    assert_eq!(submit_input("/", "/a".into()), Input::Command("/a".into()));
    assert_eq!(
        submit_input("/", " /a ".into()),
        Input::Command(" /a ".into())
    );
    assert_eq!(submit_input("/", "a".into()), Input::Text("a".into()));
    assert_eq!(submit_input("/", "".into()), Input::Text("".into()));
    assert_eq!(submit_input("", "a".into()), Input::Command("a".into()));
}

#[test]
fn draft_before_start_is_only_in_ops() {
    let (ir, mut initial) = load_min_ir();
    // 現在の Step が無い（開始前の）状態にする。
    initial.run.current = None;
    let ops = vec![HistoryOp::Draft("まだ開始前".to_owned())];
    let state = replay(&ir, initial.clone(), &ops).expect("replay");
    assert_eq!(state, initial);
    assert!(state.drafts.is_empty());
}

#[test]
fn blank_submit_is_rejected() {
    let (ir, initial) = load_min_ir();
    let ops = vec![
        HistoryOp::Draft("x".to_owned()),
        HistoryOp::Submit("  \u{3000}".to_owned()),
    ];
    assert_eq!(
        replay(&ir, initial, &ops),
        Err(ReplayError::BlankSubmit { index: 1 })
    );
}

#[test]
fn reload_is_rejected_until_task_320() {
    let (ir, initial) = load_min_ir();
    let ops = vec![HistoryOp::Draft("x".to_owned()), HistoryOp::Reload];
    assert_eq!(
        replay(&ir, initial.clone(), &ops),
        Err(ReplayError::UnsupportedReload { index: 1 })
    );
    let sample = history_sample::load("two-rounds");
    let first = sample
        .ops
        .iter()
        .position(|o| matches!(o, HistoryOp::Reload))
        .expect("two-rounds has a reload");
    assert_eq!(
        replay(&ir, initial, &sample.ops),
        Err(ReplayError::UnsupportedReload { index: first })
    );
}

#[test]
fn unreadable_profile_fails_closed() {
    let (mut ir, initial) = load_min_ir();
    ir.pack.profile = Some(serde_json::json!({ "command_prefix": "ab" }));
    assert!(matches!(
        replay(&ir, initial, &[]),
        Err(ReplayError::Profile(_))
    ));
}

/// 戦略を 1,000 通り流し、適用し直した結果と写しの一致を確かめる。
fn run_property<S>(name: &str, strategy: S)
where
    S: Strategy<Value = Vec<OpSpec>>,
{
    let (ir, initial) = load_min_ir();
    let stats = RefCell::new(BTreeMap::<&'static str, u64>::new());
    let with_drafts = (strategy, vec(0u8..5, MAX_OPS + 1));
    check(name, 1_000, with_drafts, |(specs, drafts)| {
        let mut path = RecordPath::new(&ir, &initial);
        for (i, spec) in specs.iter().enumerate() {
            if let Some(d) = draft_text(drafts[i]) {
                path.draft(d);
            }
            path.spec(spec);
            // 下書きは `apply` に影響しないため、記録の経路の実行は流した状態と一致する。
            prop_assert_eq!(&path.state.run, &path.harness.state().run, "run at #{}", i);
            prop_assert_eq!(&path.state.log, &path.harness.state().log, "log at #{}", i);
        }
        if let Some(d) = draft_text(drafts[specs.len()]) {
            path.draft(d);
        }
        let mut s = stats.borrow_mut();
        for op in path.rec.ops() {
            let key = match op {
                HistoryOp::Submit(_) => "submit",
                HistoryOp::Draft(_) => "draft",
                _ => "engine",
            };
            *s.entry(key).or_default() += 1;
        }
        drop(s);
        match replay_matches_snapshot(&ir, &initial, path) {
            Ok(()) => Ok(()),
            Err(message) => {
                prop_assert!(false, "{message}");
                Ok(())
            }
        }
    });
    println!("PROPERTY-TEST stats={:?}", stats.borrow());
}

#[test]
fn property_replay_matches_snapshot_fixed() {
    run_property("history-replay", op_specs());
}

#[test]
fn property_replay_matches_snapshot_guided() {
    run_property("history-replay-guided", guided_op_specs());
}
