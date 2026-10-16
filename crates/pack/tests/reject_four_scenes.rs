//! 受け付けない入力の 4 場面（未知のコマンド・順序違い・引数不足・引数の形）の教材テスト（TASK-268 2/2、JUDGE-4）。
//!
//! 教材パックの誤入力（`error_inputs`）をコンパイルした IR とエンジンの判定に流し、期待の分類との一致が 100% であること、
//! 拒否に付く「次の行動」が空でないことを数える。誤入力の出どころは、judgment-pack・min-3step-pack・module-reuse-rounds
//! の 3 教材で、4 分類を期待にもつ組を各 4 件、計 16 件使う（件数は固定し、データが減っても素通りしないようにする）。
//!
//! 判定の入口は 2 つある。`command_class` は `judge_input` に未結線のため（結線は TASK-269 か追随の Issue）、
//! `unknown_command`・`wrong_order` は `command_class::classify_command` と `rejection_text::resolve_rejection` で、
//! `missing_arg`・`bad_arg_shape` は本番の判定の経路 `judge::judge_input` で判定する。合成の順は
//! 「`command_class` が分類すればそれ、しなければ `judge_input`」で、結線した後も同じ結果になる。
//! 開始点の効果は当てない。4 分類は規則の `when` の評価より前に決まるためである。
//!
//! 確かめないこと: `not_in_this_step`・`not_supported`・`busy`（TASK-269）、全誤入力を流す実行系（`fandhe-pack test`、TASK-404）。

mod common;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use common::testdata;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_contract::judgment::{Judgment, RejectCategory};
use fandhe_workshop_contract::profile::read_profile;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::command_class::{classify_command, CommandClass};
use fandhe_workshop_core::command_lex::lex_command;
use fandhe_workshop_core::effect::NoConditions;
use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::judge::judge_input;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::rejection_text::{resolve_rejection, RejectionSource};
use fandhe_workshop_core::session::{RunState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::play_expect::{read_error_input_expects, ResultExpect};
use fandhe_workshop_pack::source;

const SCENES: [RejectCategory; 4] = [
    RejectCategory::UnknownCommand,
    RejectCategory::WrongOrder,
    RejectCategory::MissingArg,
    RejectCategory::BadArgShape,
];

fn packs() -> Vec<(&'static str, PathBuf)> {
    vec![
        (
            "judgment-pack",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/judgment-pack"),
        ),
        (
            "min-3step-pack",
            testdata("engine-event-consistency/test-data/min-3step-pack"),
        ),
        (
            "module-reuse-rounds",
            testdata("module-reuse-rounds/test-data/pack"),
        ),
    ]
}

/// 1 件の誤入力と、それを流す先の IR（`load` が返す列の添字）。
struct Case {
    label: String,
    ir_index: usize,
    start: String,
    line: String,
    want: RejectCategory,
}

fn compile(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("load_ir")
}

fn load() -> (Vec<Ir>, Vec<Case>) {
    let mut irs = Vec::new();
    let mut cases = Vec::new();
    for (name, dir) in packs() {
        let lp = source::load(&dir).expect("load");
        let expects = read_error_input_expects(&lp).expect("expects");
        let tests = lp.tests.as_ref().expect("tests");
        assert_eq!(expects.len(), tests.error_inputs.len());
        let ir_index = irs.len();
        irs.push(compile(&dir));
        for ((id, expect), v) in expects.iter().zip(&tests.error_inputs) {
            let ResultExpect::Rejected(want) = expect else {
                continue;
            };
            if !SCENES.contains(want) {
                continue;
            }
            // `cmd` を持たない誤入力（4 場面では起きない）は、黙って外さず失敗にする。
            let line = v["input"]["cmd"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}/{id}: cmd is missing"));
            let start = v["start"]
                .as_str()
                .unwrap_or_else(|| panic!("{name}/{id}: start is missing"));
            cases.push(Case {
                label: format!("{name}/{id}"),
                ir_index,
                start: start.to_string(),
                line: line.to_string(),
                want: *want,
            });
        }
    }
    (irs, cases)
}

fn node_of<'a>(ir: &'a Ir, start_id: &str) -> &'a Node {
    let start = ir
        .starts
        .iter()
        .find(|s| s.id == start_id)
        .unwrap_or_else(|| panic!("start {start_id} not found"));
    ir.nodes
        .iter()
        .find(|n| n.uid == start.node)
        .unwrap_or_else(|| panic!("node {} not found", start.node))
}

/// 開始点の Step に入った状態。効果は当てない。
fn run_at(node: &Node) -> RunState {
    let uid = StepUid(node.uid.clone());
    let mut slice = Slice::default();
    slice.status.insert(uid.clone(), StepStatus::Entered);
    let mut slices = BTreeMap::new();
    slices.insert(uid.root(), slice);
    RunState {
        current: Some(uid),
        slices,
        ..RunState::default()
    }
}

