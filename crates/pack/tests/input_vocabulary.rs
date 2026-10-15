//! 入力方式 5 種と待機・取り消しの宣言と `text` の判定の結合試験（PACK-10、TASK-44）。
//!
//! 5 方式と `wait`・`cancel` を宣言した教材パックが検証（`compile_dir`）を通ること、型で読めない宣言・意味の制約に反する宣言が
//! 診断になること、診断に入力の値が出ないことを確かめる。`text` の判定そのものは contract の適合試験が持つ。
//! `fandhe-pack validate` の CLI の振り分けは TASK-401 が受け持つため、ここではライブラリの経路を確かめる。
//! `wait`・`cancel` は #116（TASK-44 の 2/2）が扱う。一時ディレクトリは `CARGO_TARGET_TMPDIR` の下に作る。

mod common;

use std::path::Path;

use common::{assert_no_secret, ch1, copy_dir, testdata, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_contract::input_spec::InputKind;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::vocab::TERMS;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn five_kinds() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/input-kinds-pack")
}

/// 1 Step だけが `input` を持つ最小パックを作り、診断を返す（通れば空）。
fn diags_for_input(input: &str) -> Vec<Diagnostic> {
    let t = Tmp::new("input");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, input: {input}, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    match compile_dir(&t.0) {
        Ok(_) => Vec::new(),
        Err(d) => d,
    }
}

/// 1 Step が任意の追加欄（`processing`・`rules` など）を持つ最小パックの診断を返す。
fn diags_for_step(input: &str, extra: &str) -> Vec<Diagnostic> {
    let t = Tmp::new("step");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, input: {input}, {extra} next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    match compile_dir(&t.0) {
        Ok(_) => Vec::new(),
        Err(d) => d,
    }
}

fn only_code(d: &[Diagnostic]) -> DiagCode {
    assert_eq!(d.len(), 1, "{d:?}");
    d[0].code()
}

#[test]
fn pack10_all_kinds_with_wait_and_cancel_validate() {
    let bytes = compile_dir(&five_kinds()).expect("five kinds validate");
    let ir = load_ir(&bytes).expect("load_ir");
    let kinds: Vec<&str> = ir
        .nodes
        .iter()
        .filter_map(|n| n.step.input.as_ref()?.get("kind")?.as_str())
        .collect();
    assert_eq!(
        kinds,
        ["command", "text", "choice", "ui_action", "approval", "wait"]
    );
    let rule = ir
        .nodes
        .iter()
        .find(|n| n.step.id == "s-text")
        .and_then(|n| n.step.rules.first().cloned())
        .expect("cancel rule");
    assert_eq!(rule["on"], serde_json::json!({"cancel": true}));
}

#[test]
fn pack10_declaration_is_carried_into_the_ir_unchanged() {
    let bytes = compile_dir(&five_kinds()).expect("compile");
    let ir = load_ir(&bytes).expect("load_ir");
    let text = ir
        .nodes
        .iter()
        .find(|n| n.step.id == "s-text")
        .and_then(|n| n.step.input.clone())
        .expect("text input");
    assert_eq!(
        text,
        serde_json::json!({"kind": "text", "min": 1, "max": 100})
    );
}

