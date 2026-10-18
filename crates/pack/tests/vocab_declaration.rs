//! `pack.vocab` の宣言と使用の一致の検査（`E-VOCAB-MISMATCH`）の試験（PACK-31・VER-14・D-43、TASK-59 1/2・#425）。
//!
//! 宣言より新しい語彙の版の拡張点（EP-9〜EP-12）を使う教材が `validate` で診断になること、宣言が一致する
//! 教材・`pack.vocab` を省略した教材は診断にならないこと、読めない宣言は値を写さず `E-PARSE` になることを確かめる。
//! `compile` は検査しない。比べるのは `E-VOCAB-MISMATCH` の有無と件数で、他の診断には依存しない。

mod common;

use common::{testdata, Tmp};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const REQ: &str = "{step: {step: a, state: achieved}}";

/// 形式 B のパック。`pack_extra` は `pack:` 節への追記（インデント付きの行）、`start_extra` は開始点の追記。
fn yaml_pack(pack_extra: &str, start_extra: &str) -> Tmp {
    let t = Tmp::new("vocab-decl");
    t.put(
        "workshop.yaml",
        format!("pack:\n  id: p\n  title: t\n  version: '0'\n  ir_format: 1\n  guidance: A\n{pack_extra}")
            .as_bytes(),
    );
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: a{start_extra}}}]\nsteps:\n  - {{id: a, title: A, body: a.md, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n")
            .as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

/// 形式 A（TOML）のパック。
fn toml_pack(pack_extra: &str, start_extra: &str) -> Tmp {
    let t = Tmp::new("vocab-decl-a");
    t.put(
        "workshop.toml",
        format!("[pack]\nid = \"p\"\ntitle = \"t\"\nversion = \"0\"\nir_format = 1\nguidance = \"A\"\n{pack_extra}")
            .as_bytes(),
    );
    t.put(
        "chapters/c1/chapter.toml",
        format!("id = \"c1\"\ntitle = \"C\"\n\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n{start_extra}\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nbody = \"a.md\"\n[[steps.next]]\nto = \"b\"\n\n[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n")
            .as_bytes(),
    );
    t.put("chapters/c1/a.md", b"# a\n");
    t
}

fn mismatches(t: &Tmp) -> Vec<Diagnostic> {
    validate_dir(&t.0)
        .into_iter()
        .filter(|d| d.code() == DiagCode::VocabMismatch)
        .collect()
}

fn parse_errors(t: &Tmp) -> Vec<Diagnostic> {
    validate_dir(&t.0)
        .into_iter()
        .filter(|d| d.code() == DiagCode::Parse && d.item() == "pack:vocab")
        .collect()
}

fn req_unmet() -> String {
    format!(", requires: {REQ}, unmet: rj-x")
}

#[test]
fn pack31_ep9_with_vocab_below_1_2_is_reported() {
    for v in ["1.0", "1.1"] {
        let y = yaml_pack(&format!("  vocab: \"{v}\"\n"), &req_unmet());
        let d = mismatches(&y);
        assert_eq!(d.len(), 1, "yaml vocab {v}");
        assert_eq!(d[0].file(), "workshop.yaml");
        assert_eq!(d[0].item(), "pack:vocab");
        let m = d[0].to_string();
        assert!(m.contains("EP-9") && m.contains("1.2"), "{m}");
        assert!(m.contains(&format!("declared {v}")), "{m}");

        let a = toml_pack(
            &format!("vocab = \"{v}\"\n"),
            "requires = { step = { step = \"a\", state = \"achieved\" } }\nunmet = \"rj-x\"\n",
        );
        let d = mismatches(&a);
        assert_eq!(d.len(), 1, "toml vocab {v}");
        assert_eq!(d[0].file(), "workshop.toml");
    }
}

#[test]
fn pack31_ep9_in_module_start_is_reported() {
    let t = yaml_pack("  vocab: \"1.0\"\n", "");
    t.put(
        "modules/m1/module.yaml",
        format!("id: m1\ntitle: M\nparams: {{x: y}}\nstarts: [{{id: ms, node: a, requires: {REQ}}}]\nsteps:\n  - {{id: a, title: A, final: true}}\n").as_bytes(),
    );
    assert_eq!(mismatches(&t).len(), 1);
}

#[test]
fn pack31_newer_vocab_than_declared_is_reported() {
    let fields = [
        ("EP-10", "  profile: {}\n"),
        ("EP-11", "  appearance: {theme: standard}\n"),
        ("EP-12", "  left_pane: {}\n"),
    ];
    for (ep, line) in fields {
        let t = yaml_pack(&format!("  vocab: \"1.2\"\n{line}"), "");
        let d = mismatches(&t);
        assert_eq!(d.len(), 1, "{ep}");
        assert!(d[0].to_string().contains(ep), "{ep}");
    }
    let all: String = fields.iter().map(|(_, l)| *l).collect();
    let t = yaml_pack(&format!("  vocab: \"1.2\"\n{all}"), "");
    let d = mismatches(&t);
    assert_eq!(d.len(), 3);
    for (d, (ep, _)) in d.iter().zip(fields) {
        assert!(d.to_string().contains(ep), "order: {ep}");
    }
}

#[test]
fn pack31_one_diagnostic_per_extension_point() {
    let t = yaml_pack("  vocab: \"1.0\"\n", &req_unmet());
    t.put(
        "chapters/c2/chapter.yaml",
        format!("id: c2\ntitle: C2\nstarts: [{{id: s2, node: a, requires: {REQ}}}, {{id: s3, node: b, unmet: rj-x}}]\nsteps:\n  - {{id: a, title: A, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n").as_bytes(),
    );
    assert_eq!(mismatches(&t).len(), 1);
}

#[test]
fn pack31_spec_pack_declaring_1_0_but_using_requires_is_reported() {
    let d = validate_dir(&testdata(
        "save-restore-versions/test-data/packs/v1.0.0-vocab-1.0",
    ));
    assert!(d.iter().any(|d| d.code() == DiagCode::VocabMismatch));
}

#[test]
fn pack31_matching_declarations_are_not_reported() {
    let y = yaml_pack("  vocab: \"1.2\"\n", &req_unmet());
    assert!(mismatches(&y).is_empty());

    let all = "  profile: {}\n  appearance: {theme: standard}\n  left_pane: {}\n";
    let y = yaml_pack(&format!("  vocab: \"1.3\"\n{all}"), &req_unmet());
    assert!(mismatches(&y).is_empty());

    let y = yaml_pack(all, &req_unmet());
    assert!(mismatches(&y).is_empty(), "no vocab: not checked");
    assert!(parse_errors(&y).is_empty());

    let y = yaml_pack("  vocab: \"1.0\"\n", "");
    assert_eq!(validate_dir(&y.0), vec![]);

    let d = validate_dir(&testdata("save-restore-versions/test-data/packs/v1.0.0"));
    assert!(d.iter().all(|d| d.code() != DiagCode::VocabMismatch));
}

#[test]
fn pack31_unreadable_vocab_is_parse_error_without_value() {
    for v in [
        "1.2",
        "\"1.2.0\"",
        "\"v1.2\"",
        "\"01.2\"",
        "\"\"",
        "[SECRET]",
        "\"SECRET\"",
    ] {
        let t = yaml_pack(&format!("  vocab: {v}\n"), &req_unmet());
        let p = parse_errors(&t);
        assert_eq!(p.len(), 1, "yaml {v}");
        assert!(mismatches(&t).is_empty(), "yaml {v}");
        assert!(!p[0].to_string().contains("SECRET"), "yaml {v}");
    }
    for v in ["1.2", "\"SECRET\""] {
        let t = toml_pack(&format!("vocab = {v}\n"), "");
        let p = parse_errors(&t);
        assert_eq!(p.len(), 1, "toml {v}");
        assert!(!p[0].to_string().contains("SECRET"), "toml {v}");
    }
}

#[test]
fn pack31_is_deterministic_and_validate_only() {
    let t = yaml_pack("  vocab: \"1.0\"\n", &req_unmet());
    assert_eq!(validate_dir(&t.0), validate_dir(&t.0));
    if let Err(ds) = compile_dir(&t.0) {
        assert!(ds.iter().all(|d| d.code() != DiagCode::VocabMismatch));
    }
}
