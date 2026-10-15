//! 形式 B（Markdown と YAML）の読み込みと構造の拒否の試験。
//!
//! 04-behavior の PACK-2（`workshop.yaml` と `workshop.toml` が両方ある・どちらも無いは `E-PARSE`）と
//! PACK-3（構造定義に語彙にない欄を足すと `unknown field`（`E-PARSE`）で拒否する）の前提・操作・期待を
//! 確かめる。試験名の `pack2_`・`pack3_` で 04 の ID を追える。
//!
//! 範囲外: `commands`・`views`・`hints` など `serde_json::Value` で通す欄の中の未知の欄は、語彙の型付け
//! （TASK-42〜TASK-47）が入るまで拒否されない。ここでは「拒否される」とも「通る」とも固定しない。
//! 文字列への型変換・重複キー・行と桁の厳格化（PACK-4）は TASK-40 の 2/2 が扱う。

mod common;

use std::fs;

use common::{assert_no_secret, minimal, testdata, Tmp, CHAPTER, SECRET, WORKSHOP};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::source::{load, Format};
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn diags(t: &Tmp) -> Vec<Diagnostic> {
    match compile_dir(&t.0) {
        Ok(_) => panic!("expected diagnostics"),
        Err(d) => d,
    }
}

/// ちょうど 1 件の `E-PARSE` であることを確かめ、その診断を返す。
fn one_parse(t: &Tmp) -> Diagnostic {
    let mut d = diags(t);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::Parse, "{d:?}");
    assert_no_secret(&d);
    d.remove(0)
}

#[test]
fn pack2_premise_format_b_pack_loads_as_yaml() {
    let t = minimal();
    let p = load(&t.0).expect("load");
    assert_eq!(p.format, Format::Yaml);
    assert_eq!(p.workshop_file, "workshop.yaml");
}

#[test]
fn pack2_both_definition_files_are_rejected_with_e_parse() {
    let t = minimal();
    t.put("workshop.toml", b"[pack]\n");
    one_parse(&t);
}

#[test]
fn pack2_neither_definition_file_is_rejected_with_e_parse() {
    let t = Tmp::new("f2-none-with-chapter");
    t.put("chapters/c1/chapter.yaml", CHAPTER.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    assert_eq!(one_parse(&t).file(), "workshop.yaml");

    let t = Tmp::new("f2-empty");
    assert_eq!(one_parse(&t).file(), "workshop.yaml");
}

#[test]
fn pack2_definition_name_that_is_not_a_file_is_rejected() {
    let t = Tmp::new("f2-dir");
    fs::create_dir_all(t.0.join("workshop.yaml")).expect("mkdir");
    one_parse(&t);

    let t = Tmp::new("f2-dir-and-toml");
    fs::create_dir_all(t.0.join("workshop.yaml")).expect("mkdir");
    t.put("workshop.toml", b"[pack]\n");
    one_parse(&t);
}

#[test]
fn pack3_premise_testdata_structure_has_no_script_field() {
    fn has_script(v: &yaml_serde::Value) -> bool {
        match v {
            yaml_serde::Value::Mapping(m) => m
                .iter()
                .any(|(k, v)| k.as_str() == Some("script") || has_script(v)),
            yaml_serde::Value::Sequence(s) => s.iter().any(has_script),
            _ => false,
        }
    }
    let root = testdata("input-field-ime/test-data/ime-pack");
    let mut files = vec![
        root.join("workshop.yaml"),
        root.join("tests/playthrough.yaml"),
    ];
    for e in fs::read_dir(root.join("chapters"))
        .expect("chapters")
        .flatten()
    {
        files.push(e.path().join("chapter.yaml"));
    }
    for f in files {
        let text = fs::read_to_string(&f).expect("read");
        let v: yaml_serde::Value = yaml_serde::from_str(&text).expect("yaml");
        assert!(!has_script(&v), "script key in {}", f.display());
    }
    load(&root).expect("ime-pack loads");
}

/// 最小パックに「語彙にない欄」を足した形。`(場所の名前, 書き換える相対パス, 内容)`。
fn unknown_field_cases() -> Vec<(&'static str, &'static str, String)> {
    let ch = |steps: &str, extra: &str| format!("id: c1\ntitle: C\n{extra}steps:\n{steps}");
    vec![
        (
            "workshop-top",
            "workshop.yaml",
            format!("{WORKSHOP}script: run()\n"),
        ),
        (
            "pack",
            "workshop.yaml",
            "pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A, script: run()}\n"
                .to_string(),
        ),
        (
            "fixture",
            "workshop.yaml",
            format!(
                "{WORKSHOP}fixtures:\n  f1: {{path: fixtures/x.txt, kind: text, script: run()}}\n"
            ),
        ),
        (
            "chapter-top",
            "chapters/c1/chapter.yaml",
            format!("{CHAPTER}script: run()\n"),
        ),
        (
            "step",
            "chapters/c1/chapter.yaml",
            ch("  - {id: a, title: A, script: run()}\n", ""),
        ),
        (
            "transition",
            "chapters/c1/chapter.yaml",
            ch(
                "  - {id: a, title: A, next: [{to: b, script: run()}]}\n  - {id: b, title: B, final: true}\n",
                "",
            ),
        ),
        (
            "start",
            "chapters/c1/chapter.yaml",
            ch(
                "  - {id: a, title: A, final: true}\n",
                "starts: [{id: s1, node: a, script: run()}]\n",
            ),
        ),
        (
            "module-top",
            "modules/m1/module.yaml",
            ch("  - {id: a, title: A, final: true}\n", "script: run()\n"),
        ),
        (
            "tests-top",
            "tests/playthrough.yaml",
            "playthroughs: []\nscript: run()\n".to_string(),
        ),
    ]
}

#[test]
fn pack3_unknown_field_is_rejected_at_each_structure_level() {
    for (name, path, content) in unknown_field_cases() {
        let t = minimal();
        t.put(path, content.as_bytes());
        let s = one_parse(&t).to_string();
        assert!(s.contains("unknown field `script`"), "{name}: {s}");
        assert!(s.contains("line"), "{name}: {s}");
    }
}

#[test]
fn pack3_unknown_field_other_names_are_rejected() {
    for field in ["exec", "eval", "code", "expr"] {
        let t = minimal();
        t.put(
            "chapters/c1/chapter.yaml",
            format!("id: c1\ntitle: C\nsteps:\n  - {{id: a, title: A, {field}: x}}\n").as_bytes(),
        );
        let s = one_parse(&t).to_string();
        assert!(s.contains(&format!("unknown field `{field}`")), "{s}");
    }
}

#[test]
fn pack3_unknown_field_in_format_a_is_rejected() {
    let t = Tmp::new("f3-toml");
    t.put(
        "workshop.toml",
        b"[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\nscript = \"run()\"\n",
    );
    let s = one_parse(&t).to_string();
    assert!(s.contains("unknown field `script`"), "{s}");
}

#[test]
fn pack3_unknown_field_name_is_not_echoed_when_unsafe() {
    let long = "a".repeat(65);
    for field in [long.as_str(), "a b", "x`y"] {
        let t = minimal();
        t.put(
            "chapters/c1/chapter.yaml",
            format!("id: c1\ntitle: C\nsteps:\n  - {{id: a, title: A, \"{field}\": {SECRET}}}\n")
                .as_bytes(),
        );
        let s = one_parse(&t).to_string();
        assert!(s.contains("unknown field at line"), "{s}");
        assert!(!s.contains(field) && !s.contains(SECRET), "{s}");
    }
}
