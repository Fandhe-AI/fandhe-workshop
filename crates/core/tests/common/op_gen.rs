//! 任意の操作列の生成器と、抽象的な操作を具体の `Op` に解決して流すハーネス（TASK-219 1/3）。
//!
//! 呼び出し元は `op_sequence_replay.rs`（流し直しの一致）と `op_sequence_properties.rs`（性質 P-1〜P-6 の検査）。
//! 固定の生成（[`op_specs`]）と、現在の状態での正しい次の操作に誘導する生成（[`guided_op_specs`]）の両方を提供する。
//!
//! # 定義の出所
//!
//! 操作の種類・重み・値の表は `testdata/engine-event-consistency/test-data/property-test.md`（PoC-1 で固定）と同じである。
//! 生成の段階では状態を見ない抽象的な [`OpSpec`] だけを作り、[`Harness::resolve`] が操作の直前の状態から
//! 具体の `Op` に解決する。進行の宛先が「現在」「直前に届いたもの」「過去に出たもの」などの相対指定のため、
//! 具体の値は状態に依存するからである。
//!
//! # 決定性
//!
//! 解決は、現在の状態と、ハーネスが持つ履歴（届いた宛先）だけで決まる。乱数・時刻・I/O を使わない。
//! 同じ [`OpSpec`] 列を同じ初期状態から流せば、同じ `Op` 列と同じ状態になる。
//!
//! # 拡張点
//!
//! - #207（実装済み）: [`run_with`] の検査フックに `common/op_props.rs` の `PropChecker` を渡す。
//!   固定列 [`script_specs`] も同じ検査器に通す
//! - #208（実装済み）: [`OpSpec::Guided`] を [`Harness::resolve`] の中で現在の状態から解決する
//!   （`resolve` は `&self` で状態を読む形を保つ）。解決表は spec の `property-test-addendum.md`
//!   （PoC-1 の発見事項 F-11 で足した誘導）と同じで、操作の 40% を誘導にする [`guided_op_specs`] が使う
//!
//! - TASK-221（実装済み）: [`OpSpec::Reload`]（再読み込み。SAVE-4）を足す。`property-test.md` の操作の表には
//!   無く、表の外から足したものである（規則は spec の `engine-persistence.md` の SAVE-4）。再読み込みは `apply` の外の
//!   経路のため `Op` ではなく、[`Applied::op`] は `None` になる。進行の宛先の履歴（`seen`・`last_tick`）は再読み込みで
//!   変えないので、再読み込みの前に予約された宛先が `Tick(StaleGen)` などの古い宛先として後から届く。既存の戦略は
//!   変えず（種ごとの列と記録を保つため）、再読み込みを混ぜる戦略 [`op_specs_with_reload`]・
//!   [`guided_op_specs_with_reload`] を別に足す
//!
//! # 空列の扱い
//!
//! 連打は直前の学習者の入力が無いと捨てるため、展開後が空になりうる。この場合は先頭に
//! `OpSpec::Cmd(0)`（`/make-note first-note`）を 1 件置き、列を 1 件以上にそろえる。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, Decision, Input, Op};
use fandhe_workshop_core::session::{reload, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, Session};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;
use serde::Deserialize;

use super::contract_sample::testdata_dir;

use super::property::{op_sequence, MAX_OPS};
use super::tick::next_tick;

/// コマンドの表。
pub const CMDS: [&str; 7] = [
    "/make-note first-note",
    "/make-note",
    "/make-note Bad_Name",
    "/publish-note",
    "/sync-cloud",
    "/nothing",
    "/help",
];
/// 選択の表（`opt-none` は存在しない選択肢）。
pub const CHOICES: [&str; 3] = ["opt-memo", "opt-todo", "opt-none"];
/// 画面操作の対象（`v-files` の path）。
pub const PATHS: [&str; 2] = ["notes/draft", "notes/other"];
/// Step の表。
pub const NODES: [&str; 3] = ["main/st-1", "main/st-2", "main/st-3"];
/// 承認の表。
pub const DECISIONS: [Decision; 3] = [Decision::Approve, Decision::Reject, Decision::Revise];
/// 自然文の固定の文。
pub const TEXT: &str = "ノートを作りたい";

