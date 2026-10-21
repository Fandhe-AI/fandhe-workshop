//! 自然文の内容で規則を分ける書き方の拒否の結合試験（PACK-41、D-43、TASK-68）。
//!
//! 判定語は語彙に足さない。内容の条件（`contains` など）を規則の `on` に書いても実行時は無視されるため、
//! 書き方そのものを `validate` と `compile` が同じ `E-PARSE` で拒否することを確かめる。
//! 診断には値（`SECRET` の目印）が出ない。`on.text` の真偽値と既知のキーだけの規則は通す。

mod common;

use common::{assert_no_secret, Tmp, SECRET, WORKSHOP};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

/// `input` と 1 件の規則 `on` を持つ Step `a` の最小パックを作る。
fn pack(input: &str, on: &str) -> Tmp {
    let t = Tmp::new("pack41");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, input: {input}, confirm: {{kind: approve}}, rules: [{{id: r, on: {on}, outcome: {{kind: accept}}}}], next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t
}

const TEXT: &str = "{kind: text, min: 1, max: 10}";

fn compile_diags(t: &Tmp) -> Vec<Diagnostic> {
    compile_dir(&t.0).err().unwrap_or_default()
}

/// validate と compile が同じ 1 件の `E-PARSE` を返すことを確かめ、その診断を返す。
fn one_parse(t: &Tmp) -> Diagnostic {
    let v = validate_dir(&t.0);
    let c = compile_diags(t);
    for d in [&v, &c] {
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code(), DiagCode::Parse, "{d:?}");
        assert_eq!(d[0].file(), "chapters/c1/chapter.yaml");
        assert_eq!(d[0].item(), "step:a");
        assert!(d[0].message().starts_with("rule (text):"), "{}", d[0]);
        assert_no_secret(d);
    }
    assert_eq!(v[0].message(), c[0].message());
    v[0].clone()
}

#[test]
fn pack41_content_value_in_on_text_is_rejected() {
    for text in [
        format!("{{contains: {SECRET}}}"),
        format!("{{pattern: {SECRET}}}"),
        SECRET.to_string(),
        "3".to_string(),
        "null".to_string(),
    ] {
        let t = pack(TEXT, &format!("{{text: {text}}}"));
        let d = one_parse(&t);
        assert!(d.message().contains("on.text must be a boolean"), "{d}");
    }
}

#[test]
fn pack41_unknown_key_beside_text_is_rejected() {
    for k in ["contains", "pattern", "regex", "keywords", "matches"] {
        let t = pack(TEXT, &format!("{{text: true, {k}: {SECRET}}}"));
        let d = one_parse(&t);
        assert!(d.message().contains(k), "{d}");
    }
}

#[test]
fn pack41_unknown_key_on_text_step_is_rejected() {
    let t = pack(TEXT, &format!("{{contains: {SECRET}}}"));
    let d = one_parse(&t);
    assert!(d.message().contains("contains"), "{d}");
    let t = pack(TEXT, "[a, b]");
    let d = one_parse(&t);
    assert!(d.message().contains("on must be a mapping"), "{d}");
}

#[test]
fn pack41_unsafe_key_name_is_not_echoed() {
    let t = pack(TEXT, &format!("{{text: true, \"a b{SECRET}\": 1}}"));
    let d = one_parse(&t);
    assert_eq!(d.message(), "rule (text): unknown field");
}

#[test]
fn pack41_plain_text_rules_pass() {
    for on in [
        "{text: true}",
        "{text: false}",
        "{cancel: true}",
        "{text: true, cancel: true}",
    ] {
        let t = pack(TEXT, on);
        assert!(validate_dir(&t.0).is_empty(), "{on}");
        assert!(compile_dir(&t.0).is_ok(), "{on}");
    }
    let five = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/input-kinds-pack");
    assert!(validate_dir(&five).is_empty());
}

#[test]
fn pack41_other_steps_are_not_checked() {
    let t = pack("{kind: command, commands: [x]}", "{command: x, foo: 1}");
    for d in validate_dir(&t.0).iter().chain(compile_diags(&t).iter()) {
        assert!(!d.message().starts_with("rule (text):"), "{d}");
    }
}

