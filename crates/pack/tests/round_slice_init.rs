//! 次の周回の開始時の切り身とフラグの初期状態の試験（MOD-3、TASK-235 1/2）。
//!
//! `module-reuse-rounds` をコンパイルした IR で、1 周目の終了状態（全 Step が達成・フラグが立っている）から
//! 2 周目の開始点へ入ったとき、2 周目の根の Step が現在の Step と呼び出しの Step を除きすべて `NotStarted` で、
//! フラグが空であることを確かめる。2 周目の根は IR の開始点と実体の呼び出し元から導き、実体 ID を直書きしない
//! （1 周目のフラグを立てる実体だけ、spec の例の名前 `issue-to-merge@round1` を使う）。
//!
//! TASK-235 2/2 として、2 周目を入力で終えた後も 1 周目の切り身が 2 周目の開始の前と同じで、1 周目のフラグが
//! 2 周目の実体の切り身で立たないことも確かめる。2 周目の入力列は `pt-rounds` の `s-r2` の区間と同じである。
//! 開始は `Control::Begin` で行う。次の 2 か所は後続の部品が未実装のため、入力で進めず `GoTo` で飛ばす（`docs/testing.md` の 5.258）。
//! (1) 遷移先の `on_enter` の効果を適用しない（`idea-cycle@round2` の結果の文書が世界状態に無い）ので、
//! `st-2-5` の手前から `st-2-6` へ飛ぶ。(2) `wait` の Step で処理中が始まらないので、`st-7-2` から `st-7-4` へ飛ぶ。
//! 2 周目の終了は、最後の Step の達成と呼び出し元の達成で判定する（`ChapterCompleted` は未実装）。

mod common;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Decision, Input, Op};
use fandhe_workshop_core::instance::{owner_of, root_of};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;

fn ir() -> Ir {
    let bytes = compile_dir(&testdata("module-reuse-rounds/test-data/pack")).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

/// 開始点 `start_id` の節点と同じ `unit` の `main` の節点を呼び出し元とする実体の根（IR の順）と、開始の節点。
fn round_roots(ir: &Ir, start_id: &str) -> (Vec<Root>, String) {
    let node_id = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .expect("start")
        .node
        .clone();
    let unit = ir
        .nodes
        .iter()
        .find(|n| n.uid == node_id)
        .expect("start node")
        .unit
        .clone();
    let callers: Vec<&str> = ir
        .nodes
        .iter()
        .filter(|n| n.scope == "main" && n.unit == unit && n.enter.is_some())
        .map(|n| n.uid.as_str())
        .collect();
    let roots = ir
        .instances
        .iter()
        .filter(|i| i.caller.as_deref().is_some_and(|c| callers.contains(&c)))
        .map(|i| Root(i.id.clone()))
        .collect();
    (roots, node_id)
}

fn steps(ir: &Ir, roots: &[Root]) -> Vec<StepUid> {
    roots
        .iter()
        .flat_map(|r| fandhe_workshop_core::instance::steps_of(ir, r))
        .collect()
}

/// 1 周目の終了状態。1 周目の全 Step を達成にし、`issue-to-merge@round1` のフラグを立てる。
/// 入力で完走させる部品は未実装（`when` つきの `next` の評価なし）のため、状態を直接組む。
fn finished_round1(ir: &Ir, r1: &[Root]) -> Session {
    let mut state = SessionState::default();
    for u in steps(ir, r1) {
        let owner = owner_of(ir, &u);
        state
            .run
            .slices
            .entry(owner)
            .or_default()
            .status
            .insert(u, StepStatus::Achieved);
    }
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .expect("issue-to-merge round1");
    let slice = state.run.slices.entry(stopper.clone()).or_default();
    slice.flags.insert("stopped".into());
    slice.flags.insert("precheck_ok".into());
    // 2 周目の受付が同じ名前を消す・立てるので、根の取り違えを検出できるよう 1 周目の側にも置く。
    // 根は実体 ID の文字列ではなく、当の Step の uid から `root_of` で引く。
    let cycle_step = step_in(ir, r1, "st-2-4");
    let cycle = state.run.slices.entry(root_of(&cycle_step)).or_default();
    cycle.flags.insert("variant_miss".into());
    // 移動の元になる現在の Step は、1 周目の最後の呼び出しの Step に置く。
    state.run.current = steps(ir, r1)
        .into_iter()
        .rfind(|u| ir.nodes.iter().any(|n| n.uid == u.0 && n.enter.is_some()));
    Session::from_state(ir.clone(), state)
}

#[test]
fn second_round_roots_hold_eighteen_steps_with_two_call_steps() {
    let ir = ir();
    let (r2, _) = round_roots(&ir, "s-r2");
    assert_eq!(r2.len(), 2, "round 2 has two module instances");
    let all = steps(&ir, &r2);
    // 8 + 8 + 呼び出しの Step 2 件（spec の MOD-3 の 18 件）。
    assert_eq!(all.len(), 18);
    let calls = all
        .iter()
        .filter(|u| ir.nodes.iter().any(|n| n.uid == u.0 && n.enter.is_some()))
        .count();
    assert_eq!(calls, 2);
}

#[test]
fn second_round_is_untouched_before_it_starts() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let s = finished_round1(&ir, &r1);
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .unwrap();
    assert!(s.slice(stopper).unwrap().flags.contains("stopped"));
    for u in steps(&ir, &r2) {
        assert_eq!(s.step_status(&u), StepStatus::NotStarted, "{}", u.0);
    }
    for r in &r2 {
        assert!(s.slice(r).is_none_or(|sl| sl.flags.is_empty()));
    }
}

