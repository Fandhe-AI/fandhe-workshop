//! 1 回の操作で起きる内部の遷移の上限の試験（TASK-160 1/3、仕様 SAFE-23）。
//!
//! 作成者向けの検査を通さずに配られた巡回する IR（`testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/`）で、
//! 1,000 回に達した時点で止まり、`Warning` が 1 件だけ残り、現在の Step が空になって以降の入力が落ちずに無視されることを確かめる。
//! 巡回しない長い連鎖は、1,000 回ちょうどなら通り、1,001 回で止まる（境界）。再帰を使っていないことは小さなスタックで確かめる。
//! 開始（`Control::Begin`）は未実装（TASK-217）なので、`enter` の巡回は `Control::GoTo` で入口へ入って代わりにする。
//! TASK-160 2/3（SAFE-34）で、止めたときに専用の `TransitionLimitReached` が `Warning` の直後に 1 件だけ残り、
//! 着地の回数が上限と一致することを足した（止まらないときは 0 件）。
//! 呼び出し元は CI の `make test` と変異注入（`tests/mutations/SAFE23-*.mutation`）である。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Node, Transition};
use fandhe_workshop_contract::limits::MAX_TRANSITIONS_PER_INPUT;
use fandhe_workshop_core::addr::{StepUid, TickAddr};
use fandhe_workshop_core::event::{Cause, Event};
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::projection::Projection;
use fandhe_workshop_core::session::{Busy, EntryCopy, SessionState};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};

fn cycle_ir(name: &str) -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir")
        .join(format!("{name}.json"));
    load_ir(&std::fs::read(p).expect("read cycle ir")).expect("cycle ir must load")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

/// `at` を現在の Step として入力待ち（`Entered`）にした状態。
fn state_at(at: &str) -> SessionState {
    let mut s = SessionState::default();
    let u = uid(at);
    s.run.current = Some(u.clone());
    s.run
        .slices
        .entry(u.root())
        .or_default()
        .status
        .insert(u, StepStatus::Entered);
    s
}

fn count(events: &[Event], f: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}

fn warnings(events: &[Event]) -> usize {
    count(events, |e| matches!(e, Event::Warning { .. }))
}

/// 上限で止めたときの `Warning` だけを数える（効果の適用の `Warning` は数えない）。
fn limit_warnings(events: &[Event]) -> usize {
    count(
        events,
        |e| matches!(e, Event::Warning { message, .. } if message.contains("per-input limit")),
    )
}

/// 受講者向けの専用のイベント（SAFE-34）の着地の回数。止めたときの 1 件だけを取り出す。
fn limit_landings(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::TransitionLimitReached { landings, .. } => Some(*landings),
            _ => None,
        })
        .collect()
}

/// 止めたとき、専用のイベントが 1 件で、作成者向けの `Warning` の直後・イベント列の末尾にあることを確かめる。
fn assert_limit_event(events: &[Event]) {
    assert_eq!(
        limit_landings(events),
        vec![u32::try_from(MAX_TRANSITIONS_PER_INPUT).expect("fits")],
        "{events:?}"
    );
    let n = events.len();
    assert!(n >= 2, "{events:?}");
    assert!(
        matches!(&events[n - 2], Event::Warning { message, .. } if message.contains("per-input limit")),
        "{events:?}"
    );
    assert!(
        matches!(&events[n - 1], Event::TransitionLimitReached { .. }),
        "{events:?}"
    );
}

fn entered(events: &[Event]) -> usize {
    count(events, |e| matches!(e, Event::ModuleEntered { .. }))
}

fn exited(events: &[Event]) -> usize {
    count(events, |e| matches!(e, Event::ModuleExited { .. }))
}

fn text() -> Op {
    Op::Input(Input::Text("go".into()))
}

fn go_to(s: &str) -> Op {
    Op::Control(Control::GoTo(uid(s)))
}

fn node(ir: &Ir, uid: &str) -> Node {
    ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .cloned()
        .expect("node exists")
}

/// 止まった状態で、続く入力が落ちずに無視される（空のイベントが返る）ことを確かめる。
fn assert_halted(state: &SessionState, ir: &Ir) {
    assert_eq!(state.run.current, None);
    assert_eq!(state.run.busy, None);
    assert!(state.entry.is_none());
    let (next, events) = apply(ir, state.clone(), text());
    assert!(events.is_empty(), "{events:?}");
    assert_eq!(next.run.current, None);
}

