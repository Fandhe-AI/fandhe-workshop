//! 早送りの入・切の同一性の性質テスト（TASK-228 1/2、EVT-27）。
//!
//! 同じ操作列を、早送りの入（進行を連続適用）と切（タイマー相当で 1 件ずつ適用）で流し、各操作の後に
//! 状態・進行イベント（`Event::Progress`）の数・進行の宛先・左の最終的な変化が一致することを固定する。
//! 変わるのは進行の間隔だけで、宛先は変わらない（EVT-27）。左の変化は、core では世界状態と `Projection`
//! の印・復習の一覧で表す（web の `artifact_rows` は core から使えない）。増分の区切り方（進行 1 件ごと／
//! 連続適用 1 回分ごと）に依らず一括の導出と一致することも確かめる。
//! TASK-228 2/2 で、履歴の `ops`（保存用の包み・書き出しのバイト列を含む）の一致と、列の途中で入・切を切り替えた
//! 列（mixed）が切り替えない列と同じ結果になることも固定する。早送りの入・切そのものは `ops` に入れない（D-54・
//! SESS-6）ので、切り替えの痕跡は履歴に残らない。
//! 既存の `fast_forward.rs`（TASK-227）の試験は変えない。呼び出し元は CI の `make test` のみである。

mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use common::contract_sample::{load, testdata_dir};
use common::op_gen::{
    guided_op_specs, guided_op_specs_with_reload, op_specs, op_specs_with_reload, script_specs,
    Harness, OpSpec, Which,
};
use common::property::{check, DEFAULT_CASES};
use common::tick::next_tick;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::recorded_session::RecordedSession;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::Element;
use fandhe_workshop_core::{load_ir, Session};
use proptest::prelude::*;
use serde_json::json;

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn progress_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

fn changed_of(events: &[Event]) -> Vec<Event> {
    events
        .iter()
        .filter(|e| matches!(e, Event::Changed { .. }))
        .cloned()
        .collect()
}

/// 宛先が進まない（世代・Step が変わった、または番号が増えない）か。
fn is_stuck(state: &SessionState, prev: &TickAddr) -> bool {
    next_tick(state).is_some_and(|n| {
        n.generation != prev.generation || n.step != prev.step || n.index <= prev.index
    })
}

/// 列の途中の切り替えの予定。処理中の区間を、最初の向き（真が入）と区切りの長さ（入は件数の上限、切は件数）で表す。
type Sched = (bool, Vec<u32>);

/// 入・切の両側と、列の途中で入・切を切り替える側（mixed）と、観察のために積み上げる値。
struct Pair {
    on: RecordedSession,
    off: RecordedSession,
    mixed: RecordedSession,
    mixed_proj: Projection,
    mixed_events: Vec<Event>,
    mixed_ticks: Vec<TickAddr>,
    /// 1 つの処理中の区間で入と切の両方が進行を適用した回数（空振りの確認用）。
    toggled: usize,
    on_proj: Projection,
    off_proj: Projection,
    on_events: Vec<Event>,
    off_events: Vec<Event>,
    on_ticks: Vec<TickAddr>,
    off_ticks: Vec<TickAddr>,
    /// 連続適用で 1 件以上の進行を適用した回数（空振りの確認用）。
    ff_active: usize,
    /// 印が空でなかった確認の回数（空振りの確認用）。
    marks_seen: usize,
}

impl Pair {
    fn new(ir: Ir, initial: SessionState) -> Self {
        let off = RecordedSession::new(Session::from_state(ir, initial), "s-1");
        Pair {
            on: off.clone(),
            mixed: off.clone(),
            mixed_proj: Projection::new(),
            mixed_events: Vec::new(),
            mixed_ticks: Vec::new(),
            toggled: 0,
            off,
            on_proj: Projection::new(),
            off_proj: Projection::new(),
            on_events: Vec::new(),
            off_events: Vec::new(),
            on_ticks: Vec::new(),
            off_ticks: Vec::new(),
            ff_active: 0,
            marks_seen: 0,
        }
    }

