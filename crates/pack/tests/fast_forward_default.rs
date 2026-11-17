//! 教材の欄 `pack.fast_forward` の受け入れ試験（語彙 v1.9。SHELL-25・SESS-3、#1045）。
//!
//! 早送りの初期値を入にする教材の設定である。ここで確かめるのはパック側の約束だけで、`true` が IR に出ること・
//! `false` と省略は出ないこと・型の誤りは `E-PARSE`・`pack.vocab` が 1.9 未満なら `E-VOCAB-MISMATCH` の 4 つである
//! （web が読んで初期値を決める結線は `crates/web/tests/fast_forward_host.rs`）。小さな教材を一時ディレクトリに作る。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::Tmp;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

/// 終端の Step `a` だけを持つ教材。`pack_extra` は `pack:` の欄に挿す文字列（先頭に `, ` を含む）。
fn pack(vocab: &str, pack_extra: &str) -> Tmp {
    let t = Tmp::new("ff");
    let workshop = format!(
        "pack: {{id: p, title: t, version: '0', ir_format: 1, guidance: A, vocab: \"{vocab}\"{pack_extra}}}\n"
    );
    t.put("workshop.yaml", workshop.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - id: a\n    title: A\n    body: a.md\n    final: true\n",
    );
    t.put("chapters/c1/a.md", b"body\n");
    t
}

fn ir_json(t: &Tmp) -> String {
    String::from_utf8(compile_dir(&t.0).unwrap_or_else(|d| panic!("{d:?}"))).unwrap()
}

fn only(d: &[Diagnostic], code: DiagCode) -> Vec<&Diagnostic> {
    d.iter().filter(|x| x.code() == code).collect()
}

#[test]
fn fast_forward_true_is_in_ir_and_false_or_absent_is_not() {
    let on = pack("1.9", ", fast_forward: true");
    assert!(ir_json(&on).contains("\"fast_forward\": true"));
    assert!(validate_dir(&on.0).is_empty());
    let absent = ir_json(&pack("1.9", ""));
    assert!(!absent.contains("fast_forward"));
    // `false` は省略と同じバイト列になる（使わない教材の IR はバイト同一）。
    assert_eq!(ir_json(&pack("1.9", ", fast_forward: false")), absent);
}

#[test]
fn fast_forward_non_bool_is_parse() {
    for v in ["\"yes\"", "1", "[true]"] {
        let t = pack("1.9", &format!(", fast_forward: {v}"));
        let c = compile_dir(&t.0).expect_err("compile must fail");
        assert_eq!(only(&c, DiagCode::Parse).len(), 1, "{v}: {c:?}");
    }
}

#[test]
fn fast_forward_below_vocab_1_9_is_mismatch() {
    let t = pack("1.8", ", fast_forward: true");
    let v = validate_dir(&t.0);
    assert_eq!(only(&v, DiagCode::VocabMismatch).len(), 1, "{v:?}");
    // `false` は使用に数えない。使わない教材も 1.8 のままでよい。
    assert!(validate_dir(&pack("1.8", ", fast_forward: false").0).is_empty());
    assert!(validate_dir(&pack("1.8", "").0).is_empty());
}
