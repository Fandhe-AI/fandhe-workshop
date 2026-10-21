//! 開始点の `requires`・`unmet`（TASK-51、PACK-18、EP-9）の受け入れ試験。
//!
//! 片方だけを書くと `compile`・`validate` の両方が `E-STRUCTURE` にすること（章・モジュール・`null` の扱い）と、
//! 両方を書いた教材の IR に対して core の関門 `start_gate::check_start` が、満たすときは開始してよく、
//! 満たさないときは断って `unmet` の文面を返し状態を変えないことを確かめる。
//! 変異注入 `mutations/PACK18-1` が片方だけの検査の欠落を検出する。呼び出し元は CI の `make test` のみである。

mod common;

use common::{assert_no_secret, testdata, Tmp, SECRET};

use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Root, StepUid};
use fandhe_workshop_core::event::RefuseReason;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::start_gate::{check_start, StartRefusal};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP: &str = "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}\nrejections: {rj-x: {category: not_in_this_step, message: m, next_action: n}}\n";

fn chapter(start: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{start}]\nsteps:\n  - {{id: a, title: A, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    )
}

fn pack(start: &str) -> Tmp {
    let t = Tmp::new("start-requires");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter(start).as_bytes());
    t
}

/// `compile` と `validate` の診断から `E-STRUCTURE` だけを取り出す。
fn structure(t: &Tmp) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let c = compile_dir(&t.0).err().unwrap_or_default();
    let v = validate_dir(&t.0);
    let only = |d: Vec<Diagnostic>| -> Vec<Diagnostic> {
        d.into_iter()
            .filter(|x| x.code() == DiagCode::Structure)
            .collect()
    };
    (only(c), only(v))
}

fn assert_one_structure(t: &Tmp, file: &str, item: &str) {
    let (c, v) = structure(t);
    for d in [&c, &v] {
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].file(), file, "{d:?}");
        assert_eq!(d[0].item(), item, "{d:?}");
        assert!(d[0].message().contains("requires and unmet"), "{d:?}");
        assert_no_secret(d);
    }
}

#[test]
fn pack18_requires_without_unmet_is_e_structure() {
    let t = pack(&format!(
        "{{id: s1, node: a, requires: {{flag: {SECRET}}}}}"
    ));
    assert_one_structure(&t, "chapters/c1/chapter.yaml", "start:s1");
}

#[test]
fn pack18_unmet_without_requires_is_e_structure() {
    let t = pack("{id: s1, node: a, unmet: rj-x}");
    assert_one_structure(&t, "chapters/c1/chapter.yaml", "start:s1");
}

#[test]
fn pack18_both_or_neither_compile() {
    let t = pack("{id: s1, node: a, requires: {flag: f}, unmet: rj-x}");
    let bytes = compile_dir(&t.0).expect("both compile");
    let ir: Ir = serde_json::from_slice(&bytes).expect("ir");
    assert!(ir.starts[0].requires.is_some() && ir.starts[0].unmet.is_some());
    assert!(validate_dir(&t.0).is_empty());
    let t = pack("{id: s1, node: a}");
    compile_dir(&t.0).expect("neither compiles");
    assert!(validate_dir(&t.0).is_empty());
}

#[test]
fn pack18_requires_null_counts_as_omitted() {
    let t = pack("{id: s1, node: a, requires: null, unmet: rj-x}");
    assert_one_structure(&t, "chapters/c1/chapter.yaml", "start:s1");
    let t = pack("{id: s1, node: a, requires: null}");
    compile_dir(&t.0).expect("null alone is omitted");
    assert!(validate_dir(&t.0).is_empty());
}

#[test]
fn pack18_one_sided_in_module_start_is_e_structure() {
    // モジュールは章から呼ばれなくても読み込まれ、モジュールの定義ファイルで報告される。
    let t = Tmp::new("start-requires-module");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        chapter("{id: s1, node: a}").as_bytes(),
    );
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: M\nstarts: [{id: ms, node: ma, requires: {flag: f}}]\nsteps:\n  - {id: ma, title: MA, final: true}\n",
    );
    assert_one_structure(&t, "modules/m1/module.yaml", "start:ms");
}

/// 変換した IR を core の関門にかけ、満たさない状態では断って文面を返し、満たす状態では開始してよいことを見る。
fn gate_round(pack: &str, start: &str, step_root: &str, step: &str, text: &str) {
    let bytes = compile_dir(&testdata(pack)).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let state = SessionState::default();
    let before = serde_json::to_vec(&state).expect("ser");
    let err = check_start(&ir, &state.run, start).expect_err("unmet refuses");
    assert_eq!(err.reason(), RefuseReason::StartUnmet);
    let StartRefusal::Unmet { text: got, .. } = err else {
        panic!("expected Unmet");
    };
    let got = got.expect("text resolves");
    assert_eq!(got.text, text);
    assert!(!got.next_action.as_str().trim().is_empty());
    assert_eq!(before, serde_json::to_vec(&state).expect("ser"));

    let mut run = RunState::default();
    let mut slice = Slice::default();
    slice
        .status
        .insert(StepUid(format!("{step_root}/{step}")), StepStatus::Achieved);
    run.slices.insert(Root(step_root.to_string()), slice);
    assert!(check_start(&ir, &run, start).is_ok());
}

#[test]
fn pack18_compiled_ir_gate_refuses_then_opens() {
    gate_round(
        "condition-vocab-pack",
        "s-13-2",
        "main",
        "st-13-1",
        "rj-start-needs-previous",
    );
}

#[test]
fn pack18_compiled_ir_gate_refuses_then_opens_for_rounds_pack() {
    gate_round(
        "module-reuse-rounds/test-data/pack",
        "s-r2",
        "main",
        "r1-impl",
        "rj-start-needs-previous-round",
    );
}