    /// 操作（`None` は再読み込み）を両側へ流す。
    fn apply_op(&mut self, op: Option<&Op>) {
        match op {
            Some(op) => {
                self.off_events.extend(self.off.apply(op.clone()).to_vec());
                self.on_events.extend(self.on.apply(op.clone()).to_vec());
                self.mixed_events
                    .extend(self.mixed.apply(op.clone()).to_vec());
            }
            None => {
                self.off_events.extend(self.off.reload().to_vec());
                self.on_events.extend(self.on.reload().to_vec());
                self.mixed_events.extend(self.mixed.reload().to_vec());
            }
        }
        self.mixed_proj
            .extend(self.mixed.session().state().log.as_slice());
        self.off_proj
            .extend(self.off.session().state().log.as_slice());
        self.on_proj
            .extend(self.on.session().state().log.as_slice());
    }

    /// 切の 1 件。適用した宛先を積み、導出を 1 件ごとに進める。
    fn off_tick(&mut self, op: Op) {
        self.off_events.extend(self.off.apply(op).to_vec());
        self.off_proj
            .extend(self.off.session().state().log.as_slice());
    }

    /// 切: 処理中が解けるか番号が進まなくなるまで、タイマー相当で 1 件ずつ。基準の `Harness` も進める。
    fn off_by_tick(&mut self, harness: &mut Harness) {
        let mut guard = 0u32;
        while let Some(addr) = next_tick(self.off.session().state()) {
            let Some(op) = harness.step(&OpSpec::Tick(Which::Current)).op else {
                break;
            };
            self.off_tick(op);
            self.off_ticks.push(addr.clone());
            guard += 1;
            if is_stuck(self.off.session().state(), &addr) || guard > 10_000 {
                break;
            }
        }
    }

    /// 切: 進行の宛先をそのまま 1 件ずつ最後まで（`Harness` を使わない固定例用）。
    fn off_by_tick_all(&mut self) {
        while let Some(addr) = next_tick(self.off.session().state()) {
            self.off_tick(Op::Tick(addr.clone()));
            self.off_ticks.push(addr);
        }
    }

    /// 入: 連続適用を 1 回。増分の導出は 1 回分につき 1 回だけ進める。
    fn on_fast_forward(&mut self) {
        let ff = self.on.fast_forward(u32::MAX);
        if !ff.ticks.is_empty() {
            self.ff_active += 1;
        }
        self.on_ticks.extend(ff.ticks);
        self.on_events.extend(ff.events);
        self.on_proj
            .extend(self.on.session().state().log.as_slice());
    }

    /// mixed: 予定に沿って入（連続適用）と切（1 件ずつ）を切り替え、最後は入で終わりまで流す。
    fn mixed_run(&mut self, sched: &Sched) {
        let (mut on, segments) = (sched.0, &sched.1);
        let (mut used_on, mut used_off) = (false, false);
        for &k in segments {
            if next_tick(self.mixed.session().state()).is_none() {
                break;
            }
            let before = self.mixed_ticks.len();
            if on {
                self.mixed_fast_forward(k);
            } else {
                for _ in 0..k {
                    let Some(addr) = next_tick(self.mixed.session().state()) else {
                        break;
                    };
                    self.mixed_events
                        .extend(self.mixed.apply(Op::Tick(addr.clone())).to_vec());
                    self.mixed_ticks.push(addr.clone());
                    if is_stuck(self.mixed.session().state(), &addr) {
                        break;
                    }
                }
            }
            if self.mixed_ticks.len() > before {
                if on {
                    used_on = true;
                } else {
                    used_off = true;
                }
            }
            on = !on;
        }
        if next_tick(self.mixed.session().state()).is_some() {
            let before = self.mixed_ticks.len();
            self.mixed_fast_forward(u32::MAX);
            used_on |= self.mixed_ticks.len() > before;
        }
        if used_on && used_off {
            self.toggled += 1;
        }
        self.mixed_proj
            .extend(self.mixed.session().state().log.as_slice());
    }

