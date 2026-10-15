//! Step の確認 4 種の宣言と、確認の未指定の検査の結合試験（PACK-13、R-22、TASK-47）。
//!
//! 確認 4 種を宣言した教材パックが検証（`compile_dir`）を通ること、入力を持つ Step が確認も理由つきの省略も
//! 持たない場合に `E-NO-CONFIRM` になること、型で読めない宣言・意味の制約に反する宣言が診断になること、
//! 診断に教材データの値が出ないことを確かめる。契約の型そのものは contract の適合試験が持つ。
//!
//! 「確認なしで達成になりうる Step が 0 件」は、検証を通った教材の宣言を静的に数えた保証である。実行時に
//! 確認の前に達成しないこと（EVT-3）は core の判定のタスクが試験する。一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::path::{Path, PathBuf};

use common::{assert_no_secret, ch1, testdata, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::confirm_spec::{
    achievement, answer_spec_of, Achievement, AnswerSpec, ConfirmKind, NoneSpec,
};
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn pack_dir(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

/// 先頭の Step `a` だけが `step_fields` を持つ最小パックの診断を返す（通れば空）。
fn diags_for(step_fields: &str) -> Vec<Diagnostic> {
    let t = Tmp::new("confirm");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, {step_fields} next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    compile_dir(&t.0).err().unwrap_or_default()
}

fn diags_for_confirm(confirm: &str) -> Vec<Diagnostic> {
    diags_for(&format!("input: {{kind: approval}}, confirm: {confirm},"))
}

fn only(d: &[Diagnostic], code: DiagCode) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_eq!(d[0].file(), "chapters/c1/chapter.yaml");
    assert_eq!(d[0].item(), "step:a");
}

#[test]
fn pack13_four_kinds_validate() {
    let bytes = compile_dir(&pack_dir("confirm-kinds-pack")).expect("four kinds validate");
    let ir = load_ir(&bytes).expect("load_ir");
    let kinds: Vec<&str> = ir
        .nodes
        .iter()
        .filter_map(|n| n.step.confirm.as_ref()?.get("kind")?.as_str())
        .collect();
    assert_eq!(kinds, ["point_at", "answer", "approve", "none"]);
}

#[test]
fn pack13_confirm_is_carried_into_the_ir_unchanged() {
    let bytes = compile_dir(&pack_dir("confirm-kinds-pack")).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let confirm = |id: &str| {
        ir.nodes
            .iter()
            .find(|n| n.step.id == id)
            .and_then(|n| n.step.confirm.clone())
            .expect(id)
    };
    assert_eq!(
        confirm("s-none"),
        serde_json::json!({"kind": "none", "reason": "Nothing to verify after this step."})
    );
    assert_eq!(confirm("s-approve"), serde_json::json!({"kind": "approve"}));
    assert_eq!(
        confirm("s-point")["targets"],
        serde_json::json!([{"view": "v-files", "path": "src/main.rs"}])
    );
}

#[test]
fn pack13_four_kinds_validate_in_format_a() {
    let t = Tmp::new("confirm-a");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\ninput = { kind = \"approval\" }\nconfirm = { kind = \"point_at\", targets = [{ view = \"v\", path = \"p\" }] }\nnext = [{ to = \"b\" }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\ninput = { kind = \"approval\" }\nconfirm = { kind = \"answer\", options = [{ id = \"x\", correct = true }], on_wrong = \"w\" }\nnext = [{ to = \"c\" }]\n",
        "[[steps]]\nid = \"c\"\ntitle = \"C\"\ninput = { kind = \"approval\" }\nconfirm = { kind = \"approve\" }\nnext = [{ to = \"d\" }]\n",
        "[[steps]]\nid = \"d\"\ntitle = \"D\"\ninput = { kind = \"approval\" }\nconfirm = { kind = \"none\", reason = \"r\" }\nnext = [{ to = \"e\" }]\n",
        "[[steps]]\nid = \"e\"\ntitle = \"E\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    compile_dir(&t.0).expect("format A validates");
}

#[test]
fn pack13_missing_confirm_is_no_confirm() {
    let inputs = [
        "{kind: command, name: x}",
        "{kind: text, min: 1, max: 5}",
        "{kind: choice, options: [{id: a, label: A}]}",
        "{kind: ui_action, targets: [{view: v, path: p}]}",
        "{kind: approval}",
    ];
    for input in inputs {
        only(&diags_for(&format!("input: {input},")), DiagCode::NoConfirm);
    }
    only(
        &diags_for("input: {kind: wait}, processing: [{name: p1}],"),
        DiagCode::NoConfirm,
    );
}

#[test]
fn pack13_none_without_reason_is_no_confirm() {
    for c in [
        "{kind: none}",
        "{kind: none, reason: ''}",
        "{kind: none, reason: '   '}",
    ] {
        only(&diags_for_confirm(c), DiagCode::NoConfirm);
    }
    assert!(diags_for_confirm("{kind: none, reason: why}").is_empty());
}

#[test]
fn pack13_step_without_input_needs_no_confirm() {
    assert!(diags_for("").is_empty());
}

#[test]
fn pack13_unreadable_confirm_is_parse_error() {
    for c in [
        "{kind: skip}",
        "{reason: r}",
        "{kind: 1}",
        "[approve]",
        "approve",
        "{kind: approve, script: x}",
        "{kind: point_at, targets: [{view: v, path: p}], script: x}",
        "{kind: none, reason: r, extra: 1}",
        "{kind: answer, options: [{id: a, correct: true}]}",
        "{kind: answer, options: [{id: a, correct: 'yes'}], on_wrong: w}",
        "{kind: point_at}",
    ] {
        only(&diags_for_confirm(c), DiagCode::Parse);
    }
}

#[test]
fn pack13_semantic_violations_are_structure_errors() {
    for c in [
        "{kind: point_at, targets: []}",
        "{kind: point_at, targets: [{view: '', path: p}]}",
        "{kind: answer, options: [], on_wrong: w}",
        "{kind: answer, options: [{id: a, correct: true}, {id: a, correct: false}], on_wrong: w}",
        "{kind: answer, options: [{id: a, correct: false}], on_wrong: w}",
        "{kind: answer, options: [{id: a, correct: true}], on_wrong: ''}",
    ] {
        only(&diags_for_confirm(c), DiagCode::Structure);
    }
}

#[test]
fn pack13_diagnostics_do_not_echo_values() {
    let cases = [
        format!("{{kind: none, reason: '{SECRET}', extra: 1}}"),
        format!("{{kind: point_at, targets: [{{view: {SECRET}, path: ''}}]}}"),
        format!(
            "{{kind: answer, options: [{{id: {SECRET}, correct: false}}], on_wrong: {SECRET}}}"
        ),
        format!("{{kind: answer, options: [{{id: {SECRET}, correct: {SECRET}}}], on_wrong: w}}"),
        format!("{{kind: \"{SECRET} X\"}}"),
        format!("{{kind: approve, \"{SECRET} X\": 1}}"),
    ];
    for c in cases {
        let d = diags_for_confirm(&c);
        assert!(!d.is_empty(), "{c}");
        for x in &d {
            assert!(!x.to_string().contains(SECRET), "{x}");
        }
        assert_no_secret(&d);
    }
}

/// 検証を通る教材の、入力を持つ全 Step の確認を数える。確認なしで達成になりうる Step は 0 件でなければならない。
fn count(dir: &Path) -> (usize, usize, usize) {
    let bytes = compile_dir(dir).unwrap_or_else(|d| panic!("{}: {d:?}", dir.display()));
    let ir = load_ir(&bytes).expect("load_ir");
    let (mut confirmed, mut omitted, mut unconfirmed) = (0, 0, 0);
    for n in ir.nodes.iter().filter(|n| n.step.input.is_some()) {
        let kind = n
            .step
            .confirm
            .as_ref()
            .and_then(|c| c.get("kind")?.as_str())
            .and_then(ConfirmKind::from_str_opt);
        match achievement(kind) {
            Achievement::AfterConfirm(_) if kind.is_some() => confirmed += 1,
            Achievement::AfterConfirm(_) => unconfirmed += 1,
            Achievement::OnAccept => {
                let reason = n.step.confirm.as_ref().expect("none has confirm");
                let spec: NoneSpec = serde_json::from_value(
                    serde_json::json!({"reason": reason.get("reason").cloned().unwrap_or_default()}),
                )
                .expect("reason");
                if spec.has_reason() {
                    omitted += 1;
                } else {
                    unconfirmed += 1;
                }
            }
        }
    }
    (confirmed, omitted, unconfirmed)
}

#[test]
fn pack13_no_step_achieves_without_confirmation() {
    let packs = [
        (pack_dir("confirm-kinds-pack"), (3, 1)),
        (pack_dir("input-kinds-pack"), (6, 0)),
        (pack_dir("min-3step-pack-b"), COUNT_MIN3),
        (
            testdata("engine-event-consistency/test-data/min-3step-pack"),
            COUNT_MIN3,
        ),
        (ch1(), COUNT_CH1),
        (testdata("input-field-ime/test-data/ime-pack"), COUNT_IME),
    ];
    for (dir, (confirmed, omitted)) in packs {
        let got = count(&dir);
        assert_eq!(got, (confirmed, omitted, 0), "{}", dir.display());
        assert!(got.0 + got.1 >= 1, "{}", dir.display());
    }
}

const COUNT_MIN3: (usize, usize) = (2, 1);
const COUNT_CH1: (usize, usize) = (4, 2);
const COUNT_IME: (usize, usize) = (0, 1);

/// `confirm-label-pack` の `answer` の宣言（コンパイル後の IR から読む）。
fn label_pack_spec() -> (serde_json::Value, AnswerSpec) {
    let bytes = compile_dir(&pack_dir("confirm-label-pack")).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let confirm = ir
        .nodes
        .iter()
        .find(|n| n.step.id == "s-answer")
        .and_then(|n| n.step.confirm.clone())
        .expect("confirm");
    let spec = answer_spec_of(&confirm).expect("answer");
    (confirm, spec)
}

#[test]
fn pack23_label_validates_and_is_carried_into_the_ir() {
    let (confirm, _) = label_pack_spec();
    assert_eq!(confirm["options"][0]["label"], "正しい手順");
    assert!(confirm["options"][1].get("label").is_none());
}

#[test]
fn pack23_display_names_from_compiled_ir() {
    let (_, spec) = label_pack_spec();
    let names: Vec<&str> = spec.options.iter().map(|o| o.display_name()).collect();
    assert_eq!(names, ["正しい手順", "asked-back"]);
    assert_eq!(
        spec.selection_log_text("ok").as_deref(),
        Some("選択: 正しい手順")
    );
    assert_eq!(
        spec.selection_log_text("asked-back").as_deref(),
        Some("選択: asked-back")
    );
    // 入力方式 choice の label は別の欄であり、確認の表示名には出ない。
    for o in &spec.options {
        assert!(!o.display_name().contains("INPUT-CHOICE"));
    }
}

#[test]
fn pack23_history_records_the_id() {
    let (_, spec) = label_pack_spec();
    let value = spec.recorded_value("ok").expect("known id");
    let op = Op::Input(Input::Choice(value.to_string()));
    let json = serde_json::to_string(&op).expect("serialize");
    assert!(json.contains("\"ok\""), "{json}");
    assert!(!json.contains("正しい手順"), "{json}");
}

#[test]
fn pack23_blank_label_is_structure_error() {
    for label in ["''", "'   '"] {
        let d = diags_for_confirm(&format!(
            "{{kind: answer, options: [{{id: {SECRET}, correct: true, label: {label}}}], on_wrong: w}}"
        ));
        only(&d, DiagCode::Structure);
        assert_no_secret(&d);
    }
    let d = diags_for_confirm(
        "{kind: answer, options: [{id: a, correct: true, label: LBL-VALUE}], on_wrong: w}",
    );
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn pack23_label_validates_in_format_a() {
    let t = Tmp::new("confirm-label-a");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\ninput = { kind = \"approval\" }\nconfirm = { kind = \"answer\", options = [{ id = \"x\", correct = true, label = \"X\" }], on_wrong = \"w\" }\nnext = [{ to = \"b\" }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    compile_dir(&t.0).expect("format A validates");
}
