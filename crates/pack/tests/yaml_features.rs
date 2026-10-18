//! アンカー・エイリアス・タグ・複数文書の拒否の試験（PACK-5、D-43、TASK-57 2/2）。
//!
//! `yaml_serde` はアンカーとエイリアスを透過に展開し、`!!int` などのタグを黙って解決する。構造定義のどの位置でも
//! `E-PARSE` で拒否し、読み替えた IR を出さないことを確かめる。診断にはアンカー名・タグの字面・値を写さず、行と桁
//! だけを添える。形式 A（TOML）にはアンカー・タグ・複数文書が無いので対象外である。
//! `fandhe-pack validate` の結線は TASK-401 が受け持つため、ここではライブラリの経路（`compile_dir`・`validate_dir`）
//! を使う。10 進でない整数の拒否は `radix_literals.rs`、複数文書の位置なしの拒否は `format_b.rs` も固定している。

mod common;

use common::{Tmp, CHAPTER, WORKSHOP};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

/// 作成者の秘密の目印。診断に写らないことを確かめる。
const MARK: &str = "SECRETMARK";

fn put_pack(tag: &str, workshop: &str, chapter: &str) -> Tmp {
    let t = Tmp::new(tag);
    t.put("workshop.yaml", workshop.as_bytes());
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn compile_yaml(workshop: &str, chapter: &str) -> Result<(), Vec<Diagnostic>> {
    compile_dir(&put_pack("feat", workshop, chapter).0).map(|_| ())
}

fn workshop_with_commands(value: &str) -> String {
    format!("{WORKSHOP}commands:\n  x: {value}\n")
}

/// 診断がすべて `E-PARSE` で、`needle` と行・桁を述べ、作成者の字面を含まないことを確かめる。
fn assert_rejected(r: Result<(), Vec<Diagnostic>>, needle: &str, what: &str) {
    let d = r.expect_err(what);
    assert!(!d.is_empty(), "{what}");
    for x in &d {
        assert_eq!(x.code(), DiagCode::Parse, "{what}: {d:?}");
    }
    let s = d[0].to_string();
    assert!(s.contains(needle), "{what}: {s}");
    assert!(s.contains("line ") && s.contains("column "), "{what}: {s}");
    assert!(
        !s.contains(MARK) && !s.contains("anchor_name"),
        "{what}: echoed: {s}"
    );
    assert!(!s.contains("secrettag"), "{what}: echoed: {s}");
}

const ANCHOR: &str = "anchors and aliases are not allowed";
const TAG: &str = "tags are not allowed";

#[test]
fn pack5_premise_pack_without_anchors_or_tags_compiles() {
    assert!(compile_yaml(&workshop_with_commands("{n: 31}"), CHAPTER).is_ok());
}

#[test]
fn pack5_expect_anchor_and_alias_are_rejected_in_commands() {
    for v in [
        format!("&anchor_name {{n: {MARK}}}"),
        format!("{{a: &anchor_name {MARK}, b: *anchor_name}}"),
        format!("[&anchor_name {MARK}, *anchor_name]"),
    ] {
        assert_rejected(
            compile_yaml(&workshop_with_commands(&v), CHAPTER),
            ANCHOR,
            &v,
        );
    }
}

#[test]
fn pack5_expect_merge_key_is_rejected() {
    let w = format!(
        "{WORKSHOP}commands:\n  x: &anchor_name {{n: {MARK}}}\n  y:\n    <<: *anchor_name\n"
    );
    assert_rejected(compile_yaml(&w, CHAPTER), ANCHOR, "merge key");
}

#[test]
fn pack5_expect_anchor_is_rejected_in_every_file() {
    // 型付きの欄（`pack`）。
    let w = "pack: &anchor_name {id: p, title: t, version: '0', ir_format: 1, guidance: A}\n";
    assert_rejected(compile_yaml(w, CHAPTER), ANCHOR, "workshop pack");
    // Step の欄と `input` の中。
    let ch = CHAPTER.replace("title: A", &format!("title: &anchor_name {MARK}"));
    assert_rejected(compile_yaml(WORKSHOP, &ch), ANCHOR, "step title");
    let ch =
        "id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, body: a.md, input: {kind: text, min: 1, max: &anchor_name 31}, confirm: {kind: approve}, next: [{to: b}]}\n  - {id: b, title: B, final: true}\n"
    ;
    assert_rejected(compile_yaml(WORKSHOP, ch), ANCHOR, "step input");
    // 章・モジュール・試験の記述。
    let t = put_pack(
        "feat-ch",
        WORKSHOP,
        &format!("{CHAPTER}x: &anchor_name 1\n"),
    );
    assert_rejected(compile_dir(&t.0).map(|_| ()), ANCHOR, "chapter extra");
    let t = put_pack("feat-mod", WORKSHOP, CHAPTER);
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: &anchor_name M\nsteps: []\n",
    );
    assert_rejected(compile_dir(&t.0).map(|_| ()), ANCHOR, "module");
    let t = put_pack("feat-play", WORKSHOP, CHAPTER);
    t.put("tests/playthrough.yaml", b"steps: &anchor_name []\n");
    assert_rejected(compile_dir(&t.0).map(|_| ()), ANCHOR, "playthrough");
}