/// 進行の宛先の選び方。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Which {
    /// 現在の世代・Step・番号。
    Current,
    /// 直前に届いたものと同じ宛先。
    Dup,
    /// 1 つ前の世代。
    StaleGen,
    /// 現在と異なる Step。
    StaleNode,
    /// 過去に出た宛先から、添字で選ぶ 1 つ。
    StaleAny(usize),
}

/// 抽象的な操作（状態に解決する前）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpSpec {
    /// コマンド（`CMDS` の添字）。
    Cmd(usize),
    /// 選択（`CHOICES` の添字）。
    Choice(usize),
    /// 画面操作（`PATHS` の添字）。
    Ui(usize),
    /// 承認（`DECISIONS` の添字）。
    Approval(usize),
    /// 自然文。
    Text,
    /// 取り消し。
    Cancel,
    /// 進行。
    Tick(Which),
    /// 現在の状態での正しい次の操作（引数は選択の偶奇の決め手）。解決は [`Harness::resolve`] が行う。
    /// 固定の生成では深い状態（確認による達成・章の完了）に届く列が少ないため、誘導つきの生成が補う。
    Guided(u8),
    /// 停止（処理中でなければ取り消しとして扱う）。
    Stop,
    /// Step 移動（`NODES` の添字）。
    GoTo(usize),
    /// チェックポイント保存。
    Save,
    /// チェックポイント復帰（番号 1〜4）。
    Restore(u32),
    /// 再読み込み（SAVE-4）。`apply` の外の経路で、進行の宛先の履歴は保つ。学習者の入力ではない。
    Reload,
}

impl OpSpec {
    /// 学習者の入力か。連打の複製の対象になる。`Guided` は含めない
    /// （連打は直前の明示の入力だけを複製する。PoC-1 の展開と同じ）。
    pub fn is_learner_input(&self) -> bool {
        matches!(
            self,
            OpSpec::Cmd(_)
                | OpSpec::Choice(_)
                | OpSpec::Ui(_)
                | OpSpec::Approval(_)
                | OpSpec::Text
                | OpSpec::Cancel
        )
    }
}

/// 生成の段階の操作。連打は展開の前の形で持つ。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawSpec {
    /// 通常の操作。
    One(OpSpec),
    /// 直前の学習者の入力を、この回数だけ繰り返す。
    Burst(usize),
}

fn which_from(kind: u8, k: usize) -> Which {
    match kind {
        0..=1 => Which::Current,
        2 => Which::Dup,
        3 => Which::StaleGen,
        _ => {
            if k.is_multiple_of(2) {
                Which::StaleNode
            } else {
                Which::StaleAny(k)
            }
        }
    }
}

/// `property-test.md` の重みを付けた操作の戦略。
pub fn spec_strategy() -> impl Strategy<Value = RawSpec> {
    prop_oneof![
        6 => (0..CMDS.len()).prop_map(|i| RawSpec::One(OpSpec::Cmd(i))),
        4 => (0..CHOICES.len()).prop_map(|i| RawSpec::One(OpSpec::Choice(i))),
        4 => (0..PATHS.len()).prop_map(|i| RawSpec::One(OpSpec::Ui(i))),
        3 => (0..DECISIONS.len()).prop_map(|i| RawSpec::One(OpSpec::Approval(i))),
        1 => Just(RawSpec::One(OpSpec::Text)),
        1 => Just(RawSpec::One(OpSpec::Cancel)),
        9 => (0u8..5, 0usize..64)
            .prop_map(|(kind, k)| RawSpec::One(OpSpec::Tick(which_from(kind, k)))),
        2 => Just(RawSpec::One(OpSpec::Stop)),
        2 => (0..NODES.len()).prop_map(|i| RawSpec::One(OpSpec::GoTo(i))),
        2 => Just(RawSpec::One(OpSpec::Save)),
        2 => (1u32..=4).prop_map(|n| RawSpec::One(OpSpec::Restore(n))),
        2 => (2usize..=20).prop_map(RawSpec::Burst),
    ]
}

/// 操作の 40% を [`OpSpec::Guided`]、残り 60% を [`spec_strategy`] の重みのままにした戦略（連打は展開前の重み）。
pub fn guided_op_specs() -> impl Strategy<Value = Vec<OpSpec>> {
    let guided = (0u8..4).prop_map(|k| RawSpec::One(OpSpec::Guided(k)));
    op_sequence(prop_oneof![40 => guided, 60 => spec_strategy()]).prop_map(|raw| expand(&raw))
}

