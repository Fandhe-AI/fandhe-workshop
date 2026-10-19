//! 1 回の操作で起きる内部の遷移の上限の試験（TASK-160 1/3、仕様 SAFE-23）。
//!
//! 作成者向けの検査を通さずに配られた巡回する IR（`testdata/untrusted-input-safety/test-data-addendum-3/cycle-ir/`）で、
//! 1,000 回に達した時点で止まり、`Warning` が 1 件だけ残り、現在の Step が空になって以降の入力が落ちずに無視されることを確かめる。
//! 巡回しない長い連鎖は、1,000 回ちょうどなら通り、1,001 回で止まる（境界）。再帰を使っていないことは小さなスタックで確かめる。
//! 開始（`Control::Begin`）は未実装（TASK-217）なので、`enter` の巡回は `Control::GoTo` で入口へ入って代わりにする。
//! 呼び出し元は CI の `make test` と変異注入（`tests/mutations/SAFE23-*.mutation`）である。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Node, Transition};
use fandhe_workshop_contract::limits::MAX_TRANSITIONS_PER_INPUT;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::SessionState;
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

#[test]
fn cycle_enter_self_stops_at_limit() {
    let ir = cycle_ir("cycle-enter-self");
    let (state, events) = apply(&ir, state_at("main/st-1"), go_to("main/u-self"));
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), MAX_TRANSITIONS_PER_INPUT);
    assert!(matches!(events.last(), Some(Event::Warning { .. })));
    assert_halted(&state, &ir);
}

#[test]
fn cycle_enter_2_stops_at_limit() {
    let ir = cycle_ir("cycle-enter-2");
    let (state, events) = apply(&ir, state_at("main/st-1"), go_to("main/u-a"));
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_halted(&state, &ir);
}

#[test]
fn cycle_exit_2_stops_at_limit() {
    let ir = cycle_ir("cycle-exit-2");
    let (state, events) = apply(&ir, state_at("main/f1"), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_eq!(count(&events, |e| matches!(e, Event::Achieved { .. })), 1);
    assert_halted(&state, &ir);
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
    assert_eq!(state.run.current, Some(uid("main/tail")));
    assert_eq!(entered(&events), ok);

    let ir = enter_chain(ok + 1);
    let (state, events) = apply(&ir, state_at("main/e"), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(entered(&events), ok + 1);
    assert_halted(&state, &ir);
}

#[test]
fn exit_chain_boundary() {
    // c0 の達成の後の戻りは c1..ck と末尾の k + 1 回。k = 999 で 1,000 回ちょうど。
    let ok = MAX_TRANSITIONS_PER_INPUT - 1;
    let ir = exit_chain(ok);
    let (state, events) = apply(&ir, state_at("main/c0"), text());
    assert_eq!(warnings(&events), 0, "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    // 末尾まで戻った後の現在位置は、最後の復帰先（呼び出し先の final Step のままにしない）。
    assert_eq!(state.run.current, Some(uid("main/tail")));

    let ir = exit_chain(ok + 1);
    let (state, events) = apply(&ir, state_at("main/c0"), text());
    assert_eq!(warnings(&events), 1, "{events:?}");
    assert_eq!(exited(&events), MAX_TRANSITIONS_PER_INPUT);
    assert_halted(&state, &ir);
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
    assert!(!events
        .iter()
        .any(|e| matches!(e, Event::Warning { message, .. } if message.contains("nowhere"))));
}