#[test]
fn pack5_expect_tags_are_rejected() {
    for v in [
        format!("!secrettag {MARK}"),
        format!("!!str {MARK}"),
        "!!int 31".to_string(),
        "{n: !!int 31}".to_string(),
        "! 31".to_string(),
        "!!map {n: 1}".to_string(),
        "[!secrettag 1]".to_string(),
    ] {
        assert_rejected(compile_yaml(&workshop_with_commands(&v), CHAPTER), TAG, &v);
    }
    let ch = CHAPTER.replace("title: A", "title: !!str A");
    assert_rejected(compile_yaml(WORKSHOP, &ch), TAG, "step title");
}

#[test]
fn pack5_expect_position_points_at_the_node_line() {
    let w = format!("{WORKSHOP}commands:\n  x: 1\n  y: !!int 3\n");
    let d = compile_yaml(&w, CHAPTER).expect_err("rejected");
    assert!(d[0].to_string().contains("line 4 "), "{d:?}");
}

#[test]
fn pack5_expect_multiple_documents_are_rejected() {
    let w = format!("{WORKSHOP}---\ncommands: {{}}\n");
    let d = compile_yaml(&w, CHAPTER).expect_err("workshop");
    assert!(!d.is_empty());
    assert!(d.iter().all(|x| x.code() == DiagCode::Parse), "{d:?}");
    let ch = format!("{CHAPTER}---\nid: c2\n");
    let d = compile_yaml(WORKSHOP, &ch).expect_err("chapter");
    assert!(d.iter().all(|x| x.code() == DiagCode::Parse), "{d:?}");
}

#[test]
fn pack5_expect_validate_rejects_with_the_same_diagnostic() {
    let t = put_pack("feat-val", &workshop_with_commands("!!int 31"), CHAPTER);
    let d = validate_dir(&t.0);
    assert!(!d.is_empty());
    assert!(d.iter().all(|x| x.code() == DiagCode::Parse), "{d:?}");
}

#[test]
fn pack5_not_rejected_look_alike_text() {
    for v in [
        "\"Q&A\"",
        "'a*b'",
        "Hello!",
        "a & b",
        "\"!tag\"",
        "[x, \"*y\", '&z']",
        "|\n    * item\n    &x\n    !t\n",
        ">\n    !important\n",
    ] {
        assert!(
            compile_yaml(&workshop_with_commands(v), CHAPTER).is_ok(),
            "{v}"
        );
    }
    let w = format!("# &x comment\n{WORKSHOP}");
    assert!(compile_yaml(&w, CHAPTER).is_ok());
    // 1 つの文書の明示的な開始・終了は許す。
    let w = format!("---\n{WORKSHOP}...\n");
    assert!(compile_yaml(&w, CHAPTER).is_ok());
}