/// [`spec_strategy`] に再読み込みを数 % の重みで混ぜた戦略（TASK-221）。
pub fn reload_spec_strategy() -> impl Strategy<Value = RawSpec> {
    prop_oneof![38 => spec_strategy(), 3 => Just(RawSpec::One(OpSpec::Reload))]
}

/// 再読み込みを混ぜた固定の生成（TASK-221）。[`op_specs`] とは別の列になる。
pub fn op_specs_with_reload() -> impl Strategy<Value = Vec<OpSpec>> {
    op_sequence(reload_spec_strategy()).prop_map(|raw| expand(&raw))
}

/// 再読み込みを混ぜた誘導つきの生成（TASK-221）。操作の 40% を誘導、残り 60% を [`reload_spec_strategy`] にする。
pub fn guided_op_specs_with_reload() -> impl Strategy<Value = Vec<OpSpec>> {
    let guided = (0u8..4).prop_map(|k| RawSpec::One(OpSpec::Guided(k)));
    op_sequence(prop_oneof![40 => guided, 60 => reload_spec_strategy()])
        .prop_map(|raw| expand(&raw))
}

/// 連打を学習者の入力の複製に展開し、[`MAX_OPS`] で打ち切る。直前の入力が無い連打は捨てる。
pub fn expand(raw: &[RawSpec]) -> Vec<OpSpec> {
    let mut out: Vec<OpSpec> = Vec::new();
    let mut last_input: Option<OpSpec> = None;
    for r in raw {
        match r {
            RawSpec::One(spec) => {
                if spec.is_learner_input() {
                    last_input = Some(*spec);
                }
                out.push(*spec);
            }
            RawSpec::Burst(n) => {
                if let Some(spec) = last_input {
                    out.extend(std::iter::repeat_n(spec, *n));
                }
            }
        }
    }
    out.truncate(MAX_OPS);
    if out.is_empty() {
        out.push(OpSpec::Cmd(0));
    }
    out
}

/// 1〜[`MAX_OPS`] 個の操作列の戦略（連打は展開済み）。
pub fn op_specs() -> impl Strategy<Value = Vec<OpSpec>> {
    op_sequence(spec_strategy()).prop_map(|raw| expand(&raw))
}

/// 1 操作の結果。
#[derive(Debug, Clone, PartialEq)]
pub struct Applied {
    /// 解決した具体の操作。`None` は再読み込み（SAVE-4。`apply` の外の経路で `Op` ではない）。
    pub op: Option<Op>,
    /// 操作前の状態。
    pub before: SessionState,
    /// 操作後の状態。
    pub after: SessionState,
    /// 今回足されたイベント。
    pub events: Vec<Event>,
}

/// 状態を進める経路。
enum Route {
    /// 純粋関数 `apply` を直接呼ぶ。
    Pure,
    /// web が使う `Session` の経路。
    Session(Box<Session>),
}

/// 抽象的な操作を、現在の状態から具体の `Op` に解決して流すハーネス。
pub struct Harness<'a> {
    ir: &'a Ir,
    state: SessionState,
    seen: Vec<TickAddr>,
    last_tick: Option<TickAddr>,
    route: Route,
}

impl<'a> Harness<'a> {
    /// 純粋関数 `apply` の経路で作る。
    pub fn pure(ir: &'a Ir, initial: SessionState) -> Self {
        Harness {
            ir,
            state: initial,
            seen: Vec::new(),
            last_tick: None,
            route: Route::Pure,
        }
    }

    /// `Session::apply` の経路で作る。
    pub fn via_session(ir: &'a Ir, initial: SessionState) -> Self {
        Harness {
            ir,
            state: initial.clone(),
            seen: Vec::new(),
            last_tick: None,
            route: Route::Session(Box::new(Session::from_state(ir.clone(), initial))),
        }
    }

    /// 現在の状態。
    pub fn state(&self) -> &SessionState {
        &self.state
    }

