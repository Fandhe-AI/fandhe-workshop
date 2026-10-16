//! TASK-205 の契約適合試験（骨格は TASK-204 2/3、サンプルを使う本体は TASK-205 2/2）。
//!
//! 結線を担当するタスク（表示・保存・入力判定・案内）が、`apply()` を埋めたときの合否の基準である。
//! 呼び出し元は CI の `make test` のみで、サンプルは `testdata/contract/event-sequence.json`（版 1）。
//! 試験は 3 層に分ける。
//! 1. サンプルと EVT-6 の規則の表の照合（サンプル自体の値を検査する。常に通る）
//! 2. 実際の `apply()` の不変条件（EVT-22 の決定性、3 つの入口の一致、世代の単調性と増分の上限）。
//!    骨格でも結線後でも成り立つべき性質である
//! 3. 厳密な照合（連鎖して流し、各操作の状態とイベント列がサンプルと一致すること）。`apply()` が骨格の間は
//!    `e01` で必ず失敗するため `#[ignore]` とし、最後の分岐を埋めるタスクが外す。比較器そのものは、
//!    期待どおりの提供元と 1 か所壊した提供元で自己確認する（CI に載る）
//!
//! 手書きの `Op` 列の 2 試験は、サンプルが含まない `Control::Begin` を覆うために残す。

mod common;

use std::cell::Cell;
use std::path::PathBuf;

use common::contract_sample::{load, testdata_dir, Sample, SampleEntry};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{CheckpointId, Control, GenerationRule, Input, Op};
use fandhe_workshop_core::provider::StateChangeProvider;
use fandhe_workshop_core::session::{Engine, RunState, SessionState};
use fandhe_workshop_core::{apply, load_ir, Session};

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("read testdata")).expect("load ir")
}

fn ops() -> Vec<Op> {
    let uid = StepUid("main/st-1".to_string());
    vec![
        Op::Control(Control::Begin("start".into())),
        Op::Input(Input::Command("run".into())),
        Op::Input(Input::Cancel),
        Op::Tick(TickAddr {
            generation: Generation(1),
            step: uid.clone(),
            index: 0,
        }),
        Op::Control(Control::Stop),
        Op::Control(Control::GoTo(uid)),
        Op::Control(Control::Save),
        Op::Control(Control::Restore(CheckpointId(1))),
    ]
}

fn run_session(ir: &Ir) -> (SessionState, Vec<Vec<fandhe_workshop_core::event::Event>>) {
    let mut s = Session::new(ir.clone());
    let mut per_op = Vec::new();
    for op in ops() {
        let before = s.generation();
        let rule = op.generation_rule();
        let events = s.apply(op).to_vec();
        assert!(s.generation() >= before, "generation must not decrease");
        if rule == GenerationRule::Keep {
            assert_eq!(s.generation(), before);
        }
        per_op.push(events);
    }
    (s.into_state(), per_op)
}

#[test]
fn same_ops_give_same_result() {
    let ir = ir();
    assert_eq!(run_session(&ir), run_session(&ir));
}

#[test]
fn three_entry_points_agree_and_log_tail_matches() {
    let ir = ir();
    let engine = Engine::new(&ir);
    let mut free = SessionState::default();
    let mut via_provider = SessionState::default();
    for op in ops() {
        let len = free.log.len();
        let (next, events) = apply(&ir, free, op.clone());
        assert_eq!(&next.log[len..], events.as_slice());
        free = next;
        let (next, events_p) = engine.apply(via_provider, op);
        assert_eq!(events, events_p);
        via_provider = next;
    }
    let (session_state, _) = run_session(&ir);
    assert_eq!(free, via_provider);
    assert_eq!(free, session_state);
}

// ---- サンプルを使う部分 ----

/// サンプルの `ir` 欄から教材を読む（経路の組み立ては形の試験が `..` を拒否している部品と同じ）。
fn sample_ir(s: &Sample) -> Ir {
    let bytes = std::fs::read(testdata_dir().join(&s.ir)).expect("sample ir must be readable");
    load_ir(&bytes).expect("sample ir must load")
}

/// 世代を運ぶイベントの `new_generation`。2 件以上あれば panic する。
fn generation_event(id: &str, events: &[Event]) -> Option<Generation> {
    let mut found = events.iter().filter_map(|e| match e {
        Event::Stopped { new_generation, .. }
        | Event::Moved { new_generation, .. }
        | Event::Restored { new_generation, .. }
        | Event::Begun { new_generation, .. } => Some(*new_generation),
        _ => None,
    });
    let first = found.next();
    assert!(
        found.next().is_none(),
        "{id}: more than one generation event"
    );
    first
}

fn refused(events: &[Event]) -> bool {
    events.iter().any(|e| matches!(e, Event::Refused { .. }))
}

