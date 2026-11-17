//! Step の欄 `normal_speed` の受け入れ試験（語彙 v1.8。SHELL-26、#1043）。
//!
//! 処理中の進行そのものが学習内容の Step を、早送りの設定にかかわらず通常の速さで動かす印である。ここで確かめるのは
//! パック側の約束だけで、`true` が IR に出ること・`false` と省略は出ないこと・型の誤りは `E-PARSE`・`processing` の無い
//! Step の `true` は `E-STRUCTURE`・`pack.vocab` が 1.8 未満なら `E-VOCAB-MISMATCH` の 5 つである（web が読んで早送りを
//! 止める結線は別）。小さな教材を一時ディレクトリに作る。試験では `unwrap` を使ってよい。

mod common;

use common::Tmp;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

fn workshop(vocab: &str) -> String {
    format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, vocab: \"{vocab}\"}}\n"
    )
}

/// 終端の Step `a` だけを持つ教材。`a_extra` は `a` の欄に挿す 1 行（インデント付き）。
fn pack(vocab: &str, a_extra: &str) -> Tmp {
    let t = Tmp::new("ns");
    t.put("workshop.yaml", workshop(vocab).as_bytes());
    let chapter = format!(
        "id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - id: a\n    title: A\n    body: a.md\n{a_extra}    final: true\n"
    );
    t.put("chapters/c1/chapter.yaml", chapter.as_bytes());
    t.put("chapters/c1/a.md", b"body\n");
    t
}

const PROC: &str = "    processing: [{name: p1}]\n";

fn ir_json(t: &Tmp) -> String {
    String::from_utf8(compile_dir(&t.0).unwrap_or_else(|d| panic!("{d:?}"))).unwrap()
}

fn only(d: &[Diagnostic], code: DiagCode) -> Vec<&Diagnostic> {
    d.iter().filter(|x| x.code() == code).collect()
}

#[test]
fn normal_speed_true_is_in_ir_and_false_or_absent_is_not() {
    let on = pack("1.8", &format!("{PROC}    normal_speed: true\n"));
    assert!(ir_json(&on).contains("\"normal_speed\": true"));
    assert!(validate_dir(&on.0).is_empty());
    for extra in [format!("{PROC}    normal_speed: false\n"), PROC.to_string()] {
        let t = pack("1.8", &extra);
        assert!(!ir_json(&t).contains("normal_speed"), "{extra}");
    }
}

#[test]
fn normal_speed_non_bool_is_parse() {
    for v in ["\"yes\"", "1", "[true]"] {
        let t = pack("1.8", &format!("{PROC}    normal_speed: {v}\n"));
        let c = compile_dir(&t.0).expect_err("compile must fail");
        assert_eq!(only(&c, DiagCode::Parse).len(), 1, "{v}: {c:?}");
    }
}

#[test]
fn normal_speed_without_processing_is_structure_in_both() {
    let t = pack("1.8", "    normal_speed: true\n");
    let c = compile_dir(&t.0).expect_err("compile must fail");
    assert_eq!(c.len(), 1, "{c:?}");
    assert_eq!(c[0].code(), DiagCode::Structure);
    assert!(c[0].to_string().contains("step:a"), "{}", c[0]);
    let v = validate_dir(&t.0);
    assert_eq!(only(&v, DiagCode::Structure).len(), 1, "{v:?}");
    // false は印ではないので、processing が無くても通る。
    let off = pack("1.8", "    normal_speed: false\n");
    assert!(validate_dir(&off.0).is_empty());
}

#[test]
fn normal_speed_below_vocab_1_8_is_mismatch() {
    let t = pack("1.7", &format!("{PROC}    normal_speed: true\n"));
    let v = validate_dir(&t.0);
    assert_eq!(only(&v, DiagCode::VocabMismatch).len(), 1, "{v:?}");
    // 使わない教材は 1.7 のままでよい。
    let unused = pack("1.7", PROC);
    assert!(validate_dir(&unused.0).is_empty());
}