#[test]
fn pack10_five_kinds_validate_in_format_a() {
    let t = Tmp::new("input-a");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\ninput = { kind = \"text\", min = 1, max = 10 }\nnext = [{ to = \"b\" }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\ninput = { kind = \"approval\" }\nnext = [{ to = \"c\" }]\n",
        "[[steps]]\nid = \"c\"\ntitle = \"C\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    compile_dir(&t.0).expect("format A validates");
}

#[test]
fn pack10_text_has_no_content_judging_field() {
    for field in ["contains", "pattern", "regex"] {
        let d = diags_for_input(&format!("{{kind: text, min: 1, max: 10, {field}: x}}"));
        assert_eq!(only_code(&d), DiagCode::Parse);
        assert!(d[0].to_string().contains(field), "{}", d[0]);
    }
}

#[test]
fn pack10_unreadable_declarations_are_parse_errors() {
    let cases = [
        "{kind: nope}",
        "{min: 1}",
        "{kind: 3}",
        "[text]",
        "{kind: command}",
        "{kind: command, name: x, extra: 1}",
        "{kind: text, max: 10}",
        "{kind: text, min: '1', max: 10}",
        "{kind: text, min: -1, max: 10}",
        "{kind: choice, options: [{id: a}]}",
        "{kind: choice, options: [{id: a, label: A, extra: 1}]}",
        "{kind: ui_action, targets: [{view: v}]}",
        "{kind: command, name: x, args: [{name: a, shape: email}]}",
        "{kind: approval, extra: 1}",
    ];
    for c in cases {
        assert_eq!(only_code(&diags_for_input(c)), DiagCode::Parse, "{c}");
    }
}

#[test]
fn pack10_semantic_violations_are_structure_errors() {
    let cases = [
        "{kind: text, min: 5, max: 4}",
        "{kind: choice, options: []}",
        "{kind: choice, options: [{id: a, label: A}, {id: a, label: B}]}",
        "{kind: choice, options: [{id: a, label: ''}]}",
        "{kind: ui_action, targets: []}",
        "{kind: ui_action, targets: [{view: '', path: p}]}",
        "{kind: command, name: ''}",
        "{kind: command, name: x, args: [{name: a}, {name: a}]}",
    ];
    for c in cases {
        assert_eq!(only_code(&diags_for_input(c)), DiagCode::Structure, "{c}");
    }
}

#[test]
fn pack10_wait_with_processing_validates() {
    assert!(diags_for_step("{kind: wait}", "processing: [{name: p1}],").is_empty());
}

#[test]
fn pack10_wait_requires_processing() {
    assert_eq!(
        only_code(&diags_for_input("{kind: wait}")),
        DiagCode::Structure
    );
    assert_eq!(
        only_code(&diags_for_step("{kind: wait}", "processing: [],")),
        DiagCode::Structure
    );
}

#[test]
fn pack10_wait_has_no_fields() {
    let d = diags_for_step("{kind: wait, extra: 1}", "processing: [{name: p1}],");
    assert_eq!(only_code(&d), DiagCode::Parse);
    assert!(d[0].to_string().contains("extra"), "{}", d[0]);
}

#[test]
fn pack10_processing_without_wait_is_allowed() {
    let d = diags_for_step("{kind: command, name: x}", "processing: [{name: p1}],");
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn pack10_cancel_is_not_an_input_kind() {
    assert_eq!(
        only_code(&diags_for_input("{kind: cancel}")),
        DiagCode::Parse
    );
}

#[test]
fn pack10_cancel_rule_validates_on_any_kind() {
    let rule = "rules: [{id: r, on: {cancel: true}, outcome: {kind: accept}}],";
    for (input, extra) in [
        ("{kind: text, min: 1, max: 5}", String::new()),
        ("{kind: command, name: x}", String::new()),
        ("{kind: wait}", "processing: [{name: p1}],".to_string()),
    ] {
        let d = diags_for_step(input, &format!("{rule} {extra}"));
        assert!(d.is_empty(), "{input}: {d:?}");
    }
}

#[test]
fn pack10_cancel_rule_value_must_be_true() {
    let input = "{kind: text, min: 1, max: 5}";
    for (v, code) in [
        ("'yes'", DiagCode::Parse),
        ("1", DiagCode::Parse),
        ("false", DiagCode::Structure),
    ] {
        let extra = format!("rules: [{{id: r, on: {{cancel: {v}}}, outcome: {{kind: accept}}}}],");
        assert_eq!(only_code(&diags_for_step(input, &extra)), code, "{v}");
    }
}

#[test]
fn pack10_wait_and_cancel_in_format_a() {
    let t = Tmp::new("input-wait-a");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\ninput = { kind = \"wait\" }\nprocessing = [{ name = \"p1\" }]\n",
        "rules = [{ id = \"r\", on = { cancel = true }, outcome = { kind = \"accept\" } }]\nnext = [{ to = \"b\" }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    compile_dir(&t.0).expect("wait and cancel validate in format A");
}

#[test]
fn pack10_invalid_input_in_a_module_is_detected() {
    let t = Tmp::new("input-mod");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, final: true}\n",
    );
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: M\nsteps:\n  - {id: x, title: X, input: {kind: text, min: 3, max: 1}, final: true}\n",
    );
    let d = compile_dir(&t.0).expect_err("module input is checked");
    assert!(d.iter().any(|x| x.code() == DiagCode::Structure), "{d:?}");
}

#[test]
fn pack10_diagnostics_do_not_leak_values() {
    // 値の位置に目印を置く。欄・kind の名前は識別子として安全なときだけ写すので、ここでは値だけを試す。
    for c in [
        format!("{{kind: text, min: {SECRET}, max: 10}}"),
        format!(
            "{{kind: choice, options: [{{id: {SECRET}, label: A}}, {{id: {SECRET}, label: B}}]}}"
        ),
        format!("{{kind: command, name: x, args: [{{name: a, shape: {SECRET}}}]}}"),
        format!("{{kind: ui_action, targets: [{{view: {SECRET}, path: p}}, 1]}}"),
    ] {
        let d = diags_for_input(&c);
        assert!(!d.is_empty(), "{c}");
        assert_no_secret(&d);
    }
    for extra in [
        format!("rules: [{{id: {SECRET}, on: {{cancel: {SECRET}}}}}],"),
        format!("rules: [{{id: {SECRET}, on: {{cancel: false}}}}],"),
    ] {
        let d = diags_for_step("{kind: approval}", &extra);
        assert!(!d.is_empty(), "{extra}");
        assert_no_secret(&d);
    }
    // 識別子として安全でない欄の名前は写さない。
    let d = diags_for_input("{kind: text, min: 1, max: 10, \"SECRET BODY\": 1}");
    assert_eq!(only_code(&d), DiagCode::Parse);
    assert!(!d[0].to_string().contains("SECRET BODY"));
}

#[test]
fn pack10_existing_testdata_packs_still_validate() {
    compile_dir(&ch1()).expect("ch1-pack");
    let ime = testdata("input-field-ime/test-data/ch1-pack");
    let t = Tmp::new("input-ime");
    copy_dir(&ime, &t.0);
    compile_dir(&t.0).expect("ime pack");
}

#[test]
fn pack10_kinds_are_in_vocabulary() {
    for k in InputKind::ALL {
        let hit = TERMS
            .iter()
            .any(|t| t.kind == "input" && t.term == k.as_str());
        assert!(hit, "{} is not in the vocabulary table", k.as_str());
    }
    // cancel は規則の語で、kind には含めない（EP-2）。
    assert!(TERMS
        .iter()
        .any(|t| t.kind == "input" && t.term == "cancel"));
    assert_eq!(InputKind::from_str_opt("cancel"), None);
}
