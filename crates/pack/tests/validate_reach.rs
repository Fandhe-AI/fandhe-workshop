//! `validate` の到達性（`E-UNREACHABLE`）と行き止まり（`E-DEAD-END`）の検査（TASK-55 1/2）の受け入れ試験。
//!
//! 欠陥を 1 つだけ入れた教材パックが、期待のコードで期待のファイル・項目に報告されること、条件つきの辺だけで
//! つながる Step を到達可能と判定すること、参照切れでは `E-REF-STEP` だけが出て派生しないことを確かめる。
//!
//! 範囲外: `E-MODULE-CYCLE`・`E-NO-CONFIRM` は `validate_cycle_confirm.rs`、CLI への接続（TASK-401・TASK-403）、
//! 欠陥パックの網羅（TASK-56）。

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
";

/// `cond` は条件つきの辺だけで入る Step。`c2a` は別の章の Step で、章をまたぐ辺で入る。
const CHAPTER_1: &str = "\
id: c1
title: C1
starts: [{id: s1, node: a}]
steps:
-   {id: a, title: A, next: [{to: cond, when: {flag: f}}, {to: u}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
-   {id: cond, title: Cond, next: [{to: c2a}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
-   {id: u, title: U, use: {module: m1, instance: i1, args: {}}, next: [{to: c2a}]}
";

const CHAPTER_2: &str = "\
id: c2
title: C2
steps:
    - {id: c2a, title: C2A, final: true}
";

/// `m-mid` は途中開始の `starts` だけで入る Step。
const MODULE: &str = "\
id: m1
title: M
starts: [{id: ms, node: m-mid}]
steps:
    - {id: m-a, title: MA, next: [{to: m-b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-a, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
    - {id: m-b, title: MB, final: true}
    - {id: m-mid, title: MM, next: [{to: m-b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-mid, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}
";

const C2A: &str = "- {id: c2a, title: C2A, final: true}";
const COND: &str = "-   {id: cond, title: Cond, next: [{to: c2a}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}";

fn base() -> Tmp {
    let t = Tmp::new("reach");
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
    let d = validate_dir(&base().0);
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn step_reached_only_by_conditional_edges_is_reachable() {
    let t = base();
    // `a` のすべての辺を条件つきにしても、`cond` は条件つきの辺で到達可能のまま（`E-DEAD-END` は別に出る）。
    mutate(
        &t,
        C1,
        "next: [{to: cond, when: {flag: f}}, {to: u}],",
        "next: [{to: cond, when: {flag: f}}, {to: u, when: {flag: g}}],",
    );
    let d = validate_dir(&t.0);
    assert!(d.iter().all(|d| d.code() != DiagCode::Unreachable), "{d:?}");
    assert!(d.iter().any(|d| d.code() == DiagCode::DeadEnd), "{d:?}");
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
            "unreachable-chapter-step",
            C2,
            C2A,
            "- {id: c2a, title: C2A, final: true}\n    - {id: orphan, title: O, next: [{to: c2a}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-orphan, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            Unreachable,
            C2,
            "step:orphan",
        ),
        (
            "unreachable-module-step",
            M1,
            "- {id: m-b, title: MB, final: true}",
            "- {id: m-b, title: MB, final: true}\n    - {id: m-orphan, title: MO, next: [{to: m-b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-orphan, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            Unreachable,
            M1,
            "step:m-orphan",
        ),
        (
            "dead-end-empty-next",
            C1,
            COND,
            "-   {id: cond, title: Cond, input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            DeadEnd,
            C1,
            "step:cond",
        ),
        (
            "dead-end-all-conditional",
            C1,
            COND,
            "-   {id: cond, title: Cond, next: [{to: c2a, when: {flag: f}}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            DeadEnd,
            C1,
            "step:cond",
        ),
        (
            "dead-end-use-step",
            C1,
            "-   {id: u, title: U, use: {module: m1, instance: i1, args: {}}, next: [{to: c2a}]}",
            "-   {id: u, title: U, use: {module: m1, instance: i1, args: {}}}",
            DeadEnd,
            C1,
            "step:u",
        ),
        (
            "dead-end-module-step",
            M1,
            "- {id: m-mid, title: MM, next: [{to: m-b}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-mid, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            "- {id: m-mid, title: MM, input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-m-mid, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
            DeadEnd,
            M1,
            "step:m-mid",
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
fn both_codes_are_covered() {
    let codes: Vec<DiagCode> = cases().iter().map(|c| c.4).collect();
    for code in [DiagCode::Unreachable, DiagCode::DeadEnd] {
        assert!(codes.contains(&code), "{code:?} is not covered");
    }
}

#[test]
fn one_unconditional_edge_is_enough() {
    let t = base();
    mutate(
        &t,
        C1,
        COND,
        "-   {id: cond, title: Cond, next: [{to: c2a, when: {flag: f}}, {to: c2a}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
    );
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
}

#[test]
fn dangling_refs_report_only_ref_step() {
    let cases = [
        (
            "next-to",
            C1,
            COND,
            "-   {id: cond, title: Cond, next: [{to: nowhere}], input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-cond, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
        ),
        ("start-node", C1, "node: a}", "node: nowhere}"),
        (
            "module-next",
            M1,
            "{id: m-a, title: MA, next: [{to: m-b}],",
            "{id: m-a, title: MA, next: [{to: nowhere}],",
        ),
    ];
    for (name, file, from, to) in cases {
        let t = base();
        mutate(&t, file, from, to);
        let d = validate_dir(&t.0);
        eprintln!("case: {name}");
        assert!(!d.is_empty(), "{name}");
        assert!(
            d.iter().all(|d| d.code() == DiagCode::RefStep),
            "{name}: {d:?}"
        );
    }
}

#[test]
fn unsafe_id_is_not_echoed() {
    let t = base();
    mutate(
        &t,
        C2,
        C2A,
        "- {id: c2a, title: C2A, final: true}\n    - {id: \"a b\", title: O, final: true}",
    );
    let d = validate_dir(&t.0);
    only(&d, DiagCode::Unreachable, C2, "step:a b");
    assert_eq!(
        d[0].message(),
        "step is not reachable from any start: (id not shown)"
    );
}

#[test]
fn diagnostics_order_is_deterministic() {
    let t = base();
    mutate(
        &t,
        C2,
        C2A,
        "- {id: c2a, title: C2A, final: true}\n    - {id: o1, title: O, input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-o1, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}\n    - {id: o2, title: O, input: {kind: choice, options: [{id: ok, label: OK}]}, rules: [{id: r-o2, on: {option: ok}, outcome: {kind: accept}}], confirm: {kind: none, reason: r}}",
    );
    let a = validate_dir(&t.0);
    let b = validate_dir(&t.0);
    assert_eq!(a, b);
    let items: Vec<_> = a.iter().map(|d| (d.code(), d.item().to_string())).collect();
    assert_eq!(
        items,
        vec![
            (DiagCode::Unreachable, "step:o1".to_string()),
            (DiagCode::DeadEnd, "step:o1".to_string()),
            (DiagCode::Unreachable, "step:o2".to_string()),
            (DiagCode::DeadEnd, "step:o2".to_string()),
        ]
    );
}
