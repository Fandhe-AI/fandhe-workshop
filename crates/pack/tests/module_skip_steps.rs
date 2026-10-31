//! モジュールの先頭の Step を省けず、引数で迂回した Step が未着手のまま残ることの実行時の固定（TASK-252 1/2、MOD-17・D-43）。
//!
//! 決定（TASK-251、D-43 の追認。2026-10-31）で、先頭の Step を省く語は足さず、IR とエンジンの動きは変えない。
//! 本ファイルは MOD-17 のうち実行時の側を、コンパイルした IR を `Session` で動かして固定する。
//!
//! - (a) 引数つきの実体を `use` で呼ぶと、引数が後ろの Step を迂回させる場合でも、実行時に必ずモジュールの
//!   先頭の Step に入る（`mod17_call_enters_*`・`mod17_round2_*`）。静的な側（IR の `enter` が先頭を指すこと、
//!   省く欄が `E-PARSE` になること）は `module_first_step_gap.rs`（`docs/testing.md` の 5.217）にある
//! - (b) 迂回した Step は `NotStarted` のまま残り、途中開始（`Control::Begin` の `<実体>#<開始>`）で
//!   位置により `Skipped` になる状態とは別である（`mod17_bypassed_*`・`mod17_midstart_*`）
//! - (c) 引数の型と範囲は、`param_types` で宣言した引数だけを `validate`・`compile` が `E-STRUCTURE` で拒否する。
//!   これは `module_arg_types.rs` の `pack51_*`（5.218）で固定済みで、本ファイルでは重ねない。
//!   `mod17_args_pass_the_declared_check` は、検査を通った値で作った実体で (a)(b) を確かめる前提だけを見る
//!
//! 確かめないこと: エンジンは `next[].when` を評価しない（`crates/core/src/transit.rs`）ため、条件が真の側
//! （引数 `setup="true"` で設定の Step へ入る経路）の実行時の動きは確かめない。条件が偽の側は、評価の有無に関わらず
//! 無条件の辺に落ちるので、評価が入った後も本試験は同じ結果になる。
//! 経路は core の依存に pack が無いため、pack crate の試験として置く（記録は `docs/testing.md` の 5.422）。
//! 試験では `unwrap` を使ってよい。

mod common;
#[path = "common/rounds.rs"]
mod rounds;

use common::{assert_no_secret, testdata, Tmp, WORKSHOP};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::instance::{owner_of, root_of};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{Session, SessionState};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use rounds::{
    begin_round2_plan, round2_plan_until_create_tree, round_roots, run_plan, step_in, steps,
};

const STEP: &str = "input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}";

/// 先頭の `head` が引数 `setup` で `set-a`・`set-b` を迂回して `work` へ向かうモジュール。`setup` は型を宣言する。
fn module() -> String {
    format!(
        "id: m\ntitle: M\nparams: {{setup: \"true\"}}\nparam_types: {{setup: {{type: bool}}}}\nstarts: [{{id: s-w, node: work}}]\nsteps:\n  - {{id: head, title: Head, {STEP}, next: [{{to: set-a, when: {{param: {{name: setup, eq: \"true\"}}}}}}, {{to: work}}]}}\n  - {{id: set-a, title: A, next: [{{to: set-b}}], {STEP}}}\n  - {{id: set-b, title: B, next: [{{to: work}}], {STEP}}}\n  - {{id: work, title: W, next: [{{to: end}}], {STEP}}}\n  - {{id: end, title: E, final: true}}\n"
    )
}

