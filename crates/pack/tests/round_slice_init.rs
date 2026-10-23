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
//!
//! TASK-237 2/2 として、世界状態と切り身のハッシュの一致を確かめる（`mod5_*`。定義は `scenarios.md` の A 節）。
//! 1 周目は `pt-r1` の入力列で流す（`play_round1`）。未実装の部品のため、次の 3 か所は `GoTo` で飛ばす。
//! (a) `st-2-5` の着地の `on_enter`（結果の文書を書く）、(b) `when` つき `next` の評価（`st-4-1`・`st-5-1` へ入る）、
//! (c) `wait` の Step での処理中の開始（`st-7-2`）。記録は `docs/testing.md` の 5.260。
//!
//! TASK-241 2/2 として、復帰の範囲のうち世界状態と呼び出しの Step の持ち主を確かめる（`mod9_restore_reverts_owner_world_*`・
//! `mod9_restore_returns_call_step_status_with_owner`。記録は `docs/testing.md` の 5.262）。1 周目の世界状態の書き込みは、
//! `wait` の処理中が未実装で `prs/pr-1-a` が作られないため、1 周目の `st-2-1` への `GoTo` と入力（`ideas/bbs-with-login/README.md`
//! の `status` の書き換え）で代替する。案内レベルを戻すことの確認は、`Begin` が開始点の `guidance` を反映しない（TASK-294）ため
//! core の試験（`restore_owner_slice`）が受け持つ。

mod common;
#[path = "common/sha256.rs"]
mod sha256;

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

/// 2 周目の開始（`Control::Begin`）だけを行い、出たイベントを返す（TASK-237 1/2）。
fn begin_round2(s: &mut Session) -> Vec<Event> {
    s.apply(Op::Control(Control::Begin("s-r2".into()))).to_vec()
}

/// 2 周目を終わりまで進め、操作で出たイベントを返す。開始は `Control::Begin`（TASK-237 1/2）。
/// 入力列は `pt-rounds` の `s-r2` の区間。飛ばす 2 か所の理由は冒頭の `//!` に書いた。
fn finish_round2(s: &mut Session, ir: &Ir, r2: &[Root]) -> Vec<Event> {
    let mut events = begin_round2(s);
    events.extend(play_round2(s, ir, r2));
    events
}

