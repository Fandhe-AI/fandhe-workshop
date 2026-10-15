//! `validate` のモジュール呼び出しの巡回（`E-MODULE-CYCLE`）と確認の欠落（`E-NO-CONFIRM`）の検査（TASK-55 2/2）の
//! 受け入れ試験。
//!
//! 欠陥を 1 つだけ入れた教材パックが、期待のコードで期待のファイル・項目に報告されること、引数の置換後に
//! 判明する確認の欠落が `validate` にも届くこと、`compile` と同じ報告になること（パリティ）、
//! 診断の文面に引数の値・本文を写さないことを確かめる。
//!
//! 範囲外: CLI への接続（TASK-401・TASK-403）、欠陥パックの網羅（TASK-56）、`E-PROFILE-*`（TASK-46）。

mod common;

use std::collections::BTreeSet;

use common::{assert_no_secret, Tmp, SECRET};

use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use fandhe_workshop_pack::{DiagCode, Diagnostic};

const C1: &str = "chapters/c1/chapter.yaml";
const M1: &str = "modules/m1/module.yaml";
const M2: &str = "modules/m2/module.yaml";
const M3: &str = "modules/m3/module.yaml";

const WORKSHOP: &str = "\
pack: {id: p, title: t, version: '0', ir_format: 1, guidance: A}
commands: {help: {description: d}}
";

const CHAPTER: &str = "\
id: c1
title: C1
starts: [{id: s1, node: a}]
steps:
    - {id: a, title: A, next: [{to: u}]}
    - {id: u, title: U, use: {module: m1, instance: i1, args: {}}, next: [{to: f}]}
    - {id: f, title: F, final: true}
";

const MODULE: &str = "\
id: m1
title: M
params: {x: ''}
steps:
    - {id: ma, title: MA, final: true}
";

const A: &str = "    - {id: a, title: A, next: [{to: u}]}";

fn module(id: &str, steps: &str) -> String {
    format!("id: {id}\ntitle: M\nparams: {{x: ''}}\nsteps:\n{steps}")
}

/// `module` を呼んで終わる 1 Step（`instance` は実体の ID）。
fn call(module: &str, instance: &str) -> String {
    format!(
        "    - {{id: call, title: C, use: {{module: {module}, instance: {instance}, args: {{}}}}, final: true}}"
    )
}

fn base() -> Tmp {
    let t = Tmp::new("cycle-confirm");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(C1, CHAPTER.as_bytes());
    t.put(M1, MODULE.as_bytes());
    t.put("README.md", format!("# r\n{SECRET}\n").as_bytes());
    t
}

fn put_module(t: &Tmp, file: &str, id: &str, steps: &str) {
    t.put(file, module(id, steps).as_bytes());
}