/// 上限で止めた後、操作の前の状態へ戻ったこと（SAFE-35）。`moved` は世代が +1 される操作（`go_to`）か。
fn assert_returned(state: &SessionState, before: &SessionState, moved: bool, events: &[Event]) {
    assert_eq!(state.run.current, before.run.current);
    assert_eq!(state.run.busy, None);
    assert_eq!(state.entry, before.entry);
    let mut expected = before.run.clone();
    if moved {
        expected.generation = expected.generation.next().expect("room");
    }
    assert_eq!(state.run, expected);
    // 世界状態が操作の前に戻るので、`Changed` は出ない。
    assert_eq!(count(events, |e| matches!(e, Event::Changed { .. })), 0);
}

/// 戻った Step への次の入力が受け付けられ、受付のイベントが増えることを確かめる（繰り返し戻れることも含む）。
fn assert_next_input_received(ir: &Ir, state: &SessionState, origin: &str) {
    let (_, events) = apply(ir, state.clone(), text());
    assert!(!events.is_empty(), "input must not be ignored");
    assert!(
        matches!(&events[0], Event::InputReceived { at, .. } if at.step == uid(origin)),
        "{events:?}"
    );
}

#[test]
fn cycle_enter_self_stops_at_limit() {
    let ir = cycle_ir("cycle-enter-self");
    let before = state_at("main/st-1");
    let (state, events) = apply(&ir, before.clone(), go_to("main/u-self"));
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_limit_event(&events);
    assert_returned(&state, &before, true, &events);
    assert_next_input_received(&ir, &state, "main/st-1");
}

#[test]
fn cycle_enter_2_stops_at_limit() {
    let ir = cycle_ir("cycle-enter-2");
    let before = state_at("main/st-1");
    let (state, events) = apply(&ir, before.clone(), go_to("main/u-a"));
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_limit_event(&events);
    assert_returned(&state, &before, true, &events);
    assert_next_input_received(&ir, &state, "main/st-1");
}