#[test]
fn second_round_starts_with_only_the_current_and_call_steps_entered() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, call_node) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    let before: Vec<_> = r1.iter().map(|r| s.slice(r).cloned()).collect();
    let events = s.apply(Op::Control(Control::Begin("s-r2".into()))).to_vec();
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Begun { .. }))
            .count(),
        1,
        "{events:?}"
    );
    let after: Vec<_> = r1.iter().map(|r| s.slice(r).cloned()).collect();
    assert_eq!(before, after, "round 1 slices unchanged by Begin");
    let current = s.state().run.current.clone().expect("current");
    let all = steps(&ir, &r2);
    let entered: Vec<&StepUid> = all
        .iter()
        .filter(|u| s.step_status(u) == StepStatus::Entered)
        .collect();
    assert_eq!(entered.len(), 2, "{entered:?}");
    assert!(entered.contains(&&current));
    assert!(entered.iter().any(|u| u.0 == call_node));
    for u in all.iter().filter(|u| !entered.contains(u)) {
        assert_eq!(s.step_status(u), StepStatus::NotStarted, "{}", u.0);
    }
    // 呼び出しの Step の状態は呼ばれた実体の切り身にあり、`main` には無い。
    let main = s.slice(&Root("main".into()));
    assert!(main.is_none_or(|sl| !sl.status.contains_key(&StepUid(call_node.clone()))));
    // 2 周目のフラグは空で、1 周目のフラグは立ったまま。
    for r in &r2 {
        assert!(s.slice(r).is_none_or(|sl| sl.flags.is_empty()));
    }
    let stopper = r1
        .iter()
        .find(|r| r.0.starts_with("issue-to-merge"))
        .unwrap();
    assert!(s.slice(stopper).unwrap().flags.contains("stopped"));
}

/// 周回の根の集まりから、末尾が `/<id>` の Step を引く。
fn step_in(ir: &Ir, roots: &[Root], id: &str) -> StepUid {
    let suffix = format!("/{id}");
    steps(ir, roots)
        .into_iter()
        .find(|u| u.0.ends_with(&suffix))
        .unwrap_or_else(|| panic!("step {id}"))
}

fn go(s: &mut Session, ev: &mut Vec<Event>, ir: &Ir, roots: &[Root], id: &str) {
    let to = step_in(ir, roots, id);
    ev.extend_from_slice(s.apply(Op::Control(Control::GoTo(to))));
}

fn input(s: &mut Session, ev: &mut Vec<Event>, i: Input) {
    ev.extend_from_slice(s.apply(Op::Input(i)));
}

fn tick(s: &mut Session, ev: &mut Vec<Event>) {
    let run = &s.state().run;
    let addr = TickAddr {
        generation: run.generation,
        step: run.current.clone().expect("current"),
        index: run.busy.as_ref().expect("busy").done,
    };
    ev.extend_from_slice(s.apply(Op::Tick(addr)));
}

fn current(s: &Session) -> String {
    s.state().run.current.clone().expect("current").0
}

