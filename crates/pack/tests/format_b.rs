//! 形式 B（Markdown と YAML）の読み込みと構造の拒否の試験。
//!
//! 04-behavior の PACK-2（`workshop.yaml` と `workshop.toml` が両方ある・どちらも無いは `E-PARSE`）と
//! PACK-3（構造定義に語彙にない欄を足すと `unknown field`（`E-PARSE`）で拒否する）の前提・操作・期待を
//! 確かめる。PACK-4（値の読み方と構文エラー）は末尾の `pack4_` の試験が確かめる。試験名の `pack2_`・`pack3_`・
//! `pack4_` で 04 の ID を追える。
//!
//! 範囲外: `commands`・`views`・`hints` など `serde_json::Value` で通す欄の中の未知の欄は、語彙の型付け
//! （TASK-42〜TASK-47）が入るまで拒否されない。ここでは「拒否される」とも「通る」とも固定しない。
//! 同じ理由で、`Value` の欄の中の引用符なしの値は文字列にならない（重複キーの拒否だけが及ぶ）。

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

// ---- PACK-4: 値の読み方と構文エラー ----

/// 引用符なしだと YAML が別の型にしうる値。
const CONVERTIBLE: [&str; 5] = ["true", "no", "1.10", "010", "null"];

/// 文字列の欄すべてに、引用符なしの変換されうる値を書いた形式 B のパックを作る。
fn convertible_pack() -> Tmp {
    let t = Tmp::new("f4-convertible");
    t.put(
        "workshop.yaml",
        b"pack: {id: true, title: no, version: 1.10, ir_format: 1, guidance: A}\n",
    );
    t.put(
        "chapters/c1/chapter.yaml",
        concat!(
            "id: 010\ntitle: null\nparams: {k: true, n: 1.10}\n",
            "starts: [{id: no, node: true}]\nsteps:\n",
            "  - id: true\n    title: 1.10\n    do: null\n    lo: [010, null]\n    hints: [no]\n",
            "    body_levels: {A: 010}\n    next: [{to: no}]\n",
            "  - {id: no, title: 010, final: true}\n",
        )
        .as_bytes(),
    );
    t
}

#[test]
fn pack4_premise_unquoted_convertible_values_are_written_in_string_fields() {
    let t = convertible_pack();
    let ws = fs::read_to_string(t.0.join("workshop.yaml")).expect("read");
    let ch = fs::read_to_string(t.0.join("chapters/c1/chapter.yaml")).expect("read");
    let text = format!("{ws}{ch}");
    for v in CONVERTIBLE {
        assert!(text.contains(v), "{v}");
    }
    assert!(!text.contains(['\'', '"']));
}

#[test]
fn pack4_expect_unquoted_convertible_values_are_read_as_strings() {
    let t = convertible_pack();
    t.put("chapters/c1/010", b"# x\n");
    let p = load(&t.0).expect("load");
    assert_eq!(p.workshop.pack.id, "true");
    assert_eq!(p.workshop.pack.title, "no");
    // `1.10` が `1.1` に、`010` が `10`・`8` にならない。
    assert_eq!(p.workshop.pack.version, "1.10");
    let s = &p.scopes[0].src;
    assert_eq!(s.id, "010");
    assert_eq!(s.title, "null");
    assert_eq!(s.params["k"], "true");
    assert_eq!(s.params["n"], "1.10");
    assert_eq!(s.starts[0].id, "no");
    assert_eq!(s.starts[0].node, "true");
    let st = &s.steps[0];
    assert_eq!(st.id, "true");
    assert_eq!(st.title, "1.10");
    assert_eq!(st.do_.as_deref(), Some("null"));
    assert_eq!(st.lo, ["010", "null"]);
    assert_eq!(st.hints, ["no"]);
    assert_eq!(st.body_levels["A"], "010");
    assert_eq!(st.next[0].to, "no");
    assert_eq!(s.steps[1].title, "010");
}

#[test]
fn pack4_expect_bare_null_in_optional_string_fields_is_text() {
    let cases = [
        ("do: null", Some("null")),
        ("do: ~", Some("~")),
        ("do: 'null'", Some("null")),
        ("do:", None),
        ("lo: []", None),
    ];
    for (field, want) in cases {
        let t = minimal();
        t.put(
            "chapters/c1/chapter.yaml",
            format!(
                "id: c1\ntitle: C\nsteps:\n  - id: a\n    title: A\n    {field}\n    final: true\n"
            )
            .as_bytes(),
        );
        let p = load(&t.0).expect("load");
        assert_eq!(p.scopes[0].src.steps[0].do_.as_deref(), want, "{field}");
    }
    // `body: null` は未設定ではなく、`null` という名前の本文の参照になる。
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nsteps:\n  - {id: a, title: A, body: null, final: true}\n",
    );
    let d = diags(&t);
    assert_no_secret(&d);
    assert!(d.iter().any(|x| x.to_string().contains("null")), "{d:?}");
}