#[test]
fn cycle_exit_2_stops_at_limit() {
    let ir = cycle_ir("cycle-exit-2");
    let before = state_at("main/f1");
    let (state, events) = apply(&ir, before.clone(), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_eq!(count(&events, |e| matches!(e, Event::Achieved { .. })), 1);
    assert_limit_event(&events);
    // 止めた操作の達成は状態に残らない（f1 は Entered のまま）ので、次の入力は再び受け付けられる。
    assert_returned(&state, &before, false, &events);
    assert_next_input_received(&ir, &state, "main/f1");
    // 復習も戻る。止めた操作の `Achieved` がログに残っても、f1 は未達成のまま。
    let projection = Projection::from_events(&state.log);
    assert!(
        projection
            .review()
            .iter()
            .all(|r| r.step != uid("main/f1") || !r.achieved),
        "{:?}",
        projection.review()
    );
}

/// 巡回していても 1 回の入力ごとの遷移が少数なら、上限に当たらない対照。
#[test]
fn cycle_d07_flat_stays_under_limit() {
    let ir = cycle_ir("cycle-d07-flat");
    let mut state = state_at("impl-run@round1/merge-gate@gate/st-7-4");
    let ops = [
        Op::Input(Input::Ui {
            view: "v-pr-cards".into(),
            path: "prs/pr-1-a#merge".into(),
        }),
        Op::Input(Input::Ui {
            view: "v-issue-tree".into(),
            path: "issues/child-1".into(),
        }),
    ];
    let mut all = Vec::new();
    for op in ops {
        let (next, events) = apply(&ir, state, op);
        state = next;
        all.extend(events);
    }
    assert_eq!(limit_warnings(&all), 0, "{all:?}");
    assert!(limit_landings(&all).is_empty(), "{all:?}");
    assert!(state.run.current.is_some());
    assert!(entered(&all) >= 1, "{all:?}");
    assert!(entered(&all) < 10, "{all:?}");
}

/// `use` の連鎖（`enter`）。入口 → u-0 … u-(k-1) → 末尾で、着地は k + 1 回。
fn enter_chain(k: usize) -> Ir {
    let mut ir = cycle_ir("cycle-exit-2");
    let mut entry = node(&ir, "main/f1");
    let tail = node(&ir, "main/st-3");
    let mut user = node(&cycle_ir("cycle-enter-self"), "main/u-self");
    user.step.next.clear();
    entry.uid = "main/e".into();
    entry.exit_to = None;
    entry.step.next = vec![Transition {
        to: if k == 0 {
            "main/tail".into()
        } else {
            "main/u-0".into()
        },
        when: None,
    }];
    let mut nodes = vec![entry];
    for i in 0..k {
        let mut u = user.clone();
        u.uid = format!("main/u-{i}");
        u.enter = Some(if i + 1 == k {
            "main/tail".into()
        } else {
            format!("main/u-{}", i + 1)
        });
        nodes.push(u);
    }
    let mut t = tail;
    t.uid = "main/tail".into();
    nodes.push(t);
    ir.nodes = nodes;
    ir
}

/// `final` の連鎖（`exit_to`）。c0 → c1 … ck → 末尾で、戻りの着地は k + 1 回。
fn exit_chain(k: usize) -> Ir {
    let mut ir = cycle_ir("cycle-exit-2");
    let tail = node(&ir, "main/st-3");
    let fin = node(&ir, "main/f1");
    let mut nodes = Vec::new();
    for i in 0..=k {
        let mut f = fin.clone();
        f.uid = format!("main/c{i}");
        f.exit_to = Some(if i == k {
            "main/tail".into()
        } else {
            format!("main/c{}", i + 1)
        });
        nodes.push(f);
    }
    let mut t = tail;
    t.uid = "main/tail".into();
    nodes.push(t);
    ir.nodes = nodes;
    ir
}

#[test]
fn enter_chain_boundary() {
    let ok = MAX_TRANSITIONS_PER_INPUT - 1;
    let ir = enter_chain(ok);
    let (state, events) = apply(&ir, state_at("main/e"), text());
    assert_eq!(warnings(&events), 0, "{events:?}");
    assert!(limit_landings(&events).is_empty(), "{events:?}");
    assert_eq!(state.run.current, Some(uid("main/tail")));
    assert_eq!(entered(&events), ok);

    let ir = enter_chain(ok + 1);
    let before = state_at("main/e");
    let (state, events) = apply(&ir, before.clone(), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), ok + 1);
    assert_limit_event(&events);
    assert_returned(&state, &before, false, &events);
    assert_next_input_received(&ir, &state, "main/e");
}

#[test]
fn exit_chain_boundary() {
    // c0 の達成の後の戻りは c1..ck と末尾の k + 1 回。k = 999 で 1,000 回ちょうど。
    let ok = MAX_TRANSITIONS_PER_INPUT - 1;
    let ir = exit_chain(ok);
    let (state, events) = apply(&ir, state_at("main/c0"), text());
    assert_eq!(warnings(&events), 0, "{events:?}");
    assert!(limit_landings(&events).is_empty(), "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    // 末尾まで戻った後の現在位置は、最後の復帰先（呼び出し先の final Step のままにしない）。
    assert_eq!(state.run.current, Some(uid("main/tail")));

    let ir = exit_chain(ok + 1);
    let before = state_at("main/c0");
    let (state, events) = apply(&ir, before.clone(), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_limit_event(&events);
    assert_returned(&state, &before, false, &events);
    assert_next_input_received(&ir, &state, "main/c0");
}

/// 歩行器が再帰ならあふれる小さなスタックで巡回を流す。
#[test]
fn small_stack_does_not_overflow() {
    let h = std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(|| {
            let ir = cycle_ir("cycle-enter-self");
            let (_, events) = apply(&ir, state_at("main/st-1"), go_to("main/u-self"));
            assert_eq!(warnings(&events), 1);
            let ir = cycle_ir("cycle-exit-2");
            let (_, events) = apply(&ir, state_at("main/f1"), text());
            assert_eq!(warnings(&events), 1);
        })
        .expect("spawn");
    h.join().expect("no overflow");
}

/// 連鎖の途中の `enter` の先が IR に無いときは、固定文の `Warning` が 1 件で、panic しない。
#[test]
fn unknown_enter_target_warns() {
    let mut ir = enter_chain(3);
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/u-1") {
        n.enter = Some("main/nowhere".into());
    }
    let (_, events) = apply(&ir, state_at("main/e"), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert!(limit_landings(&events).is_empty(), "{events:?}");
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::Warning { message, .. } if message.contains("nowhere"))));
}