    fn tick_addr(&self, which: Which) -> TickAddr {
        let step = self
            .state
            .run
            .current
            .clone()
            .unwrap_or_else(|| StepUid(NODES[0].to_string()));
        let generation = self.state.run.generation;
        let current = next_tick(&self.state).unwrap_or(TickAddr {
            generation,
            step: step.clone(),
            index: 0,
        });
        match which {
            Which::Current => current,
            Which::Dup => self.last_tick.clone().unwrap_or(current),
            Which::StaleGen => TickAddr {
                generation: Generation(generation.0.saturating_sub(1)),
                ..current
            },
            Which::StaleNode => {
                let other = NODES
                    .iter()
                    .find(|n| **n != step.0)
                    .unwrap_or(&NODES[0])
                    .to_string();
                TickAddr {
                    step: StepUid(other),
                    ..current
                }
            }
            Which::StaleAny(k) => match self.seen.len() {
                0 => current,
                len => self.seen[k % len].clone(),
            },
        }
    }

    /// 現在の Step の状態（どの根にも無ければ未着手）。
    fn status_of(&self, step: &StepUid) -> StepStatus {
        self.state
            .run
            .slices
            .get(&step.root())
            .and_then(|s| s.status.get(step))
            .copied()
            .unwrap_or_default()
    }

    /// 現在の状態での正しい次の操作を、既存の腕へ委ねて解決する。
    fn guided(&self, k: u8) -> Op {
        if self.state.run.busy.is_some() {
            return Op::Tick(self.tick_addr(Which::Current));
        }
        let Some(current) = self.state.run.current.as_ref() else {
            return self
                .resolve(&OpSpec::Cancel)
                .unwrap_or(Op::Input(Input::Cancel));
        };
        let next = if current.0 == NODES[0] {
            match self.status_of(current) {
                StepStatus::NotStarted | StepStatus::Entered => OpSpec::Cmd(0),
                _ => OpSpec::Ui(0),
            }
        } else if current.0 == NODES[1] {
            OpSpec::Choice(usize::from(k % 2 == 1))
        } else {
            OpSpec::Approval(0)
        };
        // `next` は `Reload` を含まないので `resolve` は必ず `Some` を返す。万一の場合は取り消しに倒す。
        self.resolve(&next).unwrap_or(Op::Input(Input::Cancel))
    }

    /// 現在の状態から、抽象的な操作を具体の `Op` に解決する。再読み込みは `Op` を持たないので `None` を返す。
    /// 呼び出し元は [`Harness::guided`] と [`Harness::step`] で、どちらも `Reload` を渡す前に分ける。
    pub fn resolve(&self, spec: &OpSpec) -> Option<Op> {
        let cancel = || Op::Input(Input::Cancel);
        Some(match *spec {
            OpSpec::Cmd(i) => Op::Input(Input::Command(CMDS[i % CMDS.len()].to_string())),
            OpSpec::Choice(i) => Op::Input(Input::Choice(CHOICES[i % CHOICES.len()].to_string())),
            OpSpec::Ui(i) => Op::Input(Input::Ui {
                view: "v-files".to_string(),
                path: PATHS[i % PATHS.len()].to_string(),
            }),
            OpSpec::Approval(i) => Op::Input(Input::Approval {
                decision: DECISIONS[i % DECISIONS.len()],
                target: None,
            }),
            OpSpec::Text => Op::Input(Input::Text(TEXT.to_string())),
            OpSpec::Cancel => cancel(),
            OpSpec::Tick(which) => Op::Tick(self.tick_addr(which)),
            OpSpec::Guided(k) => self.guided(k),
            OpSpec::Stop if self.state.run.busy.is_some() => Op::Control(Control::Stop),
            OpSpec::Stop => cancel(),
            OpSpec::GoTo(i) => {
                Op::Control(Control::GoTo(StepUid(NODES[i % NODES.len()].to_string())))
            }
            OpSpec::Save => Op::Control(Control::Save),
            OpSpec::Restore(n) => Op::Control(Control::Restore(CheckpointId(n))),
            OpSpec::Reload => return None,
        })
    }

    /// 再読み込み（SAVE-4）を流す。`apply` の外の経路で、純粋関数の経路は `session::reload`、
    /// `Session` の経路は `Session::reload` を呼ぶ。進行の宛先の履歴（`seen`・`last_tick`）は保つ。
    pub fn reload(&mut self) -> Vec<Event> {
        match &mut self.route {
            Route::Pure => {
                let (after, events) = reload(self.state.clone());
                self.state = after;
                events
            }
            Route::Session(session) => {
                let events = session.reload().to_vec();
                self.state = session.state().clone();
                events
            }
        }
    }

