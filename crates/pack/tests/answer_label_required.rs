//! 確認 `answer` の選択肢の `label` を必須にする `validate` の検査の受け入れ試験（TASK-65 1/2、PACK-37・D-67）。
//!
//! `label` の無い（欄が無い・`null`）選択肢を持つ教材パックは、`validate` が `E-STRUCTURE` を Step ごとに 1 件報告し、
//! 終了コードの元になる `diag::exit_code` が 1 になる。`compile` は検査しない（`label` の無い IR の golden・凍結物を保つ）。
//!
//! 範囲外: `fandhe-pack validate` の CLI からの確認（本体は TASK-403。ここでは `diag::exit_code` で確かめる）、
//! 置換後に `label` が空になる場合（既存の空白の検査の担当）、入力方式 `choice` の `label`（別の欄。PACK-23）。

mod common;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::diag::exit_code;
use fandhe_workshop_pack::validate::{validate_dir, ANSWER_LABEL_REQUIRED};
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP_YAML: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
rejections: {rj-a: {category: not_in_this_step, message: m, next_action: n}}
";

/// Step `a` の `confirm` の選択肢の並びを差し込んだ章（形式 B）を持つパック。
fn pack(options: &str) -> Tmp {
    let t = Tmp::new("answer-label");
    t.put("workshop.yaml", WORKSHOP_YAML.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - id: a\n    title: A\n    body: a.md\n    input: {{kind: choice, options: [{{id: o1, label: X}}, {{id: o2, label: Y}}]}}\n    confirm: {{kind: answer, on_wrong: rj-a, options: [{options}]}}\n    next: [{{to: b}}]\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", format!("# a\n{SECRET}\n").as_bytes());
    t
}

fn label_diags(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter()
        .filter(|d| d.message() == ANSWER_LABEL_REQUIRED)
        .collect()
}

fn assert_reported_once(d: &[Diagnostic], file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Structure, "{d:?}");
    assert_eq!(d[0].message(), ANSWER_LABEL_REQUIRED, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
    assert_eq!(exit_code(d), 1);
    assert_no_secret(d);
}

#[test]
fn pack37_missing_label_is_reported_with_exit_code_1() {
    let t = pack("{id: o1, label: X, correct: true}, {id: o2, correct: false}");
    let d = validate_dir(&t.0);
    assert_reported_once(&d, "chapters/c1/chapter.yaml", "step:a");
}

#[test]
fn pack37_null_label_is_reported() {
    let t = pack("{id: o1, label: X, correct: true}, {id: o2, label: null, correct: false}");
    let d = validate_dir(&t.0);
    assert_reported_once(&d, "chapters/c1/chapter.yaml", "step:a");
}

#[test]
fn pack37_all_labels_validate_clean() {
    let t = pack("{id: o1, label: X, correct: true}, {id: o2, label: Y, correct: false}");
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
    assert_eq!(exit_code(&d), 0);
}

#[test]
fn pack37_one_diagnostic_per_step() {
    // 1 つの Step で 2 つの選択肢が欠けても 1 件。
    let t = pack("{id: o1, correct: true}, {id: o2, correct: false}");
    let d = validate_dir(&t.0);
    assert_reported_once(&d, "chapters/c1/chapter.yaml", "step:a");

    // 2 つの Step で欠ければ 2 件。
    let ch = "chapters/c1/chapter.yaml";
    let text = std::fs::read_to_string(t.0.join(ch)).expect("read");
    let second = "  - {id: b, title: B, input: {kind: choice, options: [{id: o1, label: X}]}, confirm: {kind: answer, on_wrong: rj-a, options: [{id: o1, correct: true}]}, next: [{to: c}]}\n  - {id: c, title: C, final: true}\n";
    let text = text.replace("  - {id: b, title: B, final: true}\n", second);
    t.put(ch, text.as_bytes());
    let d = validate_dir(&t.0);
    assert_eq!(label_diags(&d).len(), 2, "{d:?}");
    assert_eq!(d.len(), 2, "{d:?}");
}

#[test]
fn pack37_format_a() {
    let t = Tmp::new("answer-label-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n\n[rejections.rj-a]\ncategory = \"not_in_this_step\"\nmessage = \"m\"\nnext_action = \"n\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\nbody = \"a.md\"\nnext = [{ to = \"b\" }]\n",
        "input = { kind = \"choice\", options = [{ id = \"o1\", label = \"X\" }, { id = \"o2\", label = \"Y\" }] }\n",
        "confirm = { kind = \"answer\", on_wrong = \"rj-a\", options = [{ id = \"o1\", label = \"X\", correct = true }, { id = \"o2\", correct = false }] }\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    let d = validate_dir(&t.0);
    assert_reported_once(&d, "chapters/c1/chapter.toml", "step:a");
}

#[test]
fn pack37_module_step_is_reported_at_the_module_file() {
    let t = Tmp::new("answer-label-module");
    t.put("workshop.yaml", WORKSHOP_YAML.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u}]\nsteps:\n  - {id: u, title: U, final: true, use: {module: m, instance: i, args: {}}}\n",
    );
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nsteps:\n  - {id: a, title: T, final: true, input: {kind: choice, options: [{id: ok, label: A}]}, confirm: {kind: answer, on_wrong: rj-a, options: [{id: ok, correct: true}]}}\n",
    );
    let d = validate_dir(&t.0);
    assert_reported_once(&d, "modules/m/module.yaml", "step:a");
}