/// 移動先の `enter` が IR に無いとき、`current` を移動元に残さず、止まった状態に確定する。
#[test]
fn unknown_enter_target_halts_state_on_go_to() {
    let mut ir = enter_chain(3);
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/u-1") {
        n.enter = Some("main/nowhere".into());
    }
    let (next, events) = apply(&ir, state_at("main/e"), go_to("main/u-1"));
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_halted(&next, &ir);
}

/// 止めた後に戻った Step への次の入力は `Accepted` になる。止めた操作の達成が状態に残らないため
/// 再び達成まで進み、同じ巡回で再び止まって戻る（繰り返し戻れる、SAFE-35）。
#[test]
fn cycle_exit_2_accepts_again_and_returns_again() {
    let ir = cycle_ir("cycle-exit-2");
    let (state, _) = apply(&ir, state_at("main/f1"), text());
    let (state2, events) = apply(&ir, state, text());
    assert_eq!(
        count(&events, |e| matches!(e, Event::Accepted { .. })),
        1,
        "{events:?}"
    );
    assert_limit_event(&events);
    assert_eq!(state2.run.current, Some(uid("main/f1")));
    assert_eq!(state2.run.busy, None);
}

/// 進行つきの Step を受付済み・処理中にした状態。`with_entry` なら入口の写し（Entered）を持つ。
fn busy_state(with_entry: bool) -> (Ir, SessionState, TickAddr) {
    let mut ir = cycle_ir("cycle-exit-2");
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/f1") {
        n.step.processing = vec![serde_json::json!({ "name": "p1", "effects": [] })];
    }
    let mut before = state_at("main/f1");
    before
        .run
        .slices
        .entry(uid("main/f1").root())
        .or_default()
        .status
        .insert(uid("main/f1"), StepStatus::Accepted);
    before.run.busy = Some(Busy { done: 0, total: 1 });
    if with_entry {
        // 入口の写しは入力を受け付け直せる状態（Entered）。進行からの復帰はここへ戻る。
        before.entry = Some(EntryCopy {
            run: state_at("main/f1").run,
            log_len: 0,
        });
    }
    let addr = TickAddr {
        generation: before.run.generation,
        step: uid("main/f1"),
        index: 0,
    };
    (ir, before, addr)
}

/// 進行（`Tick`）の完了で巡回に入って止めた場合も、`busy` は空で戻る。世代は +1 になり、復帰前の宛先は
/// 再入力で同じ Step が処理中になった後も照合を通らない。止める・戻すの無限の繰り返しにもならない。
#[test]
fn tick_path_returns_without_busy_and_does_not_loop() {
    let (ir, before, addr) = busy_state(true);
    let (state, events) = apply(&ir, before.clone(), Op::Tick(addr.clone()));
    assert_limit_event(&events);
    assert_eq!(state.run.current, Some(uid("main/f1")));
    assert_eq!(state.run.busy, None);
    assert_eq!(
        state.run.slices[&uid("main/f1").root()].status[&uid("main/f1")],
        StepStatus::Entered
    );
    assert_eq!(
        state.run.generation,
        before.run.generation.next().expect("room")
    );
    // 再入力で同じ Step が処理中になっても、復帰前の宛先は無視される。
    let mut again_busy = state.clone();
    again_busy.run.busy = Some(Busy { done: 0, total: 1 });
    let (_, again) = apply(&ir, again_busy, Op::Tick(addr.clone()));
    assert!(limit_landings(&again).is_empty(), "{again:?}");
    assert_eq!(
        count(&again, |e| matches!(e, Event::Ignored { .. })),
        1,
        "{again:?}"
    );
    let (_, again) = apply(&ir, state, Op::Tick(addr));
    assert_eq!(
        count(&again, |e| matches!(e, Event::Ignored { .. })),
        1,
        "{again:?}"
    );
}

