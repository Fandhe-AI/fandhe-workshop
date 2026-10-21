//! 誤答ごとの拒否文面 `on_wrong` の検査の受け入れ試験（PACK-15、EP-6、TASK-46 1/2）。
//!
//! 正答の選択肢に `on_wrong` を書くと `E-STRUCTURE`、存在しない文面を指すと `E-REF-REJECTION` になることを、
//! 期待のファイル・項目で確かめる。実行時に誤答の文面が「選択肢の文面、無ければ Step の文面」の順で決まることは、
//! 検証を通ったパックを IR に直して core の判定に通して確かめる（core のコードは変えない）。
//! 診断に教材の本文・モジュールの引数の値が出ないことも確かめる。

mod common;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_core::input::Input;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::select_judge::{judge_confirm, ConfirmVerdict};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
rejections:
    rj-default: {category: not_in_this_step, message: m, next_action: n}
    rj-own: {category: not_in_this_step, message: m, next_action: n}
    rj-other: {category: not_in_this_step, message: m, next_action: n}
";

const OPTIONS: &str = "{id: ok, label: A, correct: true}, {id: wrong-own, label: B, correct: false, on_wrong: rj-own}, {id: wrong-default, label: C, correct: false}";

/// Step `a` の `confirm` に、Step の `on_wrong` と選択肢の並びを差し込んだパックを書き出す。
fn pack(step_on_wrong: &str, options: &str) -> Tmp {
    let t = Tmp::new("onwrong");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - id: a\n    title: A\n    body: a.md\n    input: {{kind: choice, options: [{{id: ok, label: A}}, {{id: wrong-own, label: B}}, {{id: wrong-default, label: C}}]}}\n    confirm: {{kind: answer, on_wrong: {step_on_wrong}, options: [{options}]}}\n    next: [{{to: b}}]\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    t
}

fn base() -> Tmp {
    pack("rj-default", OPTIONS)
}

fn only(d: &[Diagnostic], code: DiagCode) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_eq!(d[0].file(), "chapters/c1/chapter.yaml", "{d:?}");
    assert_eq!(d[0].item(), "step:a", "{d:?}");
    assert_no_secret(d);
}

#[test]
fn pack15_base_pack_has_no_diagnostics() {
    let t = base();
    assert!(validate_dir(&t.0).is_empty());
    compile_dir(&t.0).expect("compile");
}

#[test]
fn pack15_on_wrong_on_correct_option_is_structure() {
    let t = pack(
        "rj-default",
        &OPTIONS.replace(
            "{id: ok, label: A, correct: true}",
            "{id: ok, label: A, correct: true, on_wrong: rj-other}",
        ),
    );
    only(&validate_dir(&t.0), DiagCode::Structure);
    only(
        &compile_dir(&t.0).expect_err("structure"),
        DiagCode::Structure,
    );
}

#[test]
fn pack15_undefined_option_on_wrong_is_ref_rejection() {
    let t = pack("rj-default", &OPTIONS.replace("rj-own", "rj-missing"));
    only(&validate_dir(&t.0), DiagCode::RefRejection);
}

#[test]
fn pack15_undefined_step_on_wrong_is_ref_rejection() {
    let t = pack("rj-missing", OPTIONS);
    only(&validate_dir(&t.0), DiagCode::RefRejection);
}

#[test]
fn pack15_module_placeholder_on_correct_option_is_structure() {
    let t = Tmp::new("onwrongmod");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u}]\nsteps:\n  - {id: u, title: U, final: true, use: {module: m, instance: i, args: {w: rj-other-ARG-VALUE}}}\n",
    );
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nparams: {w: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: choice, options: [{id: ok, label: A}]}, confirm: {kind: answer, on_wrong: rj-default, options: [{id: ok, label: A, correct: true, on_wrong: \"${w}\"}]}}\n",
    );
    let d = compile_dir(&t.0).expect_err("structure");
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Structure);
    assert_eq!(d[0].file(), "modules/m/module.yaml");
    assert_eq!(d[0].item(), "step:a");
    assert!(!d[0].to_string().contains("ARG-VALUE"), "{}", d[0]);
}

#[test]
fn pack15_wrong_answer_text_resolves_option_then_step() {
    let t = base();
    let ir = load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir");
    let step = &ir.nodes.iter().find(|n| n.step.id == "a").expect("a").step;
    let verdict = |id: &str| judge_confirm(step, &Input::Choice(id.to_string())).expect("judge");
    assert_eq!(verdict("ok"), ConfirmVerdict::Confirmed);
    assert_eq!(
        verdict("wrong-own"),
        ConfirmVerdict::WrongAnswer {
            option: "wrong-own".into(),
            text: "rj-own".into()
        }
    );
    assert_eq!(
        verdict("wrong-default"),
        ConfirmVerdict::WrongAnswer {
            option: "wrong-default".into(),
            text: "rj-default".into()
        }
    );
}