    fn mixed_fast_forward(&mut self, limit: u32) {
        let ff = self.mixed.fast_forward(limit);
        self.mixed_ticks.extend(ff.ticks);
        self.mixed_events.extend(ff.events);
    }

    /// 入・切で一致すべきものを確かめる。
    fn assert_same(&mut self) -> Result<(), TestCaseError> {
        let (on, off) = (self.on.session().state(), self.off.session().state());
        prop_assert_eq!(on, off);
        prop_assert_eq!(
            progress_count(&self.on_events),
            progress_count(&self.off_events)
        );
        // 履歴の入力の列・保存用の包み・書き出しのバイト列（D-54・SESS-6）。
        prop_assert_eq!(self.on.ops(), self.off.ops());
        prop_assert_eq!(self.mixed.ops(), self.off.ops());
        let env = self.off.envelope_for_save();
        prop_assert_eq!(&self.on.envelope_for_save(), &env);
        prop_assert_eq!(&self.mixed.envelope_for_save(), &env);
        if let Some(env) = &env {
            let bytes = export_bytes(env).expect("export");
            prop_assert_eq!(
                &bytes,
                &export_bytes(&self.on.envelope_for_save().expect("on")).expect("export")
            );
            prop_assert_eq!(
                &bytes,
                &export_bytes(&self.mixed.envelope_for_save().expect("mixed")).expect("export")
            );
        }
        // 列の途中で切り替えても、状態・進行・左の変化は切り替えない列と同じ。
        prop_assert_eq!(self.mixed.session().state(), off);
        prop_assert_eq!(&self.mixed_ticks, &self.off_ticks);
        prop_assert_eq!(
            progress_count(&self.mixed_events),
            progress_count(&self.off_events)
        );
        prop_assert_eq!(changed_of(&self.mixed_events), changed_of(&self.off_events));
        prop_assert_eq!(self.mixed_proj.marks(), self.off_proj.marks());
        prop_assert_eq!(self.mixed_proj.review(), self.off_proj.review());
        prop_assert_eq!(&self.on_ticks, &self.off_ticks);
        prop_assert_eq!(&on.run.world, &off.run.world);
        prop_assert_eq!(self.on_proj.marks(), self.off_proj.marks());
        prop_assert_eq!(self.on_proj.review(), self.off_proj.review());
        let whole = Projection::from_events(&on.log);
        prop_assert_eq!(self.on_proj.marks(), whole.marks());
        prop_assert_eq!(self.off_proj.marks(), whole.marks());
        prop_assert_eq!(self.on_proj.review(), whole.review());
        prop_assert_eq!(self.off_proj.review(), whole.review());
        prop_assert_eq!(changed_of(&self.on_events), changed_of(&self.off_events));
        if !self.on_proj.marks().is_empty() {
            self.marks_seen += 1;
        }
        Ok(())
    }
}

/// 任意の操作列の各操作の後に、入・切で状態・進行の数・宛先・左の変化が一致する。
///
/// 操作は切の側の `Harness` が解決し、同じ `Op` を両側へ渡す。
///
/// `scheds` は mixed の切り替えの予定（操作ごと。足りなければ巡回し、空なら入だけ）。
fn identity(
    ir: &Ir,
    initial: SessionState,
    specs: &[OpSpec],
    scheds: &[Sched],
) -> Result<Pair, TestCaseError> {
    let mut harness = Harness::via_session(ir, initial.clone());
    let mut pair = Pair::new(ir.clone(), initial.clone());
    for (i, spec) in specs.iter().enumerate() {
        let applied = harness.step(spec);
        pair.apply_op(applied.op.as_ref());
        pair.off_by_tick(&mut harness);
        pair.on_fast_forward();
        let sched = scheds
            .get(i % scheds.len().max(1))
            .cloned()
            .unwrap_or((false, Vec::new()));
        pair.mixed_run(&sched);
        pair.assert_same()?;
    }
    prop_assert_eq!(pair.on.session().state(), harness.state());
    // `ops` だけから決定的に同じ状態へ戻せる（切り替えの痕跡に依らない）。
    let replayed = replay(ir, initial, pair.mixed.ops()).expect("replay");
    prop_assert_eq!(&replayed, pair.mixed.session().state());
    Ok(pair)
}