/// 本番の経路のように入口の写しが無くても、処理中の上限停止で学習者が現在の Step を失わない。操作の前の
/// `run` から `busy` だけを空にして戻り、世代は +1 になり、保存と移動が働く。
#[test]
fn busy_limit_without_entry_copy_keeps_current_step() {
    let (ir, before, addr) = busy_state(false);
    assert!(before.entry.is_none());
    let (state, events) = apply(&ir, before.clone(), Op::Tick(addr.clone()));
    assert_limit_event(&events);
    assert_eq!(state.run.current, Some(uid("main/f1")));
    assert_eq!(state.run.busy, None);
    assert_eq!(
        state.run.generation,
        before.run.generation.next().expect("room")
    );
    let (saved, saved_events) = apply(&ir, state.clone(), Op::Control(Control::Save));
    assert!(!saved_events.is_empty());
    assert_eq!(saved.checkpoints.len(), 1);
    let (moved, moved_events) = apply(&ir, state.clone(), go_to("main/f1"));
    assert!(!moved_events.is_empty());
    assert_eq!(moved.run.current, Some(uid("main/f1")));
    // 復帰前の宛先は無効。
    let mut again_busy = state;
    again_busy.run.busy = Some(Busy { done: 0, total: 1 });
    let (_, again) = apply(&ir, again_busy, Op::Tick(addr));
    assert_eq!(
        count(&again, |e| matches!(e, Event::Ignored { .. })),
        1,
        "{again:?}"
    );
}

/// 処理中に保存して復帰し、最後の進行で上限に達したとき、core は入口（受付の前）へ戻り、復習もその時点へ戻る
/// （SAFE-35）。受付で x を置き、最初の進行で x を消すので、入口と止めた時点の世界状態は等しく、取り消しの
/// `Changed` は出ない。core はログの `Restored` から保存の時点へ起点を移して受付の時点を導き、導出（`Projection`）
/// は差分に頼らず、保存時の入口の控えを復帰で戻して揃える。
#[test]
fn busy_restore_then_limit_rewinds_review_to_entry() {
    let mut ir = cycle_ir("cycle-exit-2");
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/f1") {
        n.step.rules = vec![serde_json::json!({
            "id": "r-put",
            "on": { "text": true },
            "outcome": { "kind": "accept", "effects": [{ "put": { "path": "x", "fixture": "fx" } }] }
        })];
        n.step.processing = vec![
            serde_json::json!({ "name": "p1", "effects": [{ "remove": "x" }] }),
            serde_json::json!({ "name": "p2", "effects": [] }),
        ];
    }
    let tick = |state: &SessionState| {
        let busy = state.run.busy.clone().expect("busy");
        Op::Tick(TickAddr {
            generation: state.run.generation,
            step: uid("main/f1"),
            index: busy.done,
        })
    };
    let (state, _) = apply(&ir, state_at("main/f1"), text());
    assert!(state.run.world.contains_key("x"), "{:?}", state.run.world);
    // 入口の写しは状態に書かない（受付の時点はログから導く。履歴の写しの形を main と同じに保つ）。
    assert!(state.entry.is_none());
    let (state, _) = apply(&ir, state.clone(), tick(&state));
    assert!(!state.run.world.contains_key("x"));
    let (state, saved) = apply(&ir, state, Op::Control(Control::Save));
    let id = match saved.as_slice() {
        [Event::CheckpointSaved { id, .. }] => *id,
        other => panic!("save must succeed: {other:?}"),
    };
    let (state, restored) = apply(&ir, state, Op::Control(Control::Restore(id)));
    assert!(
        matches!(restored.as_slice(), [Event::Restored { .. }]),
        "{restored:?}"
    );
    assert!(state.run.busy.is_some());
    let (state, events) = apply(&ir, state.clone(), tick(&state));
    assert_limit_event(&events);
    assert_eq!(count(&events, |e| matches!(e, Event::Changed { .. })), 0);
    assert_eq!(state.run.current, Some(uid("main/f1")));
    assert_eq!(state.run.busy, None);
    assert_eq!(
        state.run.slices[&uid("main/f1").root()].status[&uid("main/f1")],
        StepStatus::Entered
    );
    let p = Projection::from_events(&state.log);
    assert!(p.marks().is_empty(), "{:?}", p.marks());
    assert!(
        p.review()
            .iter()
            .all(|r| r.changes.is_empty() && !r.achieved),
        "{:?}",
        p.review()
    );
}