/// 2 周目の開始の後の入力列。
fn play_round2(s: &mut Session, ir: &Ir, r2: &[Root]) -> Vec<Event> {
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

/// 1 周目を `pt-r1` の入力列で最後まで流す（世界状態を持つ。`finished_round1` は状態を手で組むので世界状態が空）。
/// 飛ばす 3 か所 (a)(b)(c) の理由は冒頭の `//!` に書いた。
fn play_round1(ir: &Ir) -> Session {
    let (r1, _) = round_roots(ir, "s-r1");
    let mut session = Session::new(ir.clone());
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
    let s = &mut session;
    ev.extend_from_slice(s.apply(Op::Control(Control::Begin("s-r1".into()))));
    input(s, ev, cmd("/new-idea bbs-with-login"));
    input(s, ev, ui("v-files", "ideas/bbs-with-login/README.md"));
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
    input(s, ev, ui("v-doc", "ideas/bbs-with-login/README.md#status"));
    input(s, ev, ch("cand-session"));
    // (a) 着地の `on_enter` が未実装で結果の文書が無いので、`go` と verdict の操作を飛ばす。
    go(s, ev, ir, &r1, "st-2-6");
    input(s, ev, ap(Decision::Revise, Some("expect")));
    input(s, ev, ap(Decision::Approve, None));
    input(s, ev, ui("v-tasks", "TASK-1"));
    input(s, ev, ch("ans-login-1"));
    input(s, ev, ap(Decision::Approve, None));
    input(s, ev, ui("v-doc", "ideas/bbs-with-login/README.md#status"));
    input(s, ev, ch("diff-only"));
    input(s, ev, ui("v-files", "impl/docs/behavior-bbs-with-login.md"));
    // (b) `when` つき `next` が評価されず `setup=true` の枝に入らないので、`st-4-1` へ飛んで 2 つの受付を送る。
    go(s, ev, ir, &r1, "st-4-1");
    input(s, ev, cmd("/init-claude"));
    input(s, ev, cmd("/setup-repo-guards"));
    input(s, ev, cmd("/create-issue-tree behavior-bbs-with-login.md"));
    input(s, ev, ui("v-issue-tree", "issues/child-1"));
    input(s, ev, cmd("/implement-issue-tree 42 --external-checks []"));
    input(s, ev, ch("ans-none-explicit"));
    // (c) `wait` の Step で処理中が始まらないので、tick 2 回を送らず次の Step へ飛ぶ。
    go(s, ev, ir, &r1, "st-7-3");
    input(s, ev, ui("v-pr-cards", "prs/pr-1-a"));
    input(s, ev, cmd("/implement-issue-tree 42"));
    input(s, ev, ch("ans-add-arg"));
    input(s, ev, ui("v-pr-cards", "prs/pr-1-a#merge"));
    input(s, ev, ui("v-issue-tree", "issues/child-1"));
    // 2 周目の開始条件（`r1-impl` が達成）を満たす終了の状態であること。
    assert_eq!(current(s), "main/r1-impl", "{events:?}");
    for c in ["main/r1-idea", "main/r1-impl"] {
        assert_eq!(s.step_status(&StepUid(c.into())), StepStatus::Achieved);
    }
    session
}

/// `World` の正準 JSON の SHA-256（16 進）。`World`・`Element.attrs` は `BTreeMap` で順が決まる。
fn world_hash(s: &Session) -> String {
    let json = serde_json::to_string(&s.state().run.world).expect("world json");
    sha256::hex(&sha256::sha256(json.as_bytes()))
}

/// 根の切り身の正準 JSON の SHA-256（16 進）。`Slice` の欄は `BTreeMap`/`BTreeSet` で、`clarifying` は `None` なら出ない。
fn slice_hash(s: &Session, root: &Root) -> String {
    let slice = s.slice(root).expect("slice");
    let json = serde_json::to_string(slice).expect("slice json");
    sha256::hex(&sha256::sha256(json.as_bytes()))
}

/// 記録用の行。値は `docs/testing.md` の 5.260 に転記する（表明はしない）。
fn record(point: &str, hash: &str) {
    println!("STATE-HASH point={point} sha256={hash}");
}

#[test]
fn mod5_sha256_known_answers() {
    let h = |d: &[u8]| sha256::hex(&sha256::sha256(d));
    assert_eq!(
        h(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        h(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        h(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
}

#[test]
fn mod5_world_hash_is_unchanged_by_begin_after_round1() {
    let ir = ir();
    let mut s = play_round1(&ir);
    let world_end = s.state().run.world.clone();
    let h_end = world_hash(&s);
    assert!(!world_end.is_empty(), "round 1 left no world state");
    let events = begin_round2(&mut s);
    let h_start = world_hash(&s);
    record("r1-end-world", &h_end);
    record("r2-begin-world", &h_start);
    assert_eq!(
        events
            .iter()
            .filter(|e| matches!(e, Event::Begun { .. }))
            .count(),
        1,
        "{events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, Event::Changed { .. })),
        "Begin changed the world: {events:?}"
    );
    assert_eq!(s.state().run.world, world_end);
    assert_eq!(h_end, h_start);
}

/// 新しいセッションの `s-r2-fresh` の直後の世界状態は、1 周目の終了時の世界状態と等しくない。差は次の 2 経路だけで、
/// どちらも fresh にだけある。(1) `ideas/bbs-with-login/03-poc/login-session/result.md` は `st-2-5` の着地の
/// `on_enter` が書く（未実装のため `play_round1` が飛ばす）。(2) `prs/pr-1-a` は `st-7-2` の `on_enter` と `wait` の
/// 進行が書く（未実装）。後続の部品が入ると差が縮み、この試験が落ちる。そのときは等式（`H_fresh == H_end_r1`）の
/// 試験へ書き換える（`docs/testing.md` の 5.260）。
#[test]
fn mod5_fresh_start_world_differs_from_round1_end_only_by_unreachable_paths() {
    let ir = ir();
    let end = play_round1(&ir);
    let mut fresh = Session::new(ir.clone());
    fresh.apply(Op::Control(Control::Begin("s-r2-fresh".into())));
    let (h_end, h_fresh) = (world_hash(&end), world_hash(&fresh));
    record("r2-fresh-world", &h_fresh);
    let (me, mf) = (&end.state().run.world, &fresh.state().run.world);
    for (k, v) in me {
        assert_eq!(
            mf.get(k),
            Some(v),
            "path {k} differs or is missing in fresh"
        );
    }
    let only_fresh: Vec<&str> = mf
        .keys()
        .filter(|k| !me.contains_key(*k))
        .map(String::as_str)
        .collect();
    assert_eq!(
        only_fresh,
        vec![
            "ideas/bbs-with-login/03-poc/login-session/result.md",
            "prs/pr-1-a"
        ]
    );
    assert_ne!(h_end, h_fresh);
}

#[test]
fn mod5_round1_slice_hashes_match_between_round1_end_and_round2_end() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    assert_eq!(r1.len(), 2);
    let mut s = play_round1(&ir);
    let at_end: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    finish_round2(&mut s, &ir, &r2);
    assert_eq!(current(&s), "main/r2-impl");
    let at_r2: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    for ((r, a), b) in r1.iter().zip(&at_end).zip(&at_r2) {
        record(&format!("r1-slice-{}-at-r1-end", r.0), a);
        record(&format!("r1-slice-{}-at-r2-end", r.0), b);
    }
    assert_eq!(at_end, at_r2);
}

// ---- TASK-241 1/2: 持ち主の根の切り身と位置だけを戻す復帰（MOD-9。記録は docs/testing.md の 5.261）----
// 世界状態・`global_flags`・案内レベルの戻り方は TASK-241 2/2（`mod9_*` の後半）が確かめる。
// (ii-b) は `wait` の Step で処理中が始まらないため、1 周目の Step への `GoTo` で 1 周目の切り身を変える代替にした。
// 入力で届かない `issue-to-merge@round2` の `st-7-1` は `GoTo` で飛ぶ。

/// 2 周目の開始の後、`idea-cycle` 側の `st-2-4` まで入力で進める。
fn round2_inputs_to_st24(s: &mut Session) {
    let mut events = Vec::new();
    let ev = &mut events;
    let ui = |v: &str, p: &str| Input::Ui {
        view: v.into(),
        path: p.into(),
    };
    input(s, ev, Input::Command("/new-idea bbs-search".into()));
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
    input(s, ev, Input::Choice("ans-login-first".into()));
    input(
        s,
        ev,
        Input::Approval {
            decision: Decision::Approve,
            target: None,
        },
    );
    input(s, ev, ui("v-doc", "ideas/bbs-search/README.md#status"));
    assert!(current(s).ends_with("/st-2-4"), "at: {}", current(s));
}

fn save_op(s: &mut Session) {
    s.apply(Op::Control(Control::Save));
}

fn restore_op(s: &mut Session) {
    s.apply(Op::Control(Control::Restore(
        fandhe_workshop_core::input::CheckpointId(1),
    )));
}

fn cur_root(s: &Session) -> Root {
    root_of(&s.state().run.current.clone().expect("current"))
}

#[test]
fn mod9_restore_in_round2_keeps_other_instance_and_round1_slices() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = play_round1(&ir);
    begin_round2(&mut s);
    round2_inputs_to_st24(&mut s);
    let owner = cur_root(&s);
    let owner_slice = s.slice(&owner).cloned().expect("owner slice");
    let pos = s.state().run.current.clone();
    save_op(&mut s);
    let mut ev = Vec::new();
    go(&mut s, &mut ev, &ir, &r2, "st-7-1");
    let other = cur_root(&s);
    assert_ne!(other, owner);
    let other_before = s.slice(&other).cloned().expect("other slice");
    let r1_before: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    restore_op(&mut s);
    assert_eq!(s.slice(&owner), Some(&owner_slice));
    assert_eq!(s.state().run.current, pos);
    assert_eq!(s.slice(&other), Some(&other_before));
    let r1_after: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    assert_eq!(r1_before, r1_after);
}

#[test]
fn mod9_restore_does_not_rewind_round1_changed_after_save() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let mut s = play_round1(&ir);
    begin_round2(&mut s);
    round2_inputs_to_st24(&mut s);
    let r1_at_save: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    save_op(&mut s);
    let mut ev = Vec::new();
    go(&mut s, &mut ev, &ir, &r1, "st-7-3");
    let h1: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    assert_ne!(h1, r1_at_save, "the go must change a round 1 slice");
    restore_op(&mut s);
    let h2: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    assert_eq!(h1, h2);
}

#[test]
fn mod9_restore_to_round1_checkpoint_keeps_round2_progress() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = play_round1(&ir);
    let mut ev = Vec::new();
    go(&mut s, &mut ev, &ir, &r1, "st-7-3");
    let owner = cur_root(&s);
    let owner_slice = s.slice(&owner).cloned().expect("owner slice");
    let pos = s.state().run.current.clone();
    save_op(&mut s);
    let events = begin_round2(&mut s);
    assert!(events.iter().any(|e| matches!(e, Event::Begun { .. })));
    round2_inputs_to_st24(&mut s);
    let r2_before: Vec<_> = r2.iter().map(|r| s.slice(r).cloned()).collect();
    restore_op(&mut s);
    let r2_after: Vec<_> = r2.iter().map(|r| s.slice(r).cloned()).collect();
    assert_eq!(r2_before, r2_after);
    assert_eq!(s.slice(&owner), Some(&owner_slice));
    assert_eq!(s.state().run.current, pos);
}

// ---- TASK-241 2/2: 世界状態・呼び出しの Step の復帰範囲（MOD-9。記録は docs/testing.md の 5.262）----

/// 保存の後（ログの位置 `since` 以降）に `Changed` が書いた経路を、宛先の持ち主の根ごとに集める。
fn changed_paths(s: &Session, since: usize) -> Vec<(Root, String)> {
    let ir = s.ir();
    s.state()
        .log
        .iter()
        .skip(since)
        .filter_map(|e| match e {
            Event::Changed { at, path, .. } => Some((owner_of(ir, &at.step), path.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn mod9_restore_reverts_owner_world_paths_and_keeps_round1_world() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let mut s = play_round1(&ir);
    begin_round2(&mut s);
    round2_inputs_to_st24(&mut s);
    let owner = cur_root(&s);
    let world_at_save = s.state().run.world.clone();
    save_op(&mut s);
    let since = s.state().log.len();
    let mut ev = Vec::new();
    // 持ち主（2 周目の `idea-cycle`）の入力が結果の文書を書く。
    input(&mut s, &mut ev, Input::Choice("cand-session".into()));
    // 1 周目の宛先の `Changed`（1 周目の README の status の書き換え）。
    go(&mut s, &mut ev, &ir, &r1, "st-2-1");
    input(
        &mut s,
        &mut ev,
        Input::Command("/new-idea bbs-with-login".into()),
    );
    input(
        &mut s,
        &mut ev,
        Input::Ui {
            view: "v-files".into(),
            path: "ideas/bbs-with-login/README.md".into(),
        },
    );
    let written = changed_paths(&s, since);
    let owner_paths: Vec<&String> = written
        .iter()
        .filter(|(r, _)| *r == owner)
        .map(|(_, p)| p)
        .collect();
    let r1_paths: Vec<&String> = written
        .iter()
        .filter(|(r, _)| r1.contains(r))
        .map(|(_, p)| p)
        .collect();
    assert!(
        !owner_paths.is_empty(),
        "owner must write a path: {written:?}"
    );
    assert!(
        !r1_paths.is_empty(),
        "round 1 must write a path: {written:?}"
    );
    assert!(owner_paths.iter().all(|p| !r1_paths.contains(p)));
    let world_before = s.state().run.world.clone();
    let r1_hashes: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    restore_op(&mut s);
    let world_after = &s.state().run.world;
    for p in &owner_paths {
        assert_eq!(world_after.get(*p), world_at_save.get(*p), "owner path {p}");
    }
    for p in &r1_paths {
        assert_eq!(
            world_after.get(*p),
            world_before.get(*p),
            "round 1 path {p}"
        );
    }
    assert_ne!(
        world_after, &world_before,
        "the owner paths must be reverted"
    );
    let after_hashes: Vec<String> = r1.iter().map(|r| slice_hash(&s, r)).collect();
    assert_eq!(r1_hashes, after_hashes);
}

#[test]
fn mod9_restore_returns_call_step_status_with_owner() {
    let ir = ir();
    let mut s = play_round1(&ir);
    begin_round2(&mut s);
    round2_inputs_to_st24(&mut s);
    let owner = cur_root(&s);
    // 呼び出しの Step は実体の表から引く（uid を直書きしない）。
    let call = StepUid(
        ir.instances
            .iter()
            .find(|i| i.id == owner.0)
            .and_then(|i| i.caller.clone())
            .expect("caller of the owner instance"),
    );
    assert_eq!(owner_of(&ir, &call), owner);
    // 呼び出しの Step の状態は呼ばれた実体の切り身にあり、持ち主の復帰で一緒に戻る。現在の Step が呼び出しの Step そのものに
    // なる場面は `GoTo` でも入力でも作れない（着地先は呼ばれた実体の最初の Step）ので、保存の持ち主を呼ばれた実体の根にする
    // 規則（変異 X6 の否定）は core の `restore_call_step_owner` が手組みの状態で確かめる。
    let at_save = s.step_status(&call);
    assert_eq!(at_save, StepStatus::Entered);
    let (r2, _) = round_roots(&ir, "s-r2");
    let other: Vec<_> = r2.iter().filter(|r| **r != owner).collect();
    save_op(&mut s);
    // 周回の終わりまで進める。`exit_to` が呼び出しの Step を達成にする。
    let mut ev = Vec::new();
    let ui = |v: &str, p: &str| Input::Ui {
        view: v.into(),
        path: p.into(),
    };
    let ap = |d: Decision, t: Option<&str>| Input::Approval {
        decision: d,
        target: t.map(Into::into),
    };
    input(&mut s, &mut ev, Input::Choice("cand-session".into()));
    go(&mut s, &mut ev, &ir, &r2, "st-2-6");
    input(&mut s, &mut ev, ap(Decision::Revise, Some("expect")));
    input(&mut s, &mut ev, ap(Decision::Approve, None));
    input(&mut s, &mut ev, ui("v-tasks", "TASK-1"));
    input(&mut s, &mut ev, Input::Choice("ans-login-1".into()));
    input(&mut s, &mut ev, ap(Decision::Approve, None));
    input(
        &mut s,
        &mut ev,
        ui("v-doc", "ideas/bbs-search/README.md#status"),
    );
    assert!(current(&s).ends_with("/st-3-2"), "at: {}", current(&s));
    assert_eq!(s.step_status(&call), StepStatus::Achieved);
    let other_before: Vec<_> = other.iter().map(|r| s.slice(r).cloned()).collect();
    restore_op(&mut s);
    assert_eq!(s.step_status(&call), at_save);
    let other_after: Vec<_> = other.iter().map(|r| s.slice(r).cloned()).collect();
    assert_eq!(other_before, other_after);
}

#[test]
fn mod9_round1_checkpoint_restore_does_not_revert_round2_start_world() {
    let ir = ir();
    let (r1, _) = round_roots(&ir, "s-r1");
    let mut s = play_round1(&ir);
    let mut ev = Vec::new();
    go(&mut s, &mut ev, &ir, &r1, "st-7-3");
    save_op(&mut s);
    let since = s.state().log.len();
    begin_round2(&mut s);
    round2_inputs_to_st24(&mut s);
    let written = changed_paths(&s, since);
    assert!(!written.is_empty(), "round 2 must write world paths");
    let world_before = s.state().run.world.clone();
    restore_op(&mut s);
    for (root, path) in &written {
        assert!(!r1.contains(root), "{root:?} {path}");
        assert_eq!(
            s.state().run.world.get(path),
            world_before.get(path),
            "round 2 path {path}"
        );
    }
}