fn sample_ir() -> (Ir, SessionState) {
    let sample = load();
    let bytes = std::fs::read(testdata_dir().join(&sample.ir)).expect("read ir");
    (load_ir(&bytes).expect("load ir"), sample.initial)
}

/// 全 Step の `confirm` を `none` にした写し（連続適用が達成・遷移・次の Step の入場をまたぐ）。
fn confirm_none_ir() -> (Ir, SessionState) {
    let (mut ir, initial) = sample_ir();
    for n in ir.nodes.iter_mut() {
        if n.step.confirm.is_some() {
            n.step.confirm = Some(json!({ "kind": "none", "reason": "memo only" }));
        }
    }
    (ir, initial)
}

/// 切り替えの予定の生成器。区切りの長さは 0〜3、区切りは 0〜2 個、予定は 1〜7 操作分（巡回して使う）。
fn scheds() -> impl Strategy<Value = Vec<Sched>> {
    prop::collection::vec((any::<bool>(), prop::collection::vec(0u32..4, 0..3)), 1..8)
}

/// 性質テストの共通の実行。空振りでないこと（進行を連続適用した操作と、印が出た確認が 1 件以上）も確かめる。
fn run_property(
    name: &str,
    strategy: impl Strategy<Value = Vec<OpSpec>>,
    source: fn() -> (Ir, SessionState),
) {
    let (ir, initial) = source();
    let (active, marked, toggled) = (
        AtomicUsize::new(0),
        AtomicUsize::new(0),
        AtomicUsize::new(0),
    );
    check(name, DEFAULT_CASES, (strategy, scheds()), |(specs, sc)| {
        let pair = identity(&ir, initial.clone(), &specs, &sc)?;
        active.fetch_add(pair.ff_active, Ordering::Relaxed);
        marked.fetch_add(pair.marks_seen, Ordering::Relaxed);
        toggled.fetch_add(pair.toggled, Ordering::Relaxed);
        Ok(())
    });
    assert!(
        toggled.load(Ordering::Relaxed) > 0,
        "{name}: 1 つの処理中の区間で入と切の両方が進行を適用した例が無かった"
    );
    assert!(
        active.load(Ordering::Relaxed) > 0,
        "{name}: 連続適用が一度も進行を適用しなかった"
    );
    assert!(
        marked.load(Ordering::Relaxed) > 0,
        "{name}: 左の印が一度も出なかった"
    );
}

#[test]
fn on_off_identity_property() {
    run_property("fast-forward-identity", op_specs(), sample_ir);
}

#[test]
fn on_off_identity_property_guided() {
    run_property("fast-forward-identity-guided", guided_op_specs(), sample_ir);
}

#[test]
fn on_off_identity_property_with_reload() {
    run_property(
        "fast-forward-identity-with-reload",
        op_specs_with_reload(),
        sample_ir,
    );
}

#[test]
fn on_off_identity_property_guided_with_reload() {
    run_property(
        "fast-forward-identity-guided-with-reload",
        guided_op_specs_with_reload(),
        sample_ir,
    );
}

#[test]
fn on_off_identity_property_confirm_none() {
    run_property(
        "fast-forward-identity-confirm-none",
        guided_op_specs(),
        confirm_none_ir,
    );
}