/// 世代の増分。減っていれば panic する。
fn delta(id: &str, before: &RunState, after: &RunState) -> u64 {
    assert!(
        after.generation >= before.generation,
        "{id}: generation decreased"
    );
    after.generation.0 - before.generation.0
}

fn without_generation(r: &RunState) -> RunState {
    let mut r = r.clone();
    r.generation = Generation(0);
    r
}

// 層 1: サンプルと規則の表（EVT-6）

#[test]
fn sample_evt6_labels_match_generation_rule() {
    for e in &load().entries {
        let (expected_no, rule) = match &e.op {
            Op::Input(_) => (1, GenerationRule::Keep),
            Op::Tick(_) => (2, GenerationRule::Keep),
            Op::Control(Control::Stop) => (3, GenerationRule::IncrementIfProcessing),
            Op::Control(Control::GoTo(_)) => (4, GenerationRule::IncrementOnSuccess),
            Op::Control(Control::Save) => (5, GenerationRule::Keep),
            Op::Control(Control::Restore(_)) => (6, GenerationRule::IncrementOnSuccess),
            other => panic!("{}: unexpected op in sample: {other:?}", e.id),
        };
        assert_eq!(e.evt6, expected_no, "{}: evt6 label", e.id);
        assert_eq!(e.op.generation_rule(), rule, "{}: generation_rule", e.id);
    }
}

#[test]
fn sample_generation_deltas_follow_evt6() {
    for e in &load().entries {
        let d = delta(&e.id, &e.before, &e.after);
        let carried = generation_event(&e.id, &e.events);
        match e.op.generation_rule() {
            GenerationRule::Keep => {
                assert_eq!(d, 0, "{}: Keep must not change generation", e.id);
                assert!(
                    carried.is_none(),
                    "{}: Keep carries no generation event",
                    e.id
                );
            }
            GenerationRule::IncrementIfProcessing => {
                if e.before.busy.is_some() {
                    assert_eq!(d, 1, "{}: stop while busy is +1", e.id);
                    assert_eq!(
                        carried,
                        Some(e.after.generation),
                        "{}: new_generation",
                        e.id
                    );
                } else {
                    assert_eq!(d, 0, "{}: stop while idle keeps generation", e.id);
                    assert!(carried.is_none(), "{}: no generation event", e.id);
                }
            }
            GenerationRule::IncrementOnSuccess => match (carried, refused(&e.events)) {
                (Some(g), false) => {
                    assert_eq!(d, 1, "{}: success is +1", e.id);
                    assert_eq!(g, e.after.generation, "{}: new_generation", e.id);
                }
                (None, true) => assert_eq!(d, 0, "{}: refusal keeps generation", e.id),
                other => panic!("{}: neither success nor refusal alone: {other:?}", e.id),
            },
            _ => panic!("{}: unknown generation rule", e.id),
        }
    }
}

#[test]
fn sample_save_keeps_run_and_restore_returns_snapshot() {
    let s = load();
    for e in &s.entries {
        match &e.op {
            Op::Control(Control::Save) => {
                assert_eq!(e.before, e.after, "{}: save keeps run state", e.id)
            }
            Op::Control(Control::Restore(id)) if !refused(&e.events) => {
                let saver = s
                    .entries
                    .iter()
                    .find(|x| {
                        x.events
                            .iter()
                            .any(|ev| matches!(ev, Event::CheckpointSaved { id: i, .. } if i == id))
                    })
                    .unwrap_or_else(|| panic!("{}: no save for {id:?}", e.id));
                assert_eq!(
                    without_generation(&e.after),
                    without_generation(&saver.before),
                    "{}: restore returns the saved copy",
                    e.id
                );
            }
            _ => {}
        }
    }
}

// 層 2: 実際の apply() の不変条件

fn run_all(ir: &Ir, s: &Sample, upto: usize) -> (SessionState, Vec<Vec<Event>>) {
    let mut state = s.initial.clone();
    let mut per_op = Vec::new();
    for e in &s.entries[..upto] {
        let (next, events) = apply(ir, state, e.op.clone());
        state = next;
        per_op.push(events);
    }
    (state, per_op)
}

#[test]
fn sample_ops_are_deterministic() {
    let s = load();
    let ir = sample_ir(&s);
    let n = s.entries.len();
    assert_eq!(run_all(&ir, &s, n), run_all(&ir, &s, n));
    // 途中の任意の位置でも、初期状態から流し直した結果と一致する（EVT-22）。
    let mut state = s.initial.clone();
    for (i, e) in s.entries.iter().enumerate() {
        state = apply(&ir, state, e.op.clone()).0;
        assert_eq!(state, run_all(&ir, &s, i + 1).0, "{}: replay differs", e.id);
    }
}

