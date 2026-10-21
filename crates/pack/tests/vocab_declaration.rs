//! `pack.vocab` の宣言と使用の一致の検査（`E-VOCAB-MISMATCH`）の試験（PACK-31・VER-14・D-43、TASK-59 1/2・2/2・#425・#426）。
//!
//! 宣言より新しい語彙の版の拡張点（EP-9〜EP-12）と確認の選択肢の `label`（v1.3）を使う教材が `validate` で診断になること、宣言が一致する
//! 教材・`pack.vocab` を省略した教材は診断にならないこと、読めない宣言は値を写さず `E-PARSE` になることを確かめる。
//! 入力方式 `choice` の `label`（v1）は別の欄で診断にならないこと、宣言と使用が一致する教材で誤検出が 0 件であることも確かめる。
//! `compile` は検査しない。比べるのは `E-VOCAB-MISMATCH` の有無と件数で、他の診断には依存しない。

mod common;

use common::{copy_dir, testdata, Tmp};
use std::fs;
use std::path::Path;

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

#[test]
fn pack31_explicit_null_vocab_is_parse_error_but_omission_passes() {
    for v in ["null", "~", ""] {
        let t = yaml_pack(&format!("  vocab: {v}\n"), &req_unmet());
        assert_eq!(parse_errors(&t).len(), 1, "yaml vocab: {v:?}");
        assert!(mismatches(&t).is_empty(), "yaml vocab: {v:?}");
    }
    let t = yaml_pack("", &req_unmet());
    assert!(parse_errors(&t).is_empty());
    assert!(mismatches(&t).is_empty());
}

// ---- confirm.options[].label（語彙 1.3。TASK-59 2/2）----

const LABEL_TERM: &str = "confirm.options[].label";

fn label_mismatches(t: &Tmp) -> Vec<Diagnostic> {
    mismatches(t)
        .into_iter()
        .filter(|d| d.to_string().contains(LABEL_TERM))
        .collect()
}

/// 確認 `answer` の選択肢に `label` を持つ Step を 1 つ持つ形式 B のパック。
fn label_pack(vocab_line: &str, label: &str) -> Tmp {
    let t = yaml_pack(vocab_line, "");
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: a}}]\nsteps:\n  - {{id: a, title: A, body: a.md, confirm: {{kind: answer, options: [{{id: ok, correct: true{label}}}, {{id: no, correct: false}}], on_wrong: rj-x}}, next: [{{to: b}}]}}\n  - {{id: b, title: B, final: true}}\n").as_bytes(),
    );
    t
}

#[test]
fn pack31_label_with_vocab_below_1_3_is_reported() {
    for v in ["1.0", "1.1", "1.2"] {
        let t = label_pack(&format!("  vocab: \"{v}\"\n"), ", label: SECRET");
        let d = label_mismatches(&t);
        assert_eq!(d.len(), 1, "yaml vocab {v}");
        assert_eq!(d[0].file(), "workshop.yaml");
        assert_eq!(d[0].item(), "pack:vocab");
        let m = d[0].to_string();
        assert!(
            m.contains("1.3") && m.contains(&format!("declared {v}")),
            "{m}"
        );
        assert!(!m.contains("SECRET"), "{m}");
    }
    let a = toml_pack("vocab = \"1.2\"\n", "");
    a.put(
        "chapters/c1/chapter.toml",
        b"id = \"c1\"\ntitle = \"C\"\n\n[[starts]]\nid = \"s1\"\nnode = \"a\"\n\n[[steps]]\nid = \"a\"\ntitle = \"A\"\nbody = \"a.md\"\nconfirm = { kind = \"answer\", on_wrong = \"rj-x\", options = [{ id = \"ok\", correct = true, label = \"L\" }, { id = \"no\", correct = false }] }\n[[steps.next]]\nto = \"b\"\n\n[[steps]]\nid = \"b\"\ntitle = \"B\"\nfinal = true\n",
    );
    let d = label_mismatches(&a);
    assert_eq!(d.len(), 1, "toml");
    assert_eq!(d[0].file(), "workshop.toml");
}

#[test]
fn pack31_label_in_module_step_is_reported() {
    let t = yaml_pack("  vocab: \"1.0\"\n", "");
    t.put(
        "modules/m1/module.yaml",
        b"id: m1\ntitle: M\nparams: {x: y}\nstarts: [{id: ms, node: a}]\nsteps:\n  - {id: a, title: A, confirm: {kind: \"${x}\", options: [{id: ok, label: L}]}, final: true}\n",
    );
    assert_eq!(label_mismatches(&t).len(), 1);
}

#[test]
fn pack31_label_reported_once_per_pack_and_orders_after_ep12() {
    let all = "  profile: {}\n  appearance: {theme: standard}\n  left_pane: {}\n";
    let t = label_pack(&format!("  vocab: \"1.0\"\n{all}"), ", label: L");
    t.put(
        "chapters/c1/chapter.yaml",
        format!("id: c1\ntitle: C\nstarts: [{{id: s1, node: a, requires: {REQ}, unmet: rj-x}}]\nsteps:\n  - {{id: a, title: A, confirm: {{kind: answer, options: [{{id: ok, correct: true, label: L}}, {{id: no, correct: false, label: M}}], on_wrong: rj-x}}, next: [{{to: b}}]}}\n  - {{id: b, title: B, confirm: {{kind: answer, options: [{{id: ok, correct: true, label: L}}], on_wrong: rj-x}}, final: true}}\n").as_bytes(),
    );
    let d = mismatches(&t);
    assert_eq!(d.len(), 5, "{d:?}");
    assert!(d[4].to_string().contains(LABEL_TERM));
    assert!(d[..4].iter().all(|x| !x.to_string().contains(LABEL_TERM)));
}