/// 固定の操作列でも同じ性質が成り立つ。
#[test]
fn on_off_script_specs() {
    let (ir, initial) = sample_ir();
    let pair =
        identity(&ir, initial, &script_specs(), &[(false, vec![2])]).expect("identity holds");
    assert!(pair.ff_active > 0, "固定列で連続適用が進行を適用しなかった");
}

fn min_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

/// st-1 が `Entered`・処理中でない・世代 1 の状態。
fn entered() -> SessionState {
    let u = uid("main/st-1");
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    let mut el = Element::default();
    el.attrs.insert("kind".into(), "none".into());
    el.attrs.insert("state".into(), "created".into());
    run.world.insert("notes/draft".into(), el);
    SessionState {
        run,
        ..SessionState::default()
    }
}

/// st-1 を処理中（進行 5 件）にして、入・切の宛先・進行の数・左の印が一致する。
#[test]
fn on_off_fixed_st1() {
    let first = Op::Input(Input::Command("/make-note first-note".into()));
    let mut pair = Pair::new(min_ir(), entered());
    pair.apply_op(Some(&first));
    let mut guard = 0;
    while let Some(addr) = next_tick(pair.off.session().state()) {
        pair.off_tick(Op::Tick(addr.clone()));
        pair.off_ticks.push(addr);
        guard += 1;
        assert!(guard <= 5, "進行が 5 件で終わらない");
    }
    pair.on_fast_forward();
    pair.mixed_run(&(true, Vec::new()));
    pair.assert_same().expect("identity holds");
    assert_eq!(pair.on_ticks.len(), 5);
    let indexes: Vec<u32> = pair.on_ticks.iter().map(|t| t.index).collect();
    assert_eq!(indexes, vec![0, 1, 2, 3, 4]);
    assert_eq!(progress_count(&pair.on_events), 5);
    assert_eq!(progress_count(&pair.off_events), 5);
    assert!(!pair.on_proj.marks().is_empty());
}

/// st-1 を処理中（進行 5 件）にして、`ops` が入・切・mixed で一致する（受付 1 + 進行 5 の 6 件）。
#[test]
fn ops_identity_fixed_st1() {
    let first = Op::Input(Input::Command("/make-note first-note".into()));
    let mut pair = Pair::new(min_ir(), entered());
    pair.apply_op(Some(&first));
    pair.off_by_tick_all();
    pair.on_fast_forward();
    pair.mixed_run(&(false, vec![2]));
    pair.assert_same().expect("identity holds");
    assert_eq!(pair.on.ops().len(), 6);
    assert_eq!(pair.off.ops().len(), 6);
    assert_eq!(pair.mixed.ops().len(), 6);
}

/// 列の途中の切り替え 2 通り（切 2 件 → 入、入（上限 2）→ 切 → 入）でも `ops` は切だけの列と一致する。
#[test]
fn toggle_fixed_st1() {
    let first = Op::Input(Input::Command("/make-note first-note".into()));
    for sched in [(false, vec![2]), (true, vec![2, 1])] {
        let mut pair = Pair::new(min_ir(), entered());
        pair.apply_op(Some(&first));
        pair.off_by_tick_all();
        pair.on_fast_forward();
        pair.mixed_run(&sched);
        pair.assert_same().expect("identity holds");
        assert_eq!(pair.mixed.ops().len(), 6, "{sched:?}");
        assert_eq!(pair.mixed_ticks.len(), 5, "{sched:?}");
        assert_eq!(pair.toggled, 1, "{sched:?}");
    }
}

/// 処理中でないときの連続適用は何も記録せず、`ops` の件数を増やさない（早送りそのものは履歴に入れない。D-54）。
#[test]
fn fast_forward_when_idle_records_nothing() {
    let mut pair = Pair::new(min_ir(), entered());
    let before = pair.mixed.ops().len();
    pair.mixed_run(&(true, vec![1, 2]));
    assert_eq!(pair.mixed.ops().len(), before);
    assert!(pair.mixed.envelope_for_save().is_none());
}
