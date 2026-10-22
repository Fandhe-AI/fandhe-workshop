//! `validate` の ID の重複・形式の版・構造の検査（TASK-54 2/2）の受け入れ試験。
//!
//! `E-DUP-ID`・`E-FORMAT-VERSION`・`E-STRUCTURE`（正答の無い `answer` を含む）のそれぞれについて、欠陥を
//! 1 つだけ入れた教材パックが、期待のコードで期待のファイル・項目に報告されることを確かめる。
//!
//! 範囲外: CLI の `validate` への接続（TASK-401・TASK-403）、`E-NO-CONFIRM`・`E-MODULE-CYCLE` の受入試験は `validate_cycle_confirm.rs`、
//! 欠陥パック 35 種・両形式の網羅（TASK-56）。

mod common;

use std::fs;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const WORKSHOP_FILE: &str = "workshop.yaml";
const C1: &str = "chapters/c1/chapter.yaml";
const C2: &str = "chapters/c2/chapter.yaml";
const M1: &str = "modules/m1/module.yaml";

const WORKSHOP: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
commands: {help: {description: d}}
rejections: {rj-a: {category: not_in_this_step, message: m, next_action: n}}
";

const CHAPTER_1: &str = "\
id: c1
title: C1
starts: [{id: s1, node: a}]
steps:
-   id: a
    title: A
    input: {kind: command, name: help}
    confirm: {kind: answer, options: [{id: o1, label: A, correct: true}, {id: o2, label: B, correct: false}], on_wrong: rj-a}
    next: [{to: b}, {to: b2}]
-   {id: b, title: B, final: true}
-   {id: b2, title: B2, final: true}
";

const CHAPTER_2: &str = "\
id: c2
title: C2
starts: [{id: s2, node: c2z}]
steps:
    - {id: c2z, title: Z, final: true}
";

const MODULE: &str = "\
id: m1
title: M
steps:
    - {id: m-a, title: MA, next: [{to: m-b}, {to: m-c}]}
    - {id: m-b, title: MB, final: true}
    - {id: m-c, title: MC, final: true}
";

fn base() -> Tmp {
    let t = Tmp::new("structure");
    t.put(WORKSHOP_FILE, WORKSHOP.as_bytes());
    t.put(C1, CHAPTER_1.as_bytes());
    t.put(C2, CHAPTER_2.as_bytes());
    t.put(M1, MODULE.as_bytes());
    t.put("README.md", format!("# r\n{SECRET}\n").as_bytes());
    t
}

fn mutate(t: &Tmp, file: &str, from: &str, to: &str) {
    let path = t.0.join(file);
    let text = fs::read_to_string(&path).expect("read");
    assert_eq!(text.matches(from).count(), 1, "`{from}` must occur once");
    fs::write(path, text.replacen(from, to, 1)).expect("write");
}

fn only(d: &[Diagnostic], code: DiagCode, file: &str, item: &str) {
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), code, "{d:?}");
    assert_eq!(d[0].file(), file, "{d:?}");
    assert_eq!(d[0].item(), item, "{d:?}");
    assert_no_secret(d);
}

#[test]
fn premise_base_pack_has_no_diagnostics() {
    let t = base();
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
}

/// `(名前, 書き換える file, from, to, 期待のコード, 期待の file, 期待の item)`。
type Case = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    DiagCode,
    &'static str,
    &'static str,
);

fn cases() -> Vec<Case> {
    use DiagCode::*;
    vec![
        (
            "dup-step-across-chapters",
            C2,
            "- {id: c2z, title: Z, final: true}",
            "- {id: c2z, title: Z, next: [{to: b}]}\n    - {id: b, title: Z2, final: true}",
            DupId,
            C2,
            "step:b",
        ),
        (
            "dup-step-in-chapter",
            C1,
            "-   {id: b2, title: B2, final: true}",
            "-   {id: b2, title: B2, final: true}\n-   {id: a, title: A2, final: true}",
            DupId,
            C1,
            "step:a",
        ),
        (
            "dup-step-in-module",
            M1,
            "- {id: m-c, title: MC, final: true}",
            "- {id: m-c, title: MC, final: true}\n    - {id: m-a, title: MX, final: true}",
            DupId,
            M1,
            "step:m-a",
        ),
        (
            "dup-start-across-chapters",
            C2,
            "id: s2",
            "id: s1",
            DupId,
            C2,
            "start:s1",
        ),
        (
            "format-version",
            WORKSHOP_FILE,
            "ir_format: 1",
            "ir_format: 3",
            FormatVersion,
            WORKSHOP_FILE,
            "pack:ir_format",
        ),
        (
            "answer-without-correct-option",
            C1,
            "correct: true",
            "correct: false",
            Structure,
            C1,
            "step:a",
        ),
        (
            "use-and-input",
            C2,
            "- {id: c2z, title: Z, final: true}",
            "- {id: c2z, title: Z, final: true, use: {module: m1, instance: i1, args: {}}, input: {kind: command, name: help}}",
            Structure,
            C2,
            "step:c2z",
        ),
    ]
}

#[test]
fn each_defect_is_reported_once_at_the_expected_place() {
    for (name, file, from, to, code, want_file, want_item) in cases() {
        let t = base();
        mutate(&t, file, from, to);
        let d = validate_dir(&t.0);
        eprintln!("case: {name}");
        only(&d, code, want_file, want_item);
    }
}

#[test]
fn all_three_codes_are_covered() {
    let codes: Vec<DiagCode> = cases().iter().map(|c| c.4).collect();
    for code in [
        DiagCode::DupId,
        DiagCode::FormatVersion,
        DiagCode::Structure,
    ] {
        assert!(codes.contains(&code), "{code:?} is not covered");
    }
}

#[test]
fn unsafe_duplicate_id_is_not_echoed() {
    let t = base();
    mutate(
        &t,
        C2,
        "- {id: c2z, title: Z, final: true}",
        "- {id: c2z, title: Z, final: true}\n    - {id: \"a b\", title: X, final: true}",
    );
    mutate(
        &t,
        C1,
        "-   {id: b2, title: B2, final: true}",
        "-   {id: b2, title: B2, final: true}\n-   {id: \"a b\", title: X, final: true}",
    );
    let d = validate_dir(&t.0);
    let dup: Vec<_> = d.iter().filter(|d| d.code() == DiagCode::DupId).collect();
    assert_eq!(dup.len(), 1, "{d:?}");
    assert_eq!(dup[0].message(), "duplicate step id: (id not shown)");
    assert_no_secret(&d);
}