#[test]
fn pack41_module_text_rule_is_checked() {
    let t = Tmp::new("pack41-mod");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, final: true}\n",
    );
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: M\nsteps:\n  - {id: x, title: X, input: {kind: text, min: 1, max: 5}, rules: [{id: r, on: {text: true, contains: q}, outcome: {kind: accept}}], final: true}\n",
    );
    for d in [validate_dir(&t.0), compile_diags(&t)] {
        let hit: Vec<_> = d
            .iter()
            .filter(|x| x.code() == DiagCode::Parse && x.message().starts_with("rule (text):"))
            .collect();
        assert_eq!(hit.len(), 1, "{d:?}");
        assert_eq!(hit[0].file(), "modules/m1/module.yaml");
    }
}

#[test]
fn pack41_format_a_is_rejected_too() {
    let t = Tmp::new("pack41-a");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n",
    );
    let chapter = concat!(
        "id = \"c1\"\ntitle = \"C\"\nstarts = [{ id = \"s1\", node = \"a\" }]\n",
        "[[steps]]\nid = \"a\"\ntitle = \"A\"\ninput = { kind = \"text\", min = 1, max = 10 }\nconfirm = { kind = \"approve\" }\nnext = [{ to = \"b\" }]\n",
        "rules = [{ id = \"r\", on = { text = true, contains = \"q\" }, outcome = { kind = \"accept\" } }]\n",
        "[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    t.put("chapters/c1/chapter.toml", chapter.as_bytes());
    for d in [validate_dir(&t.0), compile_diags(&t)] {
        assert_eq!(d.len(), 1, "{d:?}");
        assert_eq!(d[0].code(), DiagCode::Parse);
    }
}

#[test]
fn pack41_when_content_condition_is_rejected_by_compile() {
    let t = Tmp::new("pack41-when");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, input: {TEXT}, confirm: {{kind: approve}}, rules: [{{id: r, on: {{text: true}}, when: {{contains: q}}, outcome: {{kind: accept}}}}], next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let d = compile_diags(&t);
    assert!(d.iter().any(|x| x.code() == DiagCode::Parse), "{d:?}");
}

/// モジュールの text Step を章から `use` で使うパックを作る（`input` と引数は呼び出し側で決める）。
fn placeholder_module(input: &str, args: &str) -> Tmp {
    let t = Tmp::new("pack41-ph");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: u}}]\nsteps:\n  - {{id: u, title: U, final: true, use: {{module: m, instance: i, args: {args}}}}}\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    let module = format!(
        "id: m\ntitle: M\nparams: {{kind: d, n: d}}\nsteps:\n  - {{id: x, title: X, input: {input}, confirm: {{kind: approve}}, rules: [{{id: r, on: {{contains: q}}, outcome: {{kind: accept}}}}], final: true}}\n"
    );
    t.put("modules/m/module.yaml", module.as_bytes());
    t
}

fn text_hits(d: &[Diagnostic]) -> Vec<&Diagnostic> {
    d.iter()
        .filter(|x| x.code() == DiagCode::Parse && x.message().starts_with("rule (text):"))
        .collect()
}

#[test]
fn pack41_module_literal_text_with_placeholder_elsewhere_is_checked() {
    let t = placeholder_module(
        "{kind: text, min: 1, max: \"${n}\"}",
        "{kind: text, n: \"10\"}",
    );
    for d in [validate_dir(&t.0), compile_diags(&t)] {
        assert_eq!(text_hits(&d).len(), 1, "{d:?}");
    }
}

#[test]
fn pack41_module_kind_resolved_to_text_is_checked() {
    let t = placeholder_module(
        "{kind: \"${kind}\", min: 1, max: 10}",
        "{kind: text, n: \"10\"}",
    );
    for d in [validate_dir(&t.0), compile_diags(&t)] {
        let hit = text_hits(&d);
        assert_eq!(hit.len(), 1, "{d:?}");
        assert_eq!(hit[0].file(), "modules/m/module.yaml");
        assert_no_secret(&d);
    }
    let t = placeholder_module(
        "{kind: \"${kind}\", commands: [x]}",
        "{kind: command, n: \"10\"}",
    );
    for d in [validate_dir(&t.0), compile_diags(&t)] {
        assert!(text_hits(&d).is_empty(), "{d:?}");
    }
}