#[test]
fn pack37_choice_label_is_not_confused() {
    // 入力方式 `choice` の `label` だけを持ち、確認が `answer` でない教材では出ない。
    let t = pack("{id: o1, label: X, correct: true}");
    let ch = "chapters/c1/chapter.yaml";
    let text = std::fs::read_to_string(t.0.join(ch)).expect("read");
    let text = text.replace(
        "confirm: {kind: answer, on_wrong: rj-a, options: [{id: o1, label: X, correct: true}]}",
        "confirm: {kind: approve}",
    );
    t.put(ch, text.as_bytes());
    let d = validate_dir(&t.0);
    assert!(label_diags(&d).is_empty(), "{d:?}");
}

#[test]
fn pack37_compile_does_not_check() {
    let t = pack("{id: o1, label: X, correct: true}, {id: o2, correct: false}");
    assert!(compile_dir(&t.0).is_ok());
    assert_eq!(label_diags(&validate_dir(&t.0)).len(), 1);
}

#[test]
fn pack37_message_has_no_values() {
    let t = pack(&format!(
        "{{id: {SECRET}, correct: true}}, {{id: o2, label: {SECRET}, correct: false}}"
    ));
    let d = validate_dir(&t.0);
    assert_eq!(label_diags(&d).len(), 1, "{d:?}");
    assert_no_secret(&d);
}

/// 引数置換で `kind` が `answer` になる確認も、展開後に検査する（呼び出しが 2 回でも 1 件）。
fn placeholder_kind_pack(kind_arg: &str) -> Tmp {
    let t = Tmp::new("answer-label-subst");
    t.put("workshop.yaml", WORKSHOP_YAML.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: u1}}]\nsteps:\n  - {{id: u1, title: U, use: {{module: m, instance: i1, args: {{k: {kind_arg}}}}}, next: [{{to: u2}}]}}\n  - {{id: u2, title: U, use: {{module: m, instance: i2, args: {{k: {kind_arg}}}}}, next: [{{to: z}}]}}\n  - {{id: z, title: Z, final: true}}\n").as_bytes(),
    );
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nparams: {k: d}\nsteps:\n  - {id: a, title: T, final: true, input: {kind: choice, options: [{id: ok, label: A}]}, confirm: {kind: '${k}', on_wrong: rj-a, options: [{id: ok, correct: true}]}}\n",
    );
    t
}

#[test]
fn pack37_substituted_answer_kind_is_reported_once() {
    let t = placeholder_kind_pack("answer");
    let d = validate_dir(&t.0);
    assert_eq!(label_diags(&d).len(), 1, "{d:?}");
    assert_eq!(exit_code(&d), 1);
    assert!(compile_dir(&t.0).is_ok());
}