    /// 操作を解決して流し、状態と履歴を更新する。
    pub fn step(&mut self, spec: &OpSpec) -> Applied {
        let Some(op) = self.resolve(spec) else {
            // 再読み込み。宛先の履歴は変えない（前の世代の予約が後から古い宛先として届く）。
            let before = self.state.clone();
            let events = self.reload();
            return Applied {
                op: None,
                before,
                after: self.state.clone(),
                events,
            };
        };
        if let Op::Tick(addr) = &op {
            if !self.seen.contains(addr) {
                self.seen.push(addr.clone());
            }
            self.last_tick = Some(addr.clone());
        }
        let before = self.state.clone();
        let events = match &mut self.route {
            Route::Pure => {
                let (after, events) = apply(self.ir, before.clone(), op.clone());
                self.state = after;
                events
            }
            Route::Session(session) => {
                let events = session.apply(op.clone()).to_vec();
                self.state = session.state().clone();
                events
            }
        };
        Applied {
            op: Some(op),
            before,
            after: self.state.clone(),
            events,
        }
    }
}

/// 純粋関数の経路で列を流し、各操作の後に `check` を呼ぶ。#207 が検査を渡す入口。
pub fn run_with<F>(
    ir: &Ir,
    initial: SessionState,
    specs: &[OpSpec],
    mut check: F,
) -> Result<SessionState, TestCaseError>
where
    F: FnMut(usize, &Applied) -> Result<(), TestCaseError>,
{
    let mut harness = Harness::pure(ir, initial);
    for (i, spec) in specs.iter().enumerate() {
        let applied = harness.step(spec);
        check(i, &applied)?;
    }
    Ok(harness.state().clone())
}

/// 固定列（`scripted-inputs.json`）の 1 操作（`scripted-inputs.json` の `ops` の要素）。
#[derive(Deserialize)]
struct ScriptOp {
    op: String,
    which: Option<String>,
    input: Option<serde_json::Value>,
    node: Option<String>,
    id: Option<u32>,
}

#[derive(Deserialize)]
struct Script {
    ops: Vec<ScriptOp>,
}

fn index_of(table: &[&str], value: &str) -> usize {
    table
        .iter()
        .position(|v| *v == value)
        .unwrap_or_else(|| panic!("value {value:?} is not in the op_gen table"))
}

/// 固定列の入力を、生成器の表の添字に写す。表に無い値は理由を添えて panic する。
fn input_spec(v: &serde_json::Value) -> OpSpec {
    if v == "cancel" {
        return OpSpec::Cancel;
    }
    let obj = v.as_object().expect("input object");
    if let Some(c) = obj.get("command") {
        return OpSpec::Cmd(index_of(&CMDS, c.as_str().expect("command str")));
    }
    if let Some(c) = obj.get("choice") {
        return OpSpec::Choice(index_of(&CHOICES, c.as_str().expect("choice str")));
    }
    if let Some(u) = obj.get("ui") {
        return OpSpec::Ui(index_of(&PATHS, u["path"].as_str().expect("ui path")));
    }
    if let Some(a) = obj.get("approval") {
        let d = a["decision"].as_str().expect("decision");
        let i = DECISIONS
            .iter()
            .position(|x| format!("{x:?}").to_lowercase() == d)
            .expect("decision in table");
        return OpSpec::Approval(i);
    }
    panic!("unsupported scripted input: {v}");
}

pub fn script_specs() -> Vec<OpSpec> {
    let path: PathBuf =
        testdata_dir().join("engine-event-consistency/test-data/scripted-inputs.json");
    let bytes = std::fs::read(path).expect("read scripted inputs");
    let script: Script = serde_json::from_slice(&bytes).expect("parse scripted inputs");
    script
        .ops
        .iter()
        .map(|o| match o.op.as_str() {
            "input" => input_spec(o.input.as_ref().expect("input")),
            "tick" => OpSpec::Tick(match o.which.as_deref().expect("which") {
                "current" => Which::Current,
                "dup" => Which::Dup,
                "stale_gen" => Which::StaleGen,
                "stale_node" => Which::StaleNode,
                other => panic!("unknown which {other:?}"),
            }),
            "stop" => OpSpec::Stop,
            "goto" => OpSpec::GoTo(index_of(&NODES, o.node.as_deref().expect("node"))),
            "save" => OpSpec::Save,
            "restore" => OpSpec::Restore(o.id.expect("id")),
            other => panic!("unknown op {other:?}"),
        })
        .collect()
}
