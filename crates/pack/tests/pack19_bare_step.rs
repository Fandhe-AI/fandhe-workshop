//! 入力も `use` も確認も持たない途中の Step が `E-STRUCTURE` で止まることの固定（#1709。PACK-19・D-69。
//! TASK-63 の確認だけを持つ Step の拒否〔`read_only_step.rs`〕の残り）。
//!
//! 受付が起きない Step は先へ進めないため、`validate` と `compile` の両方が定義（章・モジュール）ごとに 1 件で拒否する。
//! 終端の Step・`input` を持つ Step は拒否しない（対照）。確認だけを持つ Step は既存の拒否のままで診断は重ならない。
//!
//! 範囲外: 画面の側の挙動、`fandhe-pack validate` の CLI（TASK-403）。
//! 呼び出し元は CI の `make test` のみである。試験では `expect` を使ってよい。

mod common;

use std::process::Command;

use common::{Tmp, SECRET};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP: &str = "pack: {id: p, title: P, version: '0.0.1', ir_format: 1, guidance: A, vocab: '1.3'}\nrejections:\n  rj-wrong: {category: not_in_this_step, message: no, next_action: retry}\n";
const CHOICE: &str = "input: {kind: choice, options: [{id: opt-a, label: A}]}, rules: [{id: r-a, on: {option: opt-a}, outcome: {kind: accept}}]";

fn chapter_with(steps: &str) -> Tmp {
    let t = Tmp::new("pack19bare");
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
    assert!(!d[0].to_string().contains(SECRET), "{d:?}");
}

#[test]
fn pack19_bare_intermediate_step_is_structure_error() {
    let t = chapter_with("  - {id: a, title: A, body: a.md, next: [{to: end}]}\n");
    only_structure(&validate_dir(&t.0), "chapters/c1/chapter.yaml", "step:a");
    let d = compile_dir(&t.0).expect_err("structure");
    only_structure(&d, "chapters/c1/chapter.yaml", "step:a");
}

#[test]
fn pack19_bare_step_in_module_is_reported_once() {
    let t = Tmp::new("pack19mod");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let ch = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U1, next: [{{to: u2}}], use: {{module: m, instance: i1, args: {{why: {SECRET}}}}}}}\n  - {{id: u2, title: U2, final: true, use: {{module: m, instance: i2, args: {{why: {SECRET}}}}}}}\n"
    );
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t.put("modules/m/a.md", b"# a\n");
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nparams: {why: d}\nsteps:\n  - {id: a, title: A, body: a.md, next: [{to: z}]}\n  - {id: z, title: Z, body: a.md, final: true}\n",
    );
    // 実体は 2 つあるが、診断は定義ごとに 1 件。
    only_structure(&validate_dir(&t.0), "modules/m/module.yaml", "step:a");
    let d = compile_dir(&t.0).expect_err("structure");
    only_structure(&d, "modules/m/module.yaml", "step:a");
}

#[test]
fn pack19_cli_compile_rejects_bare_step() {
    let t = chapter_with("  - {id: a, title: A, body: a.md, next: [{to: end}]}\n");
    let o = Command::new(env!("CARGO_BIN_EXE_fandhe-pack"))
        .arg("compile")
        .arg(&t.0)
        .output()
        .expect("run compile");
    assert_eq!(o.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&o.stdout).contains("E-STRUCTURE"));
    assert!(o.stderr.is_empty());
}

#[test]
fn pack19_controls_are_not_rejected() {
    // 終端だけの章。
    let t = Tmp::new("pack19final");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: end}]\nsteps:\n  - {id: end, title: End, body: a.md, final: true}\n",
    );
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
    assert!(compile_dir(&t.0).is_ok());

    // input を持つ途中の Step。
    let t = chapter_with(&format!(
        "  - {{id: a, title: A, body: a.md, {CHOICE}, confirm: {{kind: none, reason: r}}, next: [{{to: end}}]}}\n"
    ));
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
    assert!(compile_dir(&t.0).is_ok());
}

#[test]
fn pack19_confirm_only_step_keeps_one_diagnostic() {
    let t = chapter_with(
        "  - {id: a, title: A, body: a.md, confirm: {kind: none, reason: r}, next: [{to: end}]}\n",
    );
    let d = validate_dir(&t.0);
    only_structure(&d, "chapters/c1/chapter.yaml", "step:a");
    assert!(d[0]
        .to_string()
        .contains("needs `input` unless it is final"));
}