/// 止めて戻った後も、保存などの操作がふつうに働き、同じ操作の列は同じ状態になる（決定的）。
#[test]
fn controls_work_after_return_and_replay_is_deterministic() {
    let ir = cycle_ir("cycle-enter-2");
    let run = |ops: &[Op]| {
        let mut state = state_at("main/st-1");
        for op in ops {
            state = apply(&ir, state, op.clone()).0;
        }
        state
    };
    let ops = [go_to("main/u-a"), Op::Control(Control::Save)];
    let a = run(&ops);
    assert_eq!(a.run.current, Some(uid("main/st-1")));
    assert_eq!(a.checkpoints.len(), 1);
    assert_eq!(a, run(&ops));
}

/// 入口へ戻して世界状態が変わる止め方では、`apply` が取り消しの `Changed` を末尾に足す（`record_changes`）。
/// 専用のイベントが 1 件で、その後ろが `Changed` だけであることを確かめる。
fn assert_limit_then_changes(events: &[Event]) {
    assert_eq!(
        limit_landings(events),
        vec![u32::try_from(MAX_TRANSITIONS_PER_INPUT).expect("fits")],
        "{events:?}"
    );
    let at = events
        .iter()
        .position(|e| matches!(e, Event::TransitionLimitReached { .. }))
        .expect("limit event");
    assert!(
        events[at + 1..]
            .iter()
            .all(|e| matches!(e, Event::Changed { .. })),
        "{events:?}"
    );
}

/// cycle-exit-2 の f1（確認を省く Step。達成の後の遷移が巡回して上限に達する）を、自然文の受付で
/// `accept_effects` を当て、`processing` の進行を持つ Step にした教材。
fn processing_ir(accept_effects: serde_json::Value, processing: Vec<serde_json::Value>) -> Ir {
    let mut ir = cycle_ir("cycle-exit-2");
    if let Some(n) = ir.nodes.iter_mut().find(|n| n.uid == "main/f1") {
        n.step.rules = vec![serde_json::json!({
            "id": "r-accept",
            "on": { "text": true },
            "outcome": { "kind": "accept", "effects": accept_effects }
        })];
        n.step.processing = processing;
    }
    ir
}

/// 処理中の現在の Step への、次に来るべき進行。
fn next_tick(state: &SessionState) -> Op {
    let busy = state.run.busy.clone().expect("busy");
    Op::Tick(TickAddr {
        generation: state.run.generation,
        step: uid("main/f1"),
        index: busy.done,
    })
}

fn f1_status(state: &SessionState) -> StepStatus {
    state.run.slices[&uid("main/f1").root()].status[&uid("main/f1")]
}