#[test]
fn sample_ops_agree_across_entry_points() {
    let s = load();
    let ir = sample_ir(&s);
    let engine = Engine::new(&ir);
    let mut free = s.initial.clone();
    let mut via_engine = s.initial.clone();
    let mut session = Session::from_state(ir.clone(), s.initial.clone());
    for e in &s.entries {
        let len = free.log.len();
        let (next, events) = apply(&ir, free, e.op.clone());
        assert_eq!(&next.log[len..], events.as_slice(), "{}: log tail", e.id);
        free = next;
        let (next, ev_engine) = engine.apply(via_engine, e.op.clone());
        via_engine = next;
        assert_eq!(events, ev_engine, "{}: engine events", e.id);
        let ev_session = session.apply(e.op.clone()).to_vec();
        assert_eq!(events, ev_session, "{}: session events", e.id);
    }
    assert_eq!(free, via_engine);
    assert_eq!(free, session.into_state());
}

#[test]
fn sample_ops_respect_generation_bounds() {
    let s = load();
    let ir = sample_ir(&s);
    let mut state = s.initial.clone();
    for e in &s.entries {
        let before = state.run.clone();
        let (next, events) = apply(&ir, state, e.op.clone());
        let d = delta(&e.id, &before, &next.run);
        let carried = generation_event(&e.id, &events);
        match e.op.generation_rule() {
            GenerationRule::Keep => assert_eq!(d, 0, "{}: Keep", e.id),
            _ => assert!(d <= 1, "{}: increment is at most 1", e.id),
        }
        if d == 1 {
            assert_eq!(
                carried,
                Some(next.run.generation),
                "{}: new_generation",
                e.id
            );
        } else {
            assert!(carried.is_none(), "{}: unexpected generation event", e.id);
        }
        state = next;
    }
}

// 層 3: 厳密な照合（結線の合否）

/// 最初の食い違い。
#[derive(Debug)]
struct Mismatch {
    id: String,
    what: &'static str,
    note: String,
    expected: String,
    actual: String,
}

fn check<T: PartialEq + std::fmt::Debug>(
    e: &SampleEntry,
    what: &'static str,
    expected: &T,
    actual: &T,
) -> Result<(), Mismatch> {
    if expected == actual {
        return Ok(());
    }
    Err(Mismatch {
        id: e.id.clone(),
        what,
        note: e.note.clone(),
        expected: format!("{expected:#?}"),
        actual: format!("{actual:#?}"),
    })
}

/// サンプルの先頭 `upto` 件を連鎖して流し、各操作の前後の状態・イベント列・ログの末尾を照合する。
fn replay_against_upto<P>(provider: &P, s: &Sample, upto: usize) -> Result<(), Mismatch>
where
    P: StateChangeProvider<State = SessionState, Input = Op, Event = Event>,
{
    let mut state = s.initial.clone();
    for e in s.entries.iter().take(upto) {
        check(e, "before", &e.before, &state.run)?;
        let len = state.log.len();
        let (next, events) = provider.apply(state, e.op.clone());
        check(e, "events", &e.events, &events)?;
        check(e, "after", &e.after, &next.run)?;
        check(e, "log", &events, &next.log[len..].to_vec())?;
        state = next;
    }
    Ok(())
}

/// サンプルの全件を照合する。
fn replay_against<P>(provider: &P, s: &Sample) -> Result<(), Mismatch>
where
    P: StateChangeProvider<State = SessionState, Input = Op, Event = Event>,
{
    replay_against_upto(provider, s, s.entries.len())
}

/// 入力の受付（TASK-207 1/3）と一致する進行（TASK-210 2/3）まで結線した範囲の照合。先頭 5 件（e01 拒否・e02 受付・
/// e03 再送の拒否・e04 と e05 の進行）は実際の `apply()` と一致する。最初の食い違いは e06（停止、TASK-211）で、
/// 全件の照合は下の `#[ignore]` の試験が受け持つ。
#[test]
fn sample_replay_prefix_matches_apply() {
    let s = load();
    let ir = sample_ir(&s);
    if let Err(m) = replay_against_upto(&Engine::new(&ir), &s, 5) {
        panic!(
            "{}: {} differs ({})\nexpected: {}\nactual: {}",
            m.id, m.what, m.note, m.expected, m.actual
        );
    }
}

