//! ヒントの上限の規則（TASK-296、GUIDE-6）を、教材から IR を作って core の `apply` まで通して確かめる試験。
//!
//! 上限は「現在の案内レベルで出せる数 n まで」で、3 段に固定しない（D-55）。合成の教材
//! （`tests/data/hint-limit-pack`。13 Step x 3 段 = 39 本と、水準で段数が変わる 2 Step）を `compile_dir` で IR にし、
//! 19 件の確認（13 Step の全水準 + 水準で段数が変わる 2 Step x 3 水準のうち 6 件）で、期待するヒントの並びが出て、
//! n 回までは `HintOpened`、n+1 回目は `NoHint` の拒否になり、世代が変わらないことを確かめる。
//!
//! 開始点の案内レベルの確認（試作の 14 件）は流さない。`starts[].guidance` を開始時に反映する処理（TASK-294）が
//! 入るまで流せないため。`fandhe-pack test` の計数（TASK-404）も未実装で、ここでは native の試験で同等の形を流す。
//! 試験では `unwrap`・`expect` を使ってよい。

mod common;

use std::path::Path;

use common::assert_no_secret;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{Event, RefuseReason};
use fandhe_workshop_core::hint_select::hints_at;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::{apply, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;

fn pack_dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/hint-limit-pack")
}

fn ir_of(dir: &Path) -> Ir {
    serde_json::from_slice(&compile_dir(dir).expect("compile")).expect("ir")
}

fn uid_of(ir: &Ir, step: &str) -> String {
    ir.nodes
        .iter()
        .find(|n| n.step.id == step)
        .unwrap_or_else(|| panic!("no node for {step}"))
        .uid
        .clone()
}

/// 確認 1 件: (Step, 水準, 期待するヒントの ID の並び)。
type Check = (String, GuidanceLevel, Vec<String>);

fn checks() -> Vec<Check> {
    let mut out = Vec::new();
    // 13 Step x 3 段（全水準で 3 段）。水準ごとに流すと 39 件になるため、試作と同じく水準 A の 13 件にする。
    for i in 1..=13 {
        let s = format!("s{i:02}");
        let want = (1..=3).map(|k| format!("h-{s}-{k}")).collect();
        out.push((s, GuidanceLevel::A, want));
    }
    let ids = |v: &[&str]| v.iter().map(|s| (*s).to_string()).collect::<Vec<_>>();
    let a = ids(&["h-g3a-1"]);
    let a_all = ids(&["h-g3a-1", "h-g3a-2"]);
    let b = ids(&["h-g3b-1"]);
    let b_all = ids(&["h-g3b-1", "h-g3b-2", "h-g3b-3"]);
    out.push(("g3a".into(), GuidanceLevel::A, a));
    out.push(("g3a".into(), GuidanceLevel::B, a_all.clone()));
    out.push(("g3a".into(), GuidanceLevel::C, a_all));
    out.push(("g3b".into(), GuidanceLevel::A, b));
    out.push(("g3b".into(), GuidanceLevel::B, b_all.clone()));
    out.push(("g3b".into(), GuidanceLevel::C, b_all));
    out
}

#[test]
fn the_pack_is_clean_and_has_39_hints_for_13_steps() {
    let dir = pack_dir();
    let diags = validate_dir(&dir);
    assert_no_secret(&diags);
    assert!(diags.is_empty(), "diagnostics: {diags:?}");
    let ir = ir_of(&dir);
    let total: usize = (1..=13)
        .map(|i| {
            let uid = StepUid(uid_of(&ir, &format!("s{i:02}")));
            hints_at(&ir, &uid, GuidanceLevel::A).len()
        })
        .sum();
    assert_eq!(total, 39);
}

#[test]
fn hint_checks_19_pass_through_compile_and_open() {
    let ir = ir_of(&pack_dir());
    let list = checks();
    assert_eq!(list.len(), 19);
    let mut ran = 0;
    for (step, level, want) in &list {
        let uid = uid_of(&ir, step);
        let got: Vec<String> = hints_at(&ir, &StepUid(uid.clone()), *level)
            .into_iter()
            .map(str::to_string)
            .collect();
        assert_eq!(&got, want, "hints of {step} at {level:?}");

        let mut state = Session::entered_at(ir.clone(), &uid).state().clone();
        state.run.guidance_level = *level;
        let generation = state.run.generation;
        let n = u32::try_from(want.len()).unwrap();
        for i in 1..=n {
            let (next, events) = apply(&ir, state, Op::Control(Control::OpenHint));
            state = next;
            assert_eq!(events.len(), 1, "{step} {level:?} open {i}");
            match &events[0] {
                Event::HintOpened { opened, total, .. } => {
                    assert_eq!((*opened, *total), (i, n), "{step} {level:?}");
                }
                other => panic!("{step} {level:?} open {i}: {other:?}"),
            }
        }
        let (next, events) = apply(&ir, state, Op::Control(Control::OpenHint));
        assert!(
            matches!(
                events.as_slice(),
                [Event::Refused {
                    reason: RefuseReason::NoHint,
                    ..
                }]
            ),
            "{step} {level:?} beyond n: {events:?}"
        );
        assert_eq!(next.run.generation, generation);
        assert_eq!(next.hints_opened[&StepUid(uid)], n);
        ran += 1;
    }
    assert_eq!(ran, 19);
}
