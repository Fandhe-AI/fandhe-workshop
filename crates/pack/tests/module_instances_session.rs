//! 試作の教材での学習モジュールの実体の分離の試験（MOD-1、TASK-233 1/2）。
//!
//! `module-reuse-rounds` をコンパイルした IR で `Session` を動かし、同じ定義の Step（spec の `impl-run` に当たる
//! `issue-to-merge` の Step）が round1・round2 の実体ごとに別の Step の実体として扱われ、片方の状態が
//! もう片方へ漏れないことを確かめる。`testdata/` は書き換えない。実体数・Step の実体数の照合は TASK-233 2/2 が持つ。
//! 実際の uid は IR から引き、教材の写しの更新で名前が変わっても壊れにくくする。

mod common;

use common::testdata;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::Session;
use fandhe_workshop_pack::compile::compile_dir;

const PACK: &str = "module-reuse-rounds/test-data/pack";

fn ir() -> Ir {
    let bytes = compile_dir(&testdata(PACK)).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

/// モジュールの実体の根（IR の順）。
fn module_roots(ir: &Ir) -> Vec<Root> {
    ir.instances
        .iter()
        .filter(|i| i.module.is_some())
        .map(|i| Root(i.id.clone()))
        .collect()
}

fn step_name(uid: &StepUid) -> &str {
    uid.0.rsplit_once('/').map_or(uid.0.as_str(), |(_, s)| s)
}

/// 同じモジュールの 2 つの実体（根）。
fn two_roots(s: &Session) -> (Root, Root) {
    let roots = module_roots(s.ir());
    let first = roots.first().expect("module instance").clone();
    let module = |r: &Root| {
        s.ir()
            .instances
            .iter()
            .find(|i| i.id == r.0)
            .and_then(|i| i.module.clone())
    };
    let second = roots
        .iter()
        .find(|r| **r != first && module(r) == module(&first))
        .expect("second instance of the same module")
        .clone();
    (first, second)
}

/// 呼び出しでない最初の Step の実体（移動先にできる）。
fn first_plain(s: &Session, root: &Root) -> StepUid {
    s.steps_of(root)
        .into_iter()
        .find(|u| {
            s.ir()
                .nodes
                .iter()
                .any(|n| n.uid == u.0 && n.enter.is_none())
        })
        .expect("plain step")
}

fn counterpart(s: &Session, uid: &StepUid, other: &Root) -> StepUid {
    s.steps_of(other)
        .into_iter()
        .find(|u| step_name(u) == step_name(uid))
        .expect("same definition in the other instance")
}

fn go_to(s: &mut Session, uid: &StepUid) {
    s.apply(Op::Control(Control::GoTo(uid.clone())));
}

#[test]
fn instances_include_main_and_every_module_instance() {
    let s = Session::new(ir());
    let roots = s.instances();
    assert!(roots.contains(&Root("main".into())));
    assert_eq!(roots.len(), s.ir().instances.len());
    assert!(module_roots(s.ir()).len() >= 2);
}

#[test]
fn same_definition_is_a_separate_step_instance_per_round() {
    let s = Session::new(ir());
    let (r1, r2) = two_roots(&s);
    let a = s.steps_of(&r1);
    let b = s.steps_of(&r2);
    assert!(!a.is_empty());
    assert_eq!(a.len(), b.len());
    let names_a: Vec<&str> = a.iter().map(step_name).collect();
    let names_b: Vec<&str> = b.iter().map(step_name).collect();
    assert_eq!(names_a, names_b);
    assert!(a.iter().all(|u| !b.contains(u)));
}

#[test]
fn entering_a_step_in_one_round_does_not_touch_the_other_round() {
    let s = Session::new(ir());
    let (r1, r2) = two_roots(&s);
    let in1 = first_plain(&s, &r1);
    let in2 = counterpart(&s, &in1, &r2);
    // 開始（`Control::Begin`）は未実装のため、最初の移動の元になる位置を置く。
    let mut s2 = Session::entered_at(s.ir().clone(), &in1.0);
    assert_eq!(s2.step_status(&in1), StepStatus::Entered);
    assert_eq!(s2.step_status(&in2), StepStatus::NotStarted);
    assert!(s2.slice(&r2).is_none());

    go_to(&mut s2, &in2);
    assert_eq!(s2.step_status(&in2), StepStatus::Entered);
    assert_eq!(s2.step_status(&in1), StepStatus::Entered);

    let a = s2.slice(&r1).expect("round1 slice");
    let b = s2.slice(&r2).expect("round2 slice");
    assert!(a.status.keys().all(|k| k.root() == r1));
    assert!(b.status.keys().all(|k| k.root() == r2));
}