/// 確認の操作による達成（TASK-207 2/3）の単独の照合。サンプルの e15（受付済みの Step を指して達成し、次の Step に
/// 入る）を、その前の状態から 1 件だけ流して、イベント列と後の状態が一致することを確かめる。連鎖では
/// e06（停止、TASK-211）で止まり e15 に届かないため、前の状態から直接始める。
#[test]
fn sample_confirm_entry_matches_apply() {
    let s = load();
    let ir = sample_ir(&s);
    let e = s
        .entries
        .iter()
        .find(|e| e.id == "e15")
        .expect("sample has e15");
    let state = SessionState {
        run: e.before.clone(),
        ..SessionState::default()
    };
    let (next, events) = apply(&ir, state, e.op.clone());
    assert_eq!(events, e.events, "e15: events");
    assert_eq!(next.run, e.after, "e15: after");
}

/// 一致する進行（TASK-210 2/3）の単独の照合。サンプルの e09〜e13（世代 2 の st-1 の進行 5 つ。最後の進行で
/// 処理中が解け、達成はしない）を、e09 の前の状態から連鎖して流し、各操作のイベント列と後の状態が一致することを
/// 確かめる。連鎖では e06（停止、TASK-211）で止まり届かないため、前の状態から直接始める。
#[test]
fn sample_ticks_entry_matches_apply() {
    let s = load();
    let ir = sample_ir(&s);
    let ids = ["e09", "e10", "e11", "e12", "e13"];
    let first = s
        .entries
        .iter()
        .find(|e| e.id == ids[0])
        .expect("sample has e09");
    let mut state = SessionState {
        run: first.before.clone(),
        ..SessionState::default()
    };
    for id in ids {
        let e = s
            .entries
            .iter()
            .find(|e| e.id == id)
            .expect("sample has entry");
        assert_eq!(state.run, e.before, "{id}: before");
        let (next, events) = apply(&ir, state, e.op.clone());
        assert_eq!(events, e.events, "{id}: events");
        assert_eq!(next.run, e.after, "{id}: after");
        state = next;
    }
    assert_eq!(state.run.busy, None);
}

/// 確認を省く明示の Step の受付と同時の達成（TASK-208）の単独の照合。サンプルの e16 を、その前の状態から
/// 1 件だけ流して、イベント列と後の状態が一致することを確かめる（e15 と同じ理由で連鎖では届かない）。
#[test]
fn sample_skip_confirm_entry_matches_apply() {
    let s = load();
    let ir = sample_ir(&s);
    let e = s
        .entries
        .iter()
        .find(|e| e.id == "e16")
        .expect("sample has e16");
    let state = SessionState {
        run: e.before.clone(),
        ..SessionState::default()
    };
    let (next, events) = apply(&ir, state, e.op.clone());
    assert_eq!(events, e.events, "e16: events");
    assert_eq!(next.run, e.after, "e16: after");
}

#[test]
#[ignore = "apply() の各分岐（TASK-207・210・211・213・214・215）の結線後に外す。結線の確認は --include-ignored で流す"]
fn sample_replay_matches_apply() {
    let s = load();
    let ir = sample_ir(&s);
    if let Err(m) = replay_against(&Engine::new(&ir), &s) {
        panic!(
            "{}: {} differs ({})\nexpected: {}\nactual: {}",
            m.id, m.what, m.note, m.expected, m.actual
        );
    }
}

type Perturb = fn(&str, &mut RunState, &mut Vec<Event>);

/// サンプルどおりに答える提供元。細工の関数で 1 か所だけ壊せる。
struct SampleReplay<'a> {
    sample: &'a Sample,
    calls: Cell<usize>,
    perturb: Perturb,
}

impl StateChangeProvider for SampleReplay<'_> {
    type State = SessionState;
    type Input = Op;
    type Event = Event;

    fn apply(&self, mut state: SessionState, _input: Op) -> (SessionState, Vec<Event>) {
        let e = &self.sample.entries[self.calls.get()];
        self.calls.set(self.calls.get() + 1);
        let mut run = e.after.clone();
        let mut events = e.events.clone();
        (self.perturb)(&e.id, &mut run, &mut events);
        state.run = run;
        state.log.extend(events.iter().cloned());
        (state, events)
    }
}

fn replay_with(perturb: Perturb) -> Result<(), Mismatch> {
    let s = load();
    let p = SampleReplay {
        sample: &s,
        calls: Cell::new(0),
        perturb,
    };
    replay_against(&p, &s)
}

#[test]
fn replay_accepts_the_sample_itself() {
    replay_with(|_, _, _| {}).expect("the sample must match itself");
}

#[test]
fn replay_reports_the_first_mismatch() {
    let m = replay_with(|id, _, events| {
        if id == "e06" {
            events.pop();
        }
    })
    .expect_err("dropped event must be reported");
    assert_eq!((m.id.as_str(), m.what), ("e06", "events"));

    let m = replay_with(|id, run, _| {
        if id == "e19" {
            run.generation = Generation(run.generation.0 + 1);
        }
    })
    .expect_err("changed generation must be reported");
    assert_eq!((m.id.as_str(), m.what), ("e19", "after"));
}