/// 2 周目を終わりまで進め、操作で出たイベントを返す。開始は `Control::Begin`（TASK-237 1/2）。
/// 入力列は `pt-rounds` の `s-r2` の区間。飛ばす 2 か所の理由は冒頭の `//!` に書いた。
fn finish_round2(s: &mut Session, ir: &Ir, r2: &[Root]) -> Vec<Event> {
    let mut events = Vec::new();
    let ev = &mut events;
    let cmd = |c: &str| Input::Command(c.into());
    let ui = |v: &str, p: &str| Input::Ui {
        view: v.into(),
        path: p.into(),
    };
    let ch = |c: &str| Input::Choice(c.into());
    let ap = |d: Decision, t: Option<&str>| Input::Approval {
        decision: d,
        target: t.map(Into::into),
    };
    ev.extend_from_slice(s.apply(Op::Control(Control::Begin("s-r2".into()))));
    input(s, ev, cmd("/new-idea bbs-search"));
    input(s, ev, ui("v-files", "ideas/bbs-search/README.md"));
    input(
        s,
        ev,
        Input::Text(
            "ログインした人だけが書き込める掲示板で、投稿が増えると目的の投稿を探しにくい".into(),
        ),
    );
    for _ in 0..3 {
        tick(s, ev);
    }
    input(s, ev, ch("ans-login-first"));
    input(s, ev, ap(Decision::Approve, None));
    input(s, ev, ui("v-doc", "ideas/bbs-search/README.md#status"));
    input(s, ev, ch("cand-session"));
    go(s, ev, ir, r2, "st-2-6");
    input(s, ev, ap(Decision::Revise, Some("expect")));
    input(s, ev, ap(Decision::Approve, None));
    input(s, ev, ui("v-tasks", "TASK-1"));
    input(s, ev, ch("ans-login-1"));
    input(s, ev, ap(Decision::Approve, None));
    input(s, ev, ui("v-doc", "ideas/bbs-search/README.md#status"));
    assert!(
        current(s).ends_with("/st-3-2"),
        "after idea-cycle: {}",
        current(s)
    );
    input(s, ev, ch("diff-only"));
    input(s, ev, ui("v-files", "impl/docs/behavior-bbs-search.md"));
    input(s, ev, cmd("/create-issue-tree behavior-bbs-search.md"));
    input(s, ev, ui("v-issue-tree", "issues/child-9"));
    input(s, ev, cmd("/implement-issue-tree 43 --external-checks []"));
    input(s, ev, ch("ans-none-explicit"));
    go(s, ev, ir, r2, "st-7-4");
    input(s, ev, ui("v-pr-cards", "prs/pr-2-a"));
    input(s, ev, ui("v-pr-cards", "prs/pr-2-a#merge"));
    input(s, ev, ui("v-issue-tree", "issues/child-9"));
    events
}

/// 1 周目の根ごとの切り身（すべて存在する）。
fn slices_of(s: &Session, roots: &[Root]) -> Vec<(Root, fandhe_workshop_core::session::Slice)> {
    roots
        .iter()
        .map(|r| (r.clone(), s.slice(r).cloned().expect("round 1 slice")))
        .collect()
}

fn flags_of(s: &Session, ir: &Ir, roots: &[Root], id: &str) -> Vec<String> {
    let root = root_of(&step_in(ir, roots, id));
    s.slice(&root)
        .map(|sl| sl.flags.iter().cloned().collect())
        .unwrap_or_default()
}

#[test]
fn first_round_slices_are_unchanged_after_the_second_round_ends() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    let before = slices_of(&s, &r1);
    finish_round2(&mut s, &ir, &r2);
    assert_eq!(current(&s), "main/r2-impl");
    assert_eq!(slices_of(&s, &r1), before);
    for u in steps(&ir, &r1) {
        assert_eq!(s.step_status(&u), StepStatus::Achieved, "{}", u.0);
    }
    // 呼び出しの Step の状態は終了の時点でも `main` の切り身に無い。
    let main = s.slice(&Root("main".into()));
    for c in ["main/r2-idea", "main/r2-impl"] {
        assert_eq!(s.step_status(&StepUid(c.into())), StepStatus::Achieved);
        assert!(main.is_none_or(|sl| !sl.status.contains_key(&StepUid(c.into()))));
    }
}