/// 2 コードの `(code, file, item)` の集合。パリティの比較に使う。
fn triples(d: &[Diagnostic]) -> BTreeSet<(String, String, String)> {
    d.iter()
        .filter(|d| matches!(d.code(), DiagCode::ModuleCycle | DiagCode::NoConfirm))
        .map(|d| {
            (
                format!("{:?}", d.code()),
                d.file().to_string(),
                d.item().to_string(),
            )
        })
        .collect()
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

// ---- E-NO-CONFIRM ----

/// 置換後の検査の入口になるモジュール。`reason` は引数 `why` で決まる。
const REASON_MODULE: &str = "\
id: m1
title: M
params: {why: x}
steps:
    - {id: ma, title: MA, input: {kind: approval}, confirm: {kind: none, reason: \"${why}\"}, final: true}
";

fn with_step_a(step: &str) -> Tmp {
    let t = base();
    t.put(C1, CHAPTER.replace(A, step).as_bytes());
    t
}

/// 置換後に `reason` が空になる呼び出し。
fn empty_after_substitution() -> Tmp {
    let t = base();
    t.put(M1, REASON_MODULE.as_bytes());
    t.put(
        C1,
        CHAPTER.replace("args: {}", "args: {why: ''}").as_bytes(),
    );
    t
}

fn confirm_cases() -> Vec<(&'static str, Tmp, &'static str, &'static str)> {
    let module_step = base();
    put_module(
        &module_step,
        M1,
        "m1",
        "    - {id: ma, title: MA, input: {kind: approval}, final: true}",
    );
    vec![
        (
            "chapter-step",
            with_step_a("    - {id: a, title: A, input: {kind: approval}, next: [{to: u}]}"),
            C1,
            "step:a",
        ),
        (
            "none-without-reason",
            with_step_a("    - {id: a, title: A, input: {kind: approval}, confirm: {kind: none}, next: [{to: u}]}"),
            C1,
            "step:a",
        ),
        (
            "blank-reason",
            with_step_a("    - {id: a, title: A, input: {kind: approval}, confirm: {kind: none, reason: '  '}, next: [{to: u}]}"),
            C1,
            "step:a",
        ),
        (
            "wait-input",
            with_step_a("    - {id: a, title: A, input: {kind: wait}, processing: [{name: p1}], next: [{to: u}]}"),
            C1,
            "step:a",
        ),
        ("module-step", module_step, M1, "step:ma"),
        (
            "empty-after-substitution",
            empty_after_substitution(),
            M1,
            "step:ma",
        ),
    ]
}

#[test]
fn each_missing_confirm_is_reported_once_at_the_expected_place() {
    for (name, t, file, item) in confirm_cases() {
        eprintln!("case: {name}");
        let d = validate_dir(&t.0);
        only(&d, DiagCode::NoConfirm, file, item);
    }
}

#[test]
fn reason_given_through_arguments_is_accepted() {
    let t = base();
    t.put(M1, REASON_MODULE.as_bytes());
    t.put(
        C1,
        CHAPTER
            .replace("args: {}", "args: {why: by-design}")
            .as_bytes(),
    );
    let d = validate_dir(&t.0);
    assert!(d.is_empty(), "{d:?}");
}

// ---- E-MODULE-CYCLE ----

fn only_cycle(t: &Tmp, file: &str, message: &str) {
    let d = validate_dir(&t.0);
    only(&d, DiagCode::ModuleCycle, file, "step:call");
    assert!(d[0].message().contains(message), "{d:?}");
}

fn self_call() -> Tmp {
    let t = base();
    put_module(&t, M1, "m1", &call("m1", "i2"));
    t
}

#[test]
fn self_call_is_a_cycle() {
    only_cycle(&self_call(), M1, "m1 → m1");
}

#[test]
fn mutual_call_is_reported_at_the_closing_use() {
    let t = base();
    put_module(&t, M1, "m1", &call("m2", "i2"));
    put_module(&t, M2, "m2", &call("m1", "i3"));
    only_cycle(&t, M2, "m1 → m2 → m1");
}

#[test]
fn cycle_through_an_intermediate_module_is_reported() {
    let t = base();
    put_module(&t, M1, "m1", &call("m2", "i2"));
    put_module(&t, M2, "m2", &call("m3", "i3"));
    put_module(&t, M3, "m3", &call("m1", "i4"));
    only_cycle(&t, M3, "m1 → m2 → m3 → m1");
}

/// 章から `n` 段の連鎖（巡回しない）で呼ぶ。
fn nested(n: usize) -> Tmp {
    let t = base();
    for i in 1..=n {
        let id = format!("n{i}");
        let steps = if i < n {
            call(&format!("n{}", i + 1), &format!("i{i}"))
        } else {
            "    - {id: end, title: E, final: true}".to_string()
        };
        put_module(&t, &format!("modules/{id}/module.yaml"), &id, &steps);
    }
    t.put(C1, CHAPTER.replace("module: m1", "module: n1").as_bytes());
    t
}

#[test]
fn nesting_beyond_the_limit_is_reported_and_the_limit_itself_is_not() {
    let d = validate_dir(&nested(8).0);
    assert!(d.is_empty(), "{d:?}");
    let d = validate_dir(&nested(10).0);
    assert!(!d.is_empty(), "{d:?}");
    assert!(d.iter().all(|d| d.code() == DiagCode::ModuleCycle), "{d:?}");
    assert!(d[0].message().contains("nesting exceeds"), "{d:?}");
    assert_no_secret(&d);
}

#[test]
fn cycle_through_an_argument_module_name_hides_the_route() {
    let t = base();
    t.put(C1, CHAPTER.replace("args: {}", "args: {x: m1}").as_bytes());
    put_module(
        &t,
        M1,
        "m1",
        "    - {id: call, title: C, use: {module: \"${x}\", instance: i2, args: {x: m1}}, final: true}",
    );
    let d = validate_dir(&t.0);
    only(&d, DiagCode::ModuleCycle, M1, "step:call");
    assert_eq!(d[0].message(), "module call cycle", "{d:?}");
}

#[test]
fn argument_values_are_not_echoed() {
    let t = self_call();
    t.put(
        C1,
        CHAPTER
            .replace("args: {}", "args: {x: SECRET-ARG-VALUE}")
            .as_bytes(),
    );
    let d = validate_dir(&t.0);
    assert!(!d.is_empty(), "{d:?}");
    for x in &d {
        assert!(!x.message().contains("SECRET-ARG-VALUE"), "{d:?}");
    }
}

#[test]
fn cycle_reached_from_two_instances_is_reported_once() {
    let t = self_call();
    let two = CHAPTER
        .replace("next: [{to: f}]}", "next: [{to: u2}]}")
        .replace(
            "    - {id: f,",
            "    - {id: u2, title: U2, use: {module: m1, instance: i5, args: {}}, next: [{to: f}]}\n    - {id: f,",
        );
    t.put(C1, two.as_bytes());
    let d = validate_dir(&t.0);
    assert_eq!(
        d.iter()
            .filter(|d| d.code() == DiagCode::ModuleCycle)
            .count(),
        1,
        "{d:?}"
    );
}

// ---- 全体 ----

#[test]
fn undefined_module_is_ref_module_only() {
    let t = base();
    t.put(C1, CHAPTER.replace("module: m1", "module: nope").as_bytes());
    let d = validate_dir(&t.0);
    assert_eq!(d.len(), 1, "{d:?}");
    assert_eq!(d[0].code(), DiagCode::RefModule, "{d:?}");
}

#[test]
fn validate_and_compile_agree_on_the_two_codes() {
    let mut packs: Vec<Tmp> = confirm_cases().into_iter().map(|c| c.1).collect();
    packs.push(nested(10));
    packs.push(self_call());
    for p in &packs {
        let v = triples(&validate_dir(&p.0));
        let c = triples(&compile_dir(&p.0).err().unwrap_or_default());
        assert!(!v.is_empty(), "{:?}", p.0);
        assert_eq!(v, c);
    }
}

#[test]
fn diagnostics_order_is_deterministic() {
    let t = nested(10);
    assert_eq!(validate_dir(&t.0), validate_dir(&t.0));
}

// ---- E-LIMIT（展開の打ち切りは検査の未完了として返す） ----

#[test]
fn expansion_halted_by_the_limit_is_not_reported_as_clean() {
    // 先行する実体のファンアウトで予算を使い切ると、後続の巡回・確認の欠落は検査されない。
    // その場合に診断なしを返さず、E-LIMIT を返す。
    let refs = vec!["${x}"; 300].join(",");
    let m1 = format!(
        "id: m1\ntitle: M\nparams: {{x: d}}\nsteps:\n  - {{id: a, title: \"{refs}\", final: true}}\n"
    );
    let big = "a".repeat(100_000);
    let mut chapter = String::from("id: c1\ntitle: C\nstarts: [{id: s1, node: u0}]\nsteps:\n");
    for k in 0..6 {
        chapter.push_str(&format!(
            "  - {{id: u{k}, title: U, final: true, use: {{module: m1, instance: i{k}, args: {{x: {big}}}}}}}\n"
        ));
    }
    // 巡回は章 c2 にあり、c1 の開始の展開で打ち切られた後には展開されない。
    let c2 = format!("id: c2\ntitle: C2\nsteps:\n{}\n", call("m2", "j"));
    let t = Tmp::new("limit-halt");
    t.put("workshop.yaml", WORKSHOP.as_bytes());
    t.put(C1, chapter.as_bytes());
    t.put(M1, m1.as_bytes());
    t.put("chapters/c2/chapter.yaml", c2.as_bytes());
    put_module(&t, M2, "m2", &call("m2", "k"));
    let d = validate_dir(&t.0);
    assert!(d.iter().any(|d| d.code() == DiagCode::Limit), "{d:?}");
}