/// 重複キーの場所ごとの試験。`(名前, 相対パス, 内容, 期待する行, 期待する桁)`。
fn duplicate_cases() -> Vec<(&'static str, &'static str, String, usize, usize)> {
    let ch = |body: &str| format!("id: c1\ntitle: C\n{body}");
    vec![
        (
            "workshop-top",
            "workshop.yaml",
            format!("{WORKSHOP}pack: {{}}\n"),
            2,
            1,
        ),
        (
            "pack",
            "workshop.yaml",
            "pack:\n  id: p\n  title: t\n  id: q\n".to_string(),
            4,
            3,
        ),
        (
            "commands-table",
            "workshop.yaml",
            format!("{WORKSHOP}commands:\n  ls: {{}}\n  ls: {{}}\n"),
            4,
            3,
        ),
        (
            "value-inside-commands",
            "workshop.yaml",
            format!("{WORKSHOP}commands:\n  ls:\n    a: 1\n    a: 2\n"),
            5,
            5,
        ),
        (
            "chapter-top",
            "chapters/c1/chapter.yaml",
            ch("title: D\nsteps: []\n"),
            3,
            1,
        ),
        (
            "step",
            "chapters/c1/chapter.yaml",
            ch("steps:\n  - id: a\n    title: A\n    id: b\n"),
            6,
            5,
        ),
        (
            "params",
            "chapters/c1/chapter.yaml",
            ch("params:\n  k: 1\n  k: 2\nsteps: []\n"),
            5,
            3,
        ),
        (
            "on-reject",
            "chapters/c1/chapter.yaml",
            ch("steps:\n  - id: a\n    title: A\n    on_reject:\n      r: x\n      r: y\n"),
            8,
            7,
        ),
        (
            "playthrough",
            "tests/playthrough.yaml",
            "playthroughs: []\nplaythroughs: []\n".to_string(),
            2,
            1,
        ),
    ]
}

#[test]
fn pack4_expect_duplicate_keys_are_rejected_with_e_parse() {
    for (name, path, content, line, col) in duplicate_cases() {
        let t = minimal();
        t.put(path, content.as_bytes());
        let s = one_parse(&t).to_string();
        assert!(s.contains("duplicate key"), "{name}: {s}");
        assert!(
            s.contains(&format!("at line {line} column {col}")),
            "{name}: {s}"
        );
    }
}

#[test]
fn pack4_expect_duplicate_key_name_is_not_echoed_unless_identifier() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nparams:\n  \"a {SECRET}\": 1\n  \"a {SECRET}\": 2\nsteps: []\n")
            .as_bytes(),
    );
    let s = one_parse(&t).to_string();
    assert!(s.contains("duplicate key at line 5 column 3"), "{s}");
    assert!(!s.contains("SECRET"), "{s}");

    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nparams:\n  k.v-1: 1\n  k.v-1: 2\nsteps: []\n",
    );
    let s = one_parse(&t).to_string();
    assert!(s.contains("duplicate key `k.v-1` at line 5"), "{s}");
}

#[test]
fn pack4_expect_duplicate_keys_differing_only_in_quoting_are_rejected() {
    let t = minimal();
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nparams:\n  true: 1\n  'true': 2\nsteps: []\n",
    );
    let s = one_parse(&t).to_string();
    assert!(s.contains("duplicate key `true` at line 5"), "{s}");
}

#[test]
fn pack4_expect_syntax_errors_carry_line_and_column() {
    let bad = [
        ("unclosed-flow", "id: c1\ntitle: [C\nsteps: []\n"),
        ("unclosed-quote", "id: c1\ntitle: \"C\nsteps: []\n"),
        (
            "bad-indent",
            "id: c1\ntitle: C\nsteps:\n  - id: a\n   title: A\n",
        ),
        ("tab", "id: c1\ntitle: C\nsteps:\n\t- id: a\n"),
    ];
    for (name, content) in bad {
        for path in ["workshop.yaml", "chapters/c1/chapter.yaml"] {
            let t = minimal();
            t.put(path, content.as_bytes());
            let s = one_parse(&t).to_string();
            assert!(
                s.contains("invalid syntax or structure at line "),
                "{name} {path}: {s}"
            );
            assert!(s.contains(" column "), "{name} {path}: {s}");
        }
    }
}

#[test]
fn pack4_expect_multiple_documents_are_rejected_without_position() {
    // 複数文書は yaml_serde 0.10.7 が位置を返さない。既知の制約として `E-PARSE` であることだけを固定する。
    let t = minimal();
    t.put(
        "workshop.yaml",
        format!("{WORKSHOP}---\n{WORKSHOP}").as_bytes(),
    );
    one_parse(&t);
}

#[test]
fn pack4_operation_testdata_packs_still_load() {
    for rel in [
        "input-field-ime/test-data/ime-pack",
        "input-field-ime/test-data/ch1-pack",
    ] {
        let p = load(&testdata(rel)).expect(rel);
        assert_eq!(p.format, Format::Yaml, "{rel}");
    }
}