#[test]
fn first_round_flags_are_not_set_in_second_round_instances() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    let ev = finish_round2(&mut s, &ir, &r2);
    // 2 周目の切り身には、2 周目の受付が立てたものだけがある（`stopped` は無い）。
    assert_eq!(flags_of(&s, &ir, &r2, "st-7-1"), vec!["precheck_ok"]);
    assert_eq!(flags_of(&s, &ir, &r2, "st-2-4"), vec!["revised"]);
    // 1 周目のフラグは立ったまま。
    assert_eq!(
        flags_of(&s, &ir, &r1, "st-7-3"),
        vec!["precheck_ok", "stopped"]
    );
    assert_eq!(flags_of(&s, &ir, &r1, "st-2-4"), vec!["variant_miss"]);
    // 2 周目の操作が出したフラグの変化は、すべて 2 周目の根に付く。
    let r2_scopes: Vec<String> = r2.iter().map(|r| r.0.clone()).collect();
    let mut seen = 0;
    for e in &ev {
        if let Event::Flag { scope, .. } = e {
            seen += 1;
            assert!(r2_scopes.contains(scope), "flag event in {scope}");
        }
    }
    assert!(seen > 0, "round 2 raised no flag events");
}

#[test]
fn second_round_end_matches_the_playthrough_asserts() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    finish_round2(&mut s, &ir, &r2);
    let st = |ro: &[Root], id: &str| s.step_status(&step_in(&ir, ro, id));
    assert_eq!(st(&r1, "st-7-3"), StepStatus::Achieved);
    assert_ne!(st(&r2, "st-7-3"), StepStatus::Achieved);
    assert_eq!(st(&r1, "st-4-1"), StepStatus::Achieved);
    assert_eq!(st(&r2, "st-4-1"), StepStatus::NotStarted);
    assert_eq!(st(&r2, "st-5-1"), StepStatus::NotStarted);
    assert_eq!(st(&r2, "st-7-4"), StepStatus::Achieved);
    assert_eq!(st(&r2, "st-2-8"), StepStatus::Achieved);
}

/// `s-r2-fresh` は前の周回なしで 2 周目を始める開始点で、`effects` の `put` の経路だけが世界状態に入る（TASK-237 1/2）。
#[test]
fn begin_fresh_start_writes_exactly_the_start_effects() {
    let ir = ir();
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == "s-r2-fresh")
        .expect("start")
        .clone();
    let mut s = Session::new(ir.clone());
    let events = s
        .apply(Op::Control(Control::Begin("s-r2-fresh".into())))
        .to_vec();
    assert!(
        matches!(events.first(), Some(Event::Begun { .. })),
        "{events:?}"
    );
    let mut expected: Vec<String> = start
        .effects
        .iter()
        .filter_map(|v| v.get("put").and_then(|p| p.get("path")))
        .filter_map(|p| p.as_str().map(str::to_owned))
        .collect();
    expected.sort();
    assert_eq!(expected.len(), 13, "{expected:?}");
    let mut changed: Vec<String> = events
        .iter()
        .filter_map(|e| match e {
            Event::Changed { path, cause, .. } => {
                assert_eq!(*cause, fandhe_workshop_core::event::Cause::Start);
                Some(path.to_string())
            }
            _ => None,
        })
        .collect();
    changed.sort();
    assert_eq!(changed, expected);
    assert!(s.state().run.current.is_some());
}

/// 開始条件を満たさない Begin は `Refused` 1 件で状態を変えない（最小の確認。詳細は TASK-240）。
#[test]
fn begin_with_unmet_condition_is_refused_without_change() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let mut s = finished_round1(&ir, &r1);
    let impl_step = step_in(&ir, &r1, "r1-impl");
    let owner = owner_of(&ir, &impl_step);
    let mut state = s.state().clone();
    state
        .run
        .slices
        .entry(owner)
        .or_default()
        .status
        .insert(impl_step, StepStatus::Entered);
    s = Session::from_state(ir.clone(), state);
    let before = s.state().run.clone();
    let events = s.apply(Op::Control(Control::Begin("s-r2".into()))).to_vec();
    assert!(
        matches!(
            events.as_slice(),
            [Event::Refused {
                reason: fandhe_workshop_core::event::RefuseReason::StartUnmet,
                ..
            }]
        ),
        "{events:?}"
    );
    assert_eq!(s.state().run, before);
}
