//! 読むだけの Step を 1 択の `choice` と確認 `answer` で書けること、入力のない確認だけの Step が `E-STRUCTURE` で
//! 止まることの固定（TASK-63 2/2、PACK-35・PACK-19・D-69）。
//!
//! 決定（D-69、2026-10-21）は「入力のない確認だけの Step を表す語は足さず、選択肢が 1 つだけの `choice` で表す」である。
//! 受講者の操作が 1 回増えることは限界として受け入れる（TERM-30）。本試験は次を固定する。
//!
//! - 実パック（`tests/data/read-only-step-pack`）が、新しい語彙の版を宣言せず（`pack.vocab: '1.3'`。`answer` の `label` の必須化〔PACK-37〕で、1.3 未満では書けない）検査を通り、IR になる
//! - 達成までの操作が「1 択の選択 1 回 + 回答」であること（限界の観察）
//! - 入力も `use` も無く終端でもない Step が `confirm` だけを持つと、確認が届かないため `E-STRUCTURE` になる
//!   （章・モジュールの定義ごとに 1 件。`confirm` 側で重ねて報告しない）。終端の Step は対象外
//!
//! 範囲外: `fandhe-pack validate` の CLI（TASK-403）、`Event` から `CheckObservation` への写像（TASK-304）、
//! 読むだけの Step の画面（TERM-30）。確認も持たない途中の Step の拒否は `pack19_bare_step.rs`。
//! `validate` の CLI は未実装のため、`validate_dir` と、同じ読み込みを通る
//! `compile`・`vocab` の CLI で代える。
//!
//! 呼び出し元は CI の `make test` のみである。試験では `expect` を使ってよい。

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::Tmp;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_contract::lo_check::{
    derive_records, CheckMethod, CheckObservation, CheckTarget, FirstAnswer, Reselection,
};
use fandhe_workshop_core::addr::{Generation, StepUid};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::{RunState, SessionState, Slice};
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn pack_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/read-only-step-pack")
}

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
}

fn ir_of(dir: &Path) -> Ir {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("compile {dir:?}: {d:?}"));
    load_ir(&bytes).expect("the engine loads the IR")
}

fn uid_of(ir: &Ir, step_id: &str) -> String {
    ir.nodes
        .iter()
        .find(|n| n.step.id == step_id)
        .unwrap_or_else(|| panic!("step {step_id} がある"))
        .uid
        .clone()
}

#[test]
fn pack35_one_choice_pack_validates_without_newer_vocab() {
    let dir = pack_dir();
    let diags = validate_dir(&dir);
    assert!(diags.is_empty(), "{diags:?}");
    assert!(compile_dir(&dir).is_ok());
}

#[test]
fn pack35_cli_compile_and_vocab_accept_one_choice_pack() {
    let dir = pack_dir();
    // CLI の validate は TASK-403（#647）まで未実装のため、同じ読み込みを通る compile で代える。
    let o = bin()
        .arg("compile")
        .arg(&dir)
        .output()
        .expect("run compile");
    assert_eq!(o.status.code(), Some(0));
    assert!(!o.stdout.is_empty());
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );

    let o = bin().arg("vocab").arg(&dir).output().expect("run vocab");
    assert_eq!(o.status.code(), Some(0));
    let out = String::from_utf8(o.stdout).expect("utf8");
    // 第 4 欄（extension_points）が空 = 名前つき拡張点を使っていない。
    assert!(out.contains("c1,read-1,step,\n"), "{out}");
    assert!(
        out.contains("(ii) 名前つき拡張点を使って書けた Step: 0"),
        "{out}"
    );
}

fn entered(uid: &str) -> SessionState {
    let u = StepUid(uid.to_string());
    let mut slice = Slice::default();
    slice.status.insert(u.clone(), StepStatus::Entered);
    let mut run = RunState {
        generation: Generation(1),
        current: Some(u.clone()),
        ..RunState::default()
    };
    run.slices.insert(u.root(), slice);
    SessionState {
        run,
        ..SessionState::default()
    }
}

fn status(s: &Session, uid: &str) -> StepStatus {
    let u = StepUid(uid.to_string());
    s.state()
        .run
        .slices
        .get(&u.root())
        .and_then(|sl| sl.status.get(&u))
        .copied()
        .unwrap_or_default()
}