fn judge_line(ir: &Ir, node: &Node, line: &str) -> Judgment {
    judge_input(
        ir,
        &run_at(node),
        &Input::Command(line.to_string()),
        &NoConditions,
    )
    .unwrap_or_else(|e| panic!("judge_input: {e}"))
    .judgment
}

/// 合成の入口の結果。`via_judge` は `judge_input` で判定したか。
struct Scene {
    judgment: Judgment,
    via_judge: bool,
}

fn judge_scene(ir: &Ir, c: &Case) -> Scene {
    let node = node_of(ir, &c.start);
    let prefix = read_profile(ir.pack.profile.as_ref())
        .expect("profile")
        .command_prefix;
    let lexed = lex_command(&prefix, &c.line).unwrap_or_else(|_| panic!("{}: lex", c.label));
    let class = classify_command(ir, &node.step, StepStatus::Entered, &lexed)
        .unwrap_or_else(|_| panic!("{}: classify", c.label));
    if let Some(cat) = class.category() {
        let r = resolve_rejection(ir, &node.step, RejectionSource::Category(cat))
            .unwrap_or_else(|_| panic!("{}: resolve", c.label));
        return Scene {
            judgment: Judgment::Rejected(r),
            via_judge: false,
        };
    }
    assert_eq!(class, CommandClass::Accepting, "{}", c.label);
    Scene {
        judgment: judge_line(ir, node, &c.line),
        via_judge: true,
    }
}

#[test]
fn four_scenes_are_classified_100_percent() {
    let (irs, cases) = load();
    assert_eq!(cases.len(), 16);

    let mut per: BTreeMap<&str, usize> = BTreeMap::new();
    let mut matched = 0;
    let mut empty_next = 0;
    for c in &cases {
        let scene = judge_scene(&irs[c.ir_index], c);
        if ResultExpect::Rejected(c.want).matches(&scene.judgment) {
            matched += 1;
        } else {
            eprintln!("{}: got {:?}", c.label, scene.judgment);
        }
        if let Judgment::Rejected(r) = &scene.judgment {
            if r.next_action.as_str().trim().is_empty() {
                empty_next += 1;
            }
        }
        *per.entry(c.want.as_str()).or_default() += 1;
    }
    assert_eq!(matched, cases.len(), "category match rate is not 100%");
    assert_eq!(empty_next, 0);
    let want: BTreeMap<&str, usize> = SCENES.iter().map(|c| (c.as_str(), 4)).collect();
    assert_eq!(per, want);
}

#[test]
fn missing_and_shape_go_through_judge_input() {
    let (irs, cases) = load();
    let mut n = 0;
    for c in cases.iter().filter(|c| {
        matches!(
            c.want,
            RejectCategory::MissingArg | RejectCategory::BadArgShape
        )
    }) {
        let scene = judge_scene(&irs[c.ir_index], c);
        assert!(scene.via_judge, "{}: not judged by judge_input", c.label);
        assert!(
            ResultExpect::Rejected(c.want).matches(&scene.judgment),
            "{}",
            c.label
        );
        n += 1;
    }
    assert_eq!(n, 8);
}

fn text_id(cases: &[Case], irs: &[Ir], label: &str) -> String {
    let c = cases
        .iter()
        .find(|c| c.label == label)
        .unwrap_or_else(|| panic!("{label} not found"));
    match judge_scene(&irs[c.ir_index], c).judgment {
        Judgment::Rejected(r) => r.text,
        other => panic!("{label}: {other:?}"),
    }
}

#[test]
fn rejection_text_prefers_step_on_reject() {
    let (irs, cases) = load();
    assert_eq!(
        text_id(&cases, &irs, "judgment-pack/e-missing-arg"),
        "rj-run-missing-mode"
    );
    assert_eq!(
        text_id(&cases, &irs, "module-reuse-rounds/e02"),
        "rj-6-1-missing-input"
    );
    // Step に on_reject が無い場面は defaults の文面になる。
    let want = irs[0]
        .defaults
        .get("bad_arg_shape")
        .expect("default")
        .clone();
    assert_eq!(text_id(&cases, &irs, "judgment-pack/e-shape-1"), want);
}

#[test]
fn missing_arg_is_not_clarified() {
    let (irs, _) = load();
    let ir = &irs[0];
    assert!(matches!(
        judge_line(ir, node_of(ir, "s-clarify"), "/make-idea"),
        Judgment::Clarified(_)
    ));
    assert!(matches!(
        judge_line(ir, node_of(ir, "s-two-args"), "/start-run 42"),
        Judgment::Rejected(r) if r.category == RejectCategory::MissingArg
    ));
}
