//! 数値欄の 10 進でない整数（`0x1f`・`0o17`・`0b11`）の拒否の試験（PACK-5、D-43、TASK-57 1/2）。
//!
//! `yaml_serde`・`toml` は `0x1f` を黙って 31 に読む。構造定義のどの位置でも `E-PARSE` で拒否し、31 に読み替えた
//! IR を出さないことを、形式 B と形式 A の両方で確かめる。診断には入力の値（`0x1f`）を写さず、行と桁だけを添える。
//! `fandhe-pack validate` の結線は TASK-401 が受け持つため、ここではライブラリの経路（`compile_dir`）を使う。
//! アンカー・タグ・複数文書の拒否は `yaml_features.rs`、`starts` 0 件の拒否は `no_starts.rs`（TASK-57 2/2）が扱う。

mod common;

use common::{Tmp, CHAPTER, WORKSHOP};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn compile_yaml(workshop: &str, chapter: &str) -> Result<(), Vec<Diagnostic>> {
    let t = Tmp::new("radix");
    t.put("workshop.yaml", workshop.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    compile_dir(&t.0).map(|_| ())
}

fn chapter_with_input(input: &str) -> String {
    format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, body: a.md, input: {input}, confirm: {{kind: approve}}, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n"
    )
}

fn workshop_with_commands(value: &str) -> String {
    format!("{WORKSHOP}commands:\n  x: {value}\n")
}

/// 診断がすべて `E-PARSE` で、10 進の要求と行・桁を述べ、入力の値を含まないことを確かめる。
fn assert_radix_rejected(r: Result<(), Vec<Diagnostic>>, what: &str) {
    let d = r.expect_err(what);
    assert!(!d.is_empty(), "{what}");
    for x in &d {
        assert_eq!(x.code(), DiagCode::Parse, "{what}: {d:?}");
    }
    let s = d[0].to_string();
    assert!(s.contains("must be written in decimal"), "{what}: {s}");
    assert!(s.contains("line ") && s.contains("column "), "{what}: {s}");
    for lit in ["0x", "0o", "0b", "1f"] {
        assert!(!s.contains(lit), "{what}: value echoed: {s}");
    }
}

#[test]
fn pack5_premise_decimal_value_compiles_and_is_read_as_given() {
    let ch = chapter_with_input("{kind: text, min: 1, max: 31}");
    assert!(compile_yaml(WORKSHOP, &ch).is_ok());
}

#[test]
fn pack5_expect_input_max_in_hex_is_rejected() {
    let ch = chapter_with_input("{kind: text, min: 1, max: 0x1f}");
    assert_radix_rejected(compile_yaml(WORKSHOP, &ch), "max: 0x1f");
}

#[test]
fn pack5_expect_every_radix_and_sign_is_rejected() {
    for v in ["0x1f", "-0x1f", "+0x1f", "0o17", "0b11"] {
        let ch = chapter_with_input(&format!("{{kind: text, min: 1, max: {v}}}"));
        assert_radix_rejected(compile_yaml(WORKSHOP, &ch), v);
    }
}

#[test]
fn pack5_expect_value_regions_and_nesting_are_rejected() {
    // `commands` は `serde_json::Value` の領域で、型付きの読み込みでは字面が失われる位置。
    for v in ["{n: 0x1f}", "[1, {n: 0x1f}]", "{a: {b: [0b11]}}", "0x1f"] {
        assert_radix_rejected(compile_yaml(&workshop_with_commands(v), CHAPTER), v);
    }
    let block = format!("{WORKSHOP}commands:\n  x:\n    - n: 0o17\n");
    assert_radix_rejected(compile_yaml(&block, CHAPTER), "block sequence");
}

#[test]
fn pack5_expect_position_points_at_the_scalar_line() {
    let w = format!("{WORKSHOP}commands:\n  x: 1\n  y: {{n: 0x1f}}\n");
    let d = compile_yaml(&w, CHAPTER).expect_err("rejected");
    assert!(d[0].to_string().contains("line 4 "), "{d:?}");
}

#[test]
fn pack5_expect_toml_format_is_rejected_too() {
    const HEAD: &str =
        "[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n\n";
    for v in ["0x1f", "0o17", "0b11"] {
        let t = Tmp::new("radix-toml");
        t.put(
            "workshop.toml",
            format!("{HEAD}[commands.x]\nn = {v}\n").as_bytes(),
        );
        assert_radix_rejected(compile_dir(&t.0).map(|_| ()), v);
    }
    let t = Tmp::new("radix-toml-array");
    t.put(
        "workshop.toml",
        format!("{HEAD}[commands]\nx = [1, {{ n = 0x1f }}]\n").as_bytes(),
    );
    assert_radix_rejected(compile_dir(&t.0).map(|_| ()), "toml array");
}

#[test]
fn pack5_not_rejected_text_and_non_integer_forms() {
    // 引用符で囲めば文字列として書ける。大文字の `0X1F` は `yaml_serde` が文字列として読み、数には変換されない。
    for v in ["'0x1f'", "\"0x1f\"", "0X1F", "31", "1e3"] {
        assert!(
            compile_yaml(&workshop_with_commands(v), CHAPTER).is_ok(),
            "{v}"
        );
    }
    let ch = CHAPTER.replace("title: A", "title: '0x1f'");
    assert!(compile_yaml(WORKSHOP, &ch).is_ok());
}