fn achieved_count(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

/// 選択を送り、確認の段階（送る前が `Accepted`）での回答だけを観測にして返す。
/// 正誤は戻りに `Achieved` があるかで決める（`confirm_record_boundary.rs` の `choose` と同じ規則）。
fn choose(s: &mut Session, uid: &str, choice: &str) -> Option<CheckObservation> {
    let confirming = status(s, uid) == StepStatus::Accepted;
    let events = s.apply(Op::Input(Input::Choice(choice.into()))).to_vec();
    confirming.then(|| CheckObservation::Answered {
        step: uid.into(),
        correct: achieved_count(&events) > 0,
    })
}

#[test]
fn pack35_one_extra_operation_then_answer() {
    let ir = ir_of(&pack_dir());
    let uid = uid_of(&ir, "read-1");
    let node = ir.nodes.iter().find(|n| n.uid == uid).expect("node");
    let tg = vec![CheckTarget {
        step: uid.clone(),
        lo: node.step.lo.clone(),
        method: CheckMethod::Answer,
    }];
    let mut s = Session::from_state(ir, entered(&uid));
    let mut obs = Vec::new();

    // 1 択の選択 1 回で受付になるが、達成ではない（受講者の操作が 1 回増える。TERM-30 の限界）。
    assert_eq!(choose(&mut s, &uid, "opt-read"), None);
    assert_eq!(status(&s, &uid), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), 0);
    // 誤答は受付済みのまま。
    obs.extend(choose(&mut s, &uid, "ans-ng"));
    assert_eq!(status(&s, &uid), StepStatus::Accepted);
    assert_eq!(achieved_count(s.log()), 0);
    // 正答で達成。
    obs.extend(choose(&mut s, &uid, "ans-ok"));
    assert_eq!(status(&s, &uid), StepStatus::Achieved);

    let records = derive_records(&tg, &obs);
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].first_answer, FirstAnswer::Wrong);
    assert_eq!(records[0].reselection, Reselection::Correct);
}

const WORKSHOP: &str = "pack: {id: p, title: P, version: '0.0.1', ir_format: 1, guidance: A, vocab: '1.3'}\nrejections:\n  rj-wrong: {category: not_in_this_step, message: no, next_action: retry}\n";
const CONFIRM_ANSWER: &str = "{kind: answer, on_wrong: rj-wrong, options: [{id: ans-ID-MARK, label: A, correct: true}, {id: ans-ng, label: B, correct: false}]}";

fn chapter_with(steps: &str) -> Tmp {
    let t = Tmp::new("pack35");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let ch = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n{steps}  - {{id: end, title: End, body: a.md, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t
}

fn only_structure(d: &[Diagnostic], file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Structure, "{d:?}");
    assert_eq!(d[0].file(), file);
    assert_eq!(d[0].item(), item);
    let m = d[0].to_string();
    for leaked in ["ID-MARK", "rj-wrong", "SECRET"] {
        assert!(!m.contains(leaked), "{m}");
    }
}

#[test]
fn pack35_inputless_confirm_step_is_structure_error() {
    let t = chapter_with(&format!(
        "  - {{id: a, title: A, body: a.md, confirm: {CONFIRM_ANSWER}, next: [{{to: end}}]}}\n"
    ));
    only_structure(&validate_dir(&t.0), "chapters/c1/chapter.yaml", "step:a");
    let d = compile_dir(&t.0).expect_err("structure");
    only_structure(&d, "chapters/c1/chapter.yaml", "step:a");
}

#[test]
fn pack35_inputless_confirm_in_module_is_structure_error() {
    for confirm in [CONFIRM_ANSWER, "{kind: none, reason: \"${why}\"}"] {
        let t = Tmp::new("pack35mod");
        t.put("workshop.yaml", WORKSHOP.as_bytes());
        t.put("chapters/c1/a.md", b"# a\n");
        t.put(
            "chapters/c1/chapter.yaml",
            b"id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U1, next: [{to: u2}], use: {module: m, instance: i1, args: {why: SECRET}}}\n  - {id: u2, title: U2, final: true, use: {module: m, instance: i2, args: {why: SECRET}}}\n",
        );
        t.put("modules/m/a.md", b"# a\n");
        let module = format!(
            "id: m\ntitle: M\nparams: {{why: d}}\nsteps:\n  - {{id: a, title: A, body: a.md, confirm: {confirm}, next: [{{to: z}}]}}\n  - {{id: z, title: Z, body: a.md, final: true}}\n"
        );
        t.put("modules/m/module.yaml", module.as_bytes());
        // 実体は 2 つあるが、診断は定義ごとに 1 件。
        only_structure(&validate_dir(&t.0), "modules/m/module.yaml", "step:a");
        let d = compile_dir(&t.0).expect_err("structure");
        only_structure(&d, "modules/m/module.yaml", "step:a");
    }
}

#[test]
fn pack35_cli_compile_rejects_inputless_confirm_step() {
    let t = chapter_with(&format!(
        "  - {{id: a, title: A, body: a.md, confirm: {CONFIRM_ANSWER}, next: [{{to: end}}]}}\n"
    ));
    let o = bin()
        .arg("compile")
        .arg(&t.0)
        .output()
        .expect("run compile");
    assert_eq!(o.status.code(), Some(1));
    let out = String::from_utf8(o.stdout).expect("utf8");
    assert!(out.contains("E-STRUCTURE"), "{out}");
    assert!(
        o.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&o.stderr)
    );
}

#[test]
fn pack35_final_step_without_input_is_allowed() {
    let t = Tmp::new("pack35final");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let ch = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: end}}]\nsteps:\n  - {{id: end, title: End, body: a.md, final: true, confirm: {CONFIRM_ANSWER}}}\n"
    );
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
}