#[test]
fn pack31_input_choice_label_is_not_confirm_label() {
    let t = yaml_pack("  vocab: \"1.0\"\n", "");
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: a}]\nsteps:\n  - {id: a, title: A, body: a.md, input: {kind: choice, options: [{id: ok, label: L1}, {id: no, label: L2}]}, confirm: {kind: answer, options: [{id: ok, correct: true}, {id: no, correct: false}], on_wrong: rj-x}, next: [{to: b}]}\n  - {id: b, title: B, final: true}\n",
    );
    assert!(label_mismatches(&t).is_empty());
    assert!(mismatches(&t).is_empty());
}

#[test]
fn pack31_label_matching_or_omitted_or_null_is_not_reported() {
    let t = label_pack("  vocab: \"1.3\"\n", ", label: L");
    assert!(mismatches(&t).is_empty());
    let t = label_pack("", ", label: L");
    assert!(mismatches(&t).is_empty());
    assert!(parse_errors(&t).is_empty());
    let t = label_pack("  vocab: \"1.0\"\n", ", label: null");
    assert!(label_mismatches(&t).is_empty());
}

#[test]
fn pack31_label_is_validate_only() {
    let t = label_pack("  vocab: \"1.0\"\n", ", label: L");
    assert_eq!(validate_dir(&t.0), validate_dir(&t.0));
    if let Err(ds) = compile_dir(&t.0) {
        assert!(ds.iter().all(|d| d.code() != DiagCode::VocabMismatch));
    }
}

// ---- 誤検出 0 件の確認 ----

/// `workshop.*` の `pack` 節の直後に `pack.vocab` を差し込む。差し込めなければ panic する（空振り防止）。
fn declare(dir: &Path, vocab: &str) -> Tmp {
    let t = Tmp::new("vocab-fp");
    copy_dir(dir, &t.0);
    for (file, head, line) in [
        ("workshop.yaml", "pack:", format!("  vocab: \"{vocab}\"")),
        ("workshop.toml", "[pack]", format!("vocab = \"{vocab}\"")),
    ] {
        let path = t.0.join(file);
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        assert!(!text.lines().any(|l| l.trim_start().starts_with("vocab")));
        let mut out = String::new();
        let mut done = false;
        for l in text.lines() {
            if !done && file == "workshop.yaml" && l.starts_with("pack: {") {
                // フロー形（`pack: {id: ...}`）は先頭に差し込む。
                out.push_str(&format!(
                    "pack: {{vocab: \"{vocab}\", {}\n",
                    &l["pack: {".len()..]
                ));
                done = true;
                continue;
            }
            out.push_str(l);
            out.push('\n');
            if !done && l == head {
                out.push_str(&line);
                out.push('\n');
                done = true;
            }
        }
        assert!(done, "no `{head}` line in {}", dir.display());
        fs::write(&path, out).expect("write");
        return t;
    }
    panic!("no workshop file in {}", dir.display());
}

#[test]
fn pack31_correct_packs_have_no_false_positives() {
    // 宣言が使用と一致する教材は、そのまま 0 件。
    for p in ["v1.0.0", "v1.1.0", "v1.2.0", "v2.0.0", "other-pack"] {
        let d = validate_dir(&testdata(&format!(
            "save-restore-versions/test-data/packs/{p}"
        )));
        assert!(d.iter().all(|d| d.code() != DiagCode::VocabMismatch), "{p}");
    }
    // 入力方式 `choice` の `label` を持つ教材に最低の版を宣言しても、`label` の診断は 0 件。
    let mut checked = 0;
    // `condition-vocab-pack`・`pack-defects/base/format-b-v1_1` は、確認 `answer` の `label` が必須になった（PACK-37・D-67）ため、
    // 入力方式 `choice` の `label` だけを持つ前提から外れた。取り違えの確認は、残る 3 件で続ける。
    for p in [
        "module-reuse-rounds/test-data/pack",
        "input-field-ime/test-data/ch1-pack",
        "engine-event-consistency/test-data/min-3step-pack",
    ] {
        let t = declare(&testdata(p), "1.0");
        assert!(label_mismatches(&t).is_empty(), "{p}");
        checked += 1;
    }
    println!("false-positive packs checked: {checked}");
    // 確認の `label` と入力方式 `choice` の `label` が同居する教材は、宣言 1.3 で 0 件。
    let confirm =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/confirm-label-pack");
    let t = declare(&confirm, "1.3");
    assert!(mismatches(&t).is_empty());
}

#[test]
fn pack31_confirm_label_pack_positive_control() {
    let confirm =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/confirm-label-pack");
    let t = declare(&confirm, "1.2");
    assert_eq!(label_mismatches(&t).len(), 1);
}