/// 本番の経路（`apply` の受付から処理中になる。状態の入口の写しは空のまま）で、最後の進行から上限に達しても、
/// ログから導いた受付の時点へ戻り、入力を受け付け直せる（解決済みの P1「入口の写しが無い処理中の Step も入力を
/// 受け付け直せる状態へ戻す」の再発防止）。受付と進行の世界状態の変化は取り消され、取り消しは原因 `Stop` の
/// `Changed` になる。受け付け直した後も、同じ導き方で再び戻れる。
#[test]
fn busy_limit_rewinds_to_accept_derived_from_log() {
    let ir = processing_ir(
        serde_json::json!([{ "put": { "path": "x", "fixture": "fx" } }]),
        vec![
            serde_json::json!({ "name": "p1", "effects": [{ "put": { "path": "y", "fixture": "fy" } }] }),
        ],
    );
    let entered = state_at("main/f1");
    let (accepted, _) = apply(&ir, entered.clone(), text());
    assert!(accepted.run.busy.is_some());
    assert_eq!(accepted.entry, None);
    let (state, events) = apply(&ir, accepted.clone(), next_tick(&accepted));
    assert_limit_then_changes(&events);
    assert_eq!(state.entry, None);
    assert_eq!(state.run.current, Some(uid("main/f1")));
    assert_eq!(state.run.busy, None);
    assert_eq!(f1_status(&state), StepStatus::Entered);
    assert_eq!(state.run.world, entered.run.world);
    assert_eq!(
        state.run.generation,
        accepted.run.generation.next().expect("room")
    );
    let stops: Vec<&str> = events
        .iter()
        .filter_map(|e| match e {
            Event::Changed { path, cause, .. } if *cause == Cause::Stop => Some(path.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(stops, vec!["x"], "{events:?}");
    let p = Projection::from_events(&state.log);
    assert!(p.marks().is_empty(), "{:?}", p.marks());
    assert!(
        p.review()
            .iter()
            .all(|r| r.changes.is_empty() && !r.achieved),
        "{:?}",
        p.review()
    );
    // 受け付け直せる。
    let (again, events) = apply(&ir, state, text());
    assert_eq!(
        count(&events, |e| matches!(e, Event::Accepted { .. })),
        1,
        "{events:?}"
    );
    assert!(again.run.busy.is_some());
    let (again, events) = apply(&ir, again.clone(), next_tick(&again));
    assert_limit_then_changes(&events);
    assert_eq!(f1_status(&again), StepStatus::Entered);
    assert_eq!(again.run.world, entered.run.world);
}

/// 処理中に保存し、先へ進めてから復帰し、停止したとき、core はログの `Restored` から保存の時点へ起点を移して
/// 受付の時点を導き、受付の前の状態へ戻る（状態の入口の写しは空のまま）。
#[test]
fn busy_restore_then_stop_returns_to_accept() {
    let ir = processing_ir(
        serde_json::json!([{ "put": { "path": "x", "fixture": "fx" } }]),
        vec![
            serde_json::json!({ "name": "p1", "effects": [{ "put": { "path": "y", "fixture": "fy" } }] }),
            serde_json::json!({ "name": "p2", "effects": [{ "put": { "path": "z", "fixture": "fz" } }] }),
            serde_json::json!({ "name": "p3", "effects": [] }),
        ],
    );
    let entered = state_at("main/f1");
    let (state, _) = apply(&ir, entered.clone(), text());
    let (state, _) = apply(&ir, state.clone(), next_tick(&state));
    let (state, saved) = apply(&ir, state, Op::Control(Control::Save));
    let id = match saved.as_slice() {
        [Event::CheckpointSaved { id, .. }] => *id,
        other => panic!("save must succeed: {other:?}"),
    };
    assert_eq!(state.checkpoints[&id].entry, None);
    let (state, _) = apply(&ir, state.clone(), next_tick(&state));
    assert!(state.run.world.contains_key("z"));
    let (state, _) = apply(&ir, state, Op::Control(Control::Restore(id)));
    assert!(state.run.busy.is_some());
    assert_eq!(state.entry, None);
    let (stopped, events) = apply(&ir, state, Op::Control(Control::Stop));
    assert!(
        matches!(events.first(), Some(Event::Stopped { .. })),
        "{events:?}"
    );
    assert_eq!(stopped.run.busy, None);
    assert_eq!(f1_status(&stopped), StepStatus::Entered);
    assert_eq!(stopped.run.world, entered.run.world);
    assert_eq!(stopped.entry, None);
}

/// 復帰の写しが `Restored` と食い違う（保存から読んだ写しの書き換えなど）ときは、受付の時点を一意に決められない
/// ので取り消しをしない。上限で止めた後は、操作の前の `run` から `busy` だけを空にして戻り（Step は受付済みのまま、
/// 世界状態も操作の前のまま）、停止は `Warning` だけで状態を変えない（fail-closed）。
#[test]
fn mismatched_checkpoint_is_not_used_to_derive_entry() {
    let ir = processing_ir(
        serde_json::json!([{ "put": { "path": "x", "fixture": "fx" } }]),
        vec![
            serde_json::json!({ "name": "p1", "effects": [] }),
            serde_json::json!({ "name": "p2", "effects": [] }),
        ],
    );
    let (state, _) = apply(&ir, state_at("main/f1"), text());
    let (state, saved) = apply(&ir, state, Op::Control(Control::Save));
    let id = match saved.as_slice() {
        [Event::CheckpointSaved { id, .. }] => *id,
        other => panic!("save must succeed: {other:?}"),
    };
    let (mut state, _) = apply(&ir, state, Op::Control(Control::Restore(id)));
    if let Some(cp) = state.checkpoints.get_mut(&id) {
        cp.log_len += 1;
    }
    let (stopped, events) = apply(&ir, state.clone(), Op::Control(Control::Stop));
    assert!(
        matches!(events.as_slice(), [Event::Warning { .. }]),
        "{events:?}"
    );
    assert_eq!(stopped.run, state.run);

    let (state, _) = apply(&ir, state.clone(), next_tick(&state));
    let before = state.clone();
    let (state, events) = apply(&ir, state, next_tick(&before));
    assert_limit_then_changes(&events);
    assert_eq!(state.run.current, Some(uid("main/f1")));
    assert_eq!(state.run.busy, None);
    assert_eq!(f1_status(&state), StepStatus::Accepted);
    assert_eq!(state.run.world, before.run.world);
    assert!(state.run.world.contains_key("x"));
}

/// フラグは受付の時点の値へ戻さない（`Event::Flag` は変更前の値を持たず、ログから一意に決められないため。
/// `entry_derive` の「戻さない欄」）。世界状態と Step の状態は受付の時点へ戻り、受け付け直すと同じフラグを
/// 置き直すので、効果は二重にならない。この扱いを固定する。
#[test]
fn busy_limit_keeps_flags_set_by_the_stopped_operation() {
    let ir = processing_ir(
        serde_json::json!([
            { "put": { "path": "x", "fixture": "fx" } },
            { "set_flag": "seen" }
        ]),
        vec![serde_json::json!({ "name": "p1", "effects": [] })],
    );
    let entered = state_at("main/f1");
    let (accepted, _) = apply(&ir, entered.clone(), text());
    let root = uid("main/f1").root();
    assert!(accepted.run.slices[&root].flags.contains("seen"));
    let (state, events) = apply(&ir, accepted.clone(), next_tick(&accepted));
    assert_limit_then_changes(&events);
    assert_eq!(f1_status(&state), StepStatus::Entered);
    assert_eq!(state.run.world, entered.run.world);
    assert!(state.run.slices[&root].flags.contains("seen"));
    let (again, _) = apply(&ir, state, text());
    assert_eq!(
        again.run.slices[&root].flags,
        accepted.run.slices[&root].flags
    );
}

/// 受付の時点は、処理中の区間（受付から現在まで）の中でだけ探す。区間の外の受付（ここでは移動で Step に入り直す
/// 前の受付）からは導かない。受付が無いまま処理中になった状態（保存から読んだ不整合など）では、停止は `Warning`
/// だけで状態を変えず、上限で止めた後は Step を受付済みのまま `busy` だけを空にして戻す（fail-closed）。
#[test]
fn entry_is_not_derived_across_a_step_entry() {
    let ir = processing_ir(
        serde_json::json!([{ "put": { "path": "x", "fixture": "fx" } }]),
        vec![serde_json::json!({ "name": "p1", "effects": [] })],
    );
    let (state, _) = apply(&ir, state_at("main/f1"), text());
    let (moved, _) = apply(&ir, state, go_to("main/f1"));
    assert_eq!(moved.run.busy, None);
    // 移動の後に受付を経ずに処理中になった不整合な状態。
    let mut broken = moved;
    broken
        .run
        .slices
        .entry(uid("main/f1").root())
        .or_default()
        .status
        .insert(uid("main/f1"), StepStatus::Accepted);
    broken.run.busy = Some(Busy { done: 0, total: 1 });
    let (stopped, events) = apply(&ir, broken.clone(), Op::Control(Control::Stop));
    assert!(
        matches!(events.as_slice(), [Event::Warning { .. }]),
        "{events:?}"
    );
    assert_eq!(stopped.run, broken.run);

    let (state, events) = apply(&ir, broken.clone(), next_tick(&broken));
    assert_limit_event(&events);
    assert_eq!(f1_status(&state), StepStatus::Accepted);
    assert_eq!(state.run.busy, None);
    assert_eq!(state.run.world, broken.run.world);
}