fn pack() -> Tmp {
    let t = Tmp::new("mod17");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u0}}]\nsteps:\n  - {{id: u0, title: U0, next: [{{to: u1}}], {STEP}}}\n  - {{id: u1, title: U1, use: {{module: m, instance: off, args: {{setup: \"false\"}}}}, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("modules/m/module.yaml", module().as_bytes());
    t
}

fn ir_of(t: &Tmp) -> Ir {
    load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn choice(s: &mut Session) -> Vec<Event> {
    s.apply(Op::Input(Input::Choice("ok".into()))).to_vec()
}

fn begin(s: &mut Session, id: &str) -> Vec<Event> {
    s.apply(Op::Control(Control::Begin(id.into()))).to_vec()
}

/// `ModuleEntered` の直後の `Entered` の Step（無ければ panic）。
fn entered_after_module(events: &[Event], instance: &str) -> StepUid {
    let i = events
        .iter()
        .position(|e| matches!(e, Event::ModuleEntered { instance: n, .. } if n == instance))
        .expect("ModuleEntered");
    match &events[i + 1] {
        Event::Entered { at } => at.step.clone(),
        other => panic!("Entered expected after ModuleEntered, got {other:?}"),
    }
}

fn skipped_in(events: &[Event], scope: &str) -> Vec<StepUid> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Skipped { at } if at.step.0.starts_with(&format!("{scope}/")) => {
                Some(at.step.clone())
            }
            _ => None,
        })
        .collect()
}

fn bypassed(s: &Session) -> [StepStatus; 2] {
    [
        s.step_status(&uid("m@off/set-a")),
        s.step_status(&uid("m@off/set-b")),
    ]
}

/// 呼び出しまで進めた状態（`u0` の回答の後で `m@off/head` に着いている）。
fn at_head(ir: &Ir) -> (Session, Vec<Event>) {
    let mut s = Session::from_state(ir.clone(), SessionState::default());
    let mut ev = begin(&mut s, "s1");
    ev.extend(choice(&mut s));
    (s, ev)
}

#[test]
fn mod17_args_pass_the_declared_check() {
    let t = pack();
    let d = validate_dir(&t.0);
    assert_no_secret(&d);
    assert!(d.is_empty(), "{d:?}");
    let ir = ir_of(&t);
    let inst = ir
        .instances
        .iter()
        .find(|i| i.id == "m@off")
        .expect("m@off");
    assert_eq!(inst.args.get("setup").map(String::as_str), Some("false"));
    // 迂回の分かれ目が実在する（空振りの防止）。
    let head = ir
        .nodes
        .iter()
        .find(|n| n.uid == "m@off/head")
        .expect("head");
    assert_eq!(head.step.next.len(), 2);
    assert_eq!(head.step.next[0].to, "m@off/set-a");
    assert!(head.step.next[0].when.is_some());
    assert_eq!(head.step.next[1].to, "m@off/work");
    assert!(head.step.next[1].when.is_none());
}

#[test]
fn mod17_call_enters_the_first_step_at_runtime() {
    let ir = ir_of(&pack());
    let (s, ev) = at_head(&ir);
    assert_eq!(entered_after_module(&ev, "m@off"), uid("m@off/head"));
    assert_eq!(s.state().run.current, Some(uid("m@off/head")));
    assert_eq!(s.step_status(&uid("m@off/head")), StepStatus::Entered);
    for id in ["set-a", "set-b", "work"] {
        assert_eq!(
            s.step_status(&uid(&format!("m@off/{id}"))),
            StepStatus::NotStarted,
            "{id}"
        );
    }
}

#[test]
fn mod17_bypassed_steps_stay_not_started() {
    let ir = ir_of(&pack());
    let (mut s, mut ev) = at_head(&ir);
    ev.extend(choice(&mut s));
    assert_eq!(s.state().run.current, Some(uid("m@off/work")));
    assert_eq!(bypassed(&s), [StepStatus::NotStarted; 2]);
    ev.extend(choice(&mut s));
    // 最後まで通った（`work` が達成され、最後の `end` に着いた）後も、迂回した Step は未着手のまま。
    assert_eq!(s.step_status(&uid("m@off/work")), StepStatus::Achieved);
    assert_eq!(s.step_status(&uid("m@off/end")), StepStatus::Entered);
    assert_eq!(bypassed(&s), [StepStatus::NotStarted; 2]);
    assert!(skipped_in(&ev, "m@off").is_empty(), "{ev:?}");
}

#[test]
fn mod17_midstart_marks_the_same_steps_skipped() {
    let ir = ir_of(&pack());
    let (mut called, _) = at_head(&ir);
    choice(&mut called);
    let mut s = Session::from_state(ir.clone(), SessionState::default());
    let ev = begin(&mut s, "m@off#s-w");
    assert_eq!(
        skipped_in(&ev, "m@off"),
        vec![uid("m@off/head"), uid("m@off/set-a"), uid("m@off/set-b")]
    );
    assert_eq!(s.step_status(&uid("m@off/work")), StepStatus::Entered);
    // 呼び出しで迂回した同じ Step は、途中開始では別の状態になる。
    for id in ["set-a", "set-b"] {
        let u = uid(&format!("m@off/{id}"));
        assert_ne!(s.step_status(&u), called.step_status(&u), "{id}");
    }
}

/// 1 周目の終了状態（`round_slice_init.rs` の `finished_round1` と同じ作り。2 周目の開始の前提）。
fn finished_round1(ir: &Ir, r1: &[Root]) -> Session {
    let mut state = SessionState::default();
    for u in steps(ir, r1) {
        state
            .run
            .slices
            .entry(owner_of(ir, &u))
            .or_default()
            .status
            .insert(u, StepStatus::Achieved);
    }
    let cycle = state
        .run
        .slices
        .entry(root_of(&step_in(ir, r1, "st-2-4")))
        .or_default();
    cycle.flags.insert("variant_miss".into());
    state.run.current = steps(ir, r1)
        .into_iter()
        .rfind(|u| ir.nodes.iter().any(|n| n.uid == u.0 && n.enter.is_some()));
    Session::from_state(ir.clone(), state)
}

#[test]
fn mod17_round2_call_enters_st_3_2_and_bypassed_steps_stay_not_started() {
    let ir =
        load_ir(&compile_dir(&testdata("module-reuse-rounds/test-data/pack")).unwrap()).unwrap();
    let inst = ir
        .instances
        .iter()
        .find(|i| i.id == "issue-to-merge@round2")
        .expect("round2 instance");
    assert_eq!(inst.args.get("setup").map(String::as_str), Some("false"));
    let enter = ir
        .nodes
        .iter()
        .find(|n| n.uid == "main/r2-impl")
        .and_then(|n| n.enter.clone())
        .expect("enter");
    assert_eq!(enter, "issue-to-merge@round2/st-3-2");
    let (r1, _) = round_roots(&ir, "s-r1");
    let (r2, _) = round_roots(&ir, "s-r2");
    let mut s = finished_round1(&ir, &r1);
    let mut ev = run_plan(&mut s, &begin_round2_plan());
    ev.extend(run_plan(&mut s, &round2_plan_until_create_tree(&ir)));
    assert_eq!(
        entered_after_module(&ev, "issue-to-merge@round2"),
        uid(&enter)
    );
    assert!(s
        .state()
        .run
        .current
        .as_ref()
        .unwrap()
        .0
        .ends_with("/st-6-1"));
    for id in ["st-4-1", "st-5-1"] {
        assert_eq!(
            s.step_status(&step_in(&ir, &r2, id)),
            StepStatus::NotStarted,
            "{id}"
        );
    }
    assert!(
        skipped_in(&ev, "issue-to-merge@round2").is_empty(),
        "{ev:?}"
    );
}
