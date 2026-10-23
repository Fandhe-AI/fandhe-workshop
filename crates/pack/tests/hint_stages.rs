//! ヒントの段の構造の確認（TASK-298、GUIDE-8・D-55）の試験。
//!
//! 確かめること: 段数・答えの段・Step ごとの例外を教材テストが宣言として渡せること、宣言と違う Step が報告されること、
//! エンジン（`validate`・`compile`・`visible_hints`）が段の数を固定しないこと、報告に値が写らないこと。
//! 確かめないこと: 段の中身の意味と教育上の適切さ。呼び出し元は将来の `fandhe-pack test`（TASK-404）。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;

use common::{Tmp, SECRET, WORKSHOP};
use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::hint_stages::{
    check_hint_stages, AnswerText, HintStageFinding as F, HintStageRule, HintStageRuleError,
};
use fandhe_workshop_pack::validate::validate_dir;

/// 章 c1 に Step を並べた教材。`steps` は (Step ID, ヒント ID の列)、`hints` は (ヒント ID, 定義の YAML)。
fn pack(steps: &[(&str, &[&str])], hints: &[(&str, &str)]) -> Tmp {
    let t = Tmp::new("g8");
    let mut ws = String::from(WORKSHOP);
    ws.push_str("hints:\n");
    for (id, def) in hints {
        ws.push_str(&format!("  {id}: {def}\n"));
    }
    t.put("workshop.yaml", ws.as_bytes());
    let starts: Vec<String> = steps
        .iter()
        .map(|(id, _)| format!("{{id: s-{id}, node: {id}}}"))
        .collect();
    let mut ch = format!(
        "id: c1\ntitle: C\nstarts: [{}]\nsteps:\n",
        starts.join(", ")
    );
    for (id, hs) in steps {
        ch.push_str(&format!(
            "  - {{id: {id}, title: T, hints: [{}], final: true}}\n",
            hs.join(", ")
        ));
    }
    t.put("chapters/c1/chapter.yaml", ch.as_bytes());
    t
}

fn ir(t: &Tmp) -> Ir {
    load_ir(&compile_dir(&t.0).expect("compile")).expect("load_ir")
}

const H: &[(&str, &str)] = &[
    ("h1", "{text: 'place'}"),
    ("h2", "{text: 'tool'}"),
    ("h3", "{text: 'run make now'}"),
    ("h4", "{text: 'extra'}"),
];

fn rule(stages: usize) -> HintStageRule {
    HintStageRule {
        stages,
        answer_stage: None,
        per_step: BTreeMap::new(),
        answer_text: None,
    }
}

fn answers(field: &str, pairs: &[(&str, &str)]) -> Option<AnswerText> {
    Some(AnswerText {
        field: field.into(),
        answers: pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect(),
    })
}

fn u(s: &str) -> String {
    format!("main/{s}")
}

#[test]
fn guide8_three_stage_steps_pass_the_declared_rule() {
    let t = pack(&[("a", &["h1", "h2", "h3"]), ("b", &["h1", "h2", "h3"])], H);
    let mut r = rule(3);
    r.answer_stage = Some(3);
    assert_eq!(check_hint_stages(&ir(&t), &r), Ok(vec![]));
}

#[test]
fn guide8_steps_not_matching_the_declared_count_are_reported() {
    let t = pack(
        &[
            ("one", &["h1"]),
            ("two", &["h1", "h2"]),
            ("three", &["h1", "h2", "h3"]),
            ("four", &["h1", "h2", "h3", "h4"]),
            ("none", &[]),
        ],
        H,
    );
    let got = check_hint_stages(&ir(&t), &rule(3)).unwrap();
    let want: Vec<F> = [("one", 1), ("two", 2), ("four", 4)]
        .iter()
        .map(|(s, n)| F::StageCount {
            uid: u(s),
            found: *n,
            declared: 3,
        })
        .collect();
    assert_eq!(got, want);
}

#[test]
fn guide8_engine_does_not_fix_three() {
    let t = pack(
        &[("two", &["h1", "h2"]), ("four", &["h1", "h2", "h3", "h4"])],
        H,
    );
    assert!(validate_dir(&t.0).is_empty(), "{:?}", validate_dir(&t.0));
    let ir = ir(&t);
    let n = ir.nodes.iter().find(|n| n.step.id == "four").unwrap();
    assert_eq!(visible_hints(&ir, &n.step, GuidanceLevel::A).len(), 4);
    for (stages, ok) in [(2, "two"), (3, ""), (4, "four")] {
        let got = check_hint_stages(&ir, &rule(stages)).unwrap();
        let reported = |s: &str| {
            got.iter()
                .any(|f| matches!(f, F::StageCount { uid, .. } if *uid == u(s)))
        };
        for s in ["two", "four"] {
            assert_eq!(reported(s), s != ok, "stages={stages} step={s}");
        }
    }
}

#[test]
fn guide8_per_step_exception_is_honored() {
    let t = pack(&[("a", &["h1", "h2", "h3"]), ("b", &["h1", "h2"])], H);
    let ir = ir(&t);
    assert_eq!(check_hint_stages(&ir, &rule(3)).unwrap().len(), 1);
    let mut r = rule(3);
    r.per_step.insert(u("b"), 2);
    assert_eq!(check_hint_stages(&ir, &r), Ok(vec![]));
    r.per_step.insert(u("zz"), 2);
    assert_eq!(
        check_hint_stages(&ir, &r).unwrap(),
        [F::UnknownStep { uid: u("zz") }]
    );
}

#[test]
fn guide8_answer_stage_must_resolve() {
    let t = pack(&[("b", &["h1", "h2", "gone"])], H);
    // 未定義の ID は段に数えない（b は 2 段のまま）。
    let ir = ir(&t);
    assert_eq!(check_hint_stages(&ir, &rule(2)), Ok(vec![]));
    let got = check_hint_stages(&ir, &rule(3)).unwrap();
    assert_eq!(
        got,
        [F::StageCount {
            uid: u("b"),
            found: 2,
            declared: 3
        }]
    );
    let mut r = rule(3);
    r.answer_stage = Some(3);
    let got = check_hint_stages(&ir, &r).unwrap();
    assert!(got.contains(&F::AnswerStageUnresolved {
        uid: u("b"),
        stage: 3
    }));
}

#[test]
fn guide8_answer_text_contains_answer() {
    for field in ["text", "body_text"] {
        let hints: Vec<(&str, String)> = vec![
            ("h1", format!("{{{field}: place}}")),
            ("h2", format!("{{{field}: tool}}")),
            ("h3", format!("{{{field}: 'run make now'}}")),
            ("h4", format!("{{{field}: 'run nothing'}}")),
            ("h5", "{other: 'run make now'}".to_string()),
            ("h6", format!("{{{field}: 7}}")),
        ];
        let hs: Vec<(&str, &str)> = hints.iter().map(|(a, b)| (*a, b.as_str())).collect();
        let t = pack(
            &[
                ("ok", &["h1", "h2", "h3"]),
                ("miss", &["h1", "h2", "h4"]),
                ("nofield", &["h1", "h2", "h5"]),
                ("notstr", &["h1", "h2", "h6"]),
                ("noans", &["h1", "h2", "h3"]),
                ("empty", &["h1", "h2", "h3"]),
            ],
            &hs,
        );
        let mut r = rule(3);
        r.answer_stage = Some(3);
        r.answer_text = answers(
            field,
            &[
                ("main/ok", "make"),
                ("main/miss", "make"),
                ("main/nofield", "make"),
                ("main/notstr", "make"),
                ("main/empty", ""),
            ],
        );
        assert_eq!(
            check_hint_stages(&ir(&t), &r).unwrap(),
            [
                F::AnswerMissing { uid: u("miss") },
                F::AnswerTextUnreadable { uid: u("nofield") },
                F::AnswerTextUnreadable { uid: u("notstr") },
                F::AnswerNotGiven { uid: u("noans") },
                F::AnswerNotGiven { uid: u("empty") },
            ],
            "field={field}"
        );
    }
}

#[test]
fn guide8_answer_stage_position_is_declared_by_the_pack() {
    let h: &[(&str, &str)] = &[("h1", "{text: place}"), ("h2", "{text: 'use make'}")];
    let t = pack(&[("a", &["h1", "h2"])], h);
    let mut r = rule(2);
    r.answer_stage = Some(2);
    r.answer_text = answers("text", &[("main/a", "make")]);
    assert_eq!(check_hint_stages(&ir(&t), &r), Ok(vec![]));
    r.answer_stage = Some(1);
    assert_eq!(
        check_hint_stages(&ir(&t), &r).unwrap(),
        [F::AnswerMissing { uid: u("a") }]
    );
}

#[test]
fn guide8_module_instances_are_checked_per_uid() {
    let t = Tmp::new("g8-mod");
    let mut ws = String::from(WORKSHOP);
    ws.push_str("hints:\n  h1: {text: a}\n  h2: {text: b}\n  h3: {text: c}\n");
    t.put("workshop.yaml", ws.as_bytes());
    t.put(
        "chapters/c1/chapter.yaml",
        b"id: c1\ntitle: C\nstarts: [{id: s1, node: u1}]\nsteps:\n  - {id: u1, title: U, use: {module: m, instance: i1, args: {x: h3}}, next: [{to: u2}]}\n  - {id: u2, title: U, use: {module: m, instance: i2, args: {x: gone}}, final: true}\n",
    );
    t.put(
        "modules/m/module.yaml",
        b"id: m\ntitle: M\nparams: {x: h3}\nsteps:\n  - {id: a, title: T, hints: [h1, h2, \"${x}\"], final: true}\n",
    );
    let got = check_hint_stages(&ir(&t), &rule(3)).unwrap();
    assert_eq!(got.len(), 1, "{got:?}");
    match &got[0] {
        F::StageCount {
            uid,
            found,
            declared,
        } => {
            assert!(uid.contains("i2"), "{uid}");
            assert_eq!((*found, *declared), (2, 3));
            // 実体の `uid`（`<モジュール>@<実体>` を含む）は表示でも伏せずに写す。
            assert!(got[0].to_string().contains(uid.as_str()), "{}", got[0]);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn guide8_step_with_only_undefined_hints_is_checked() {
    // 参照を持つが 1 つも解決できない Step は対象外にせず、段数 0 として報告する（`hints` が空の Step は対象外のまま）。
    let t = pack(
        &[("a", &["gone"]), ("n", &[]), ("b", &["h1", "h2", "h3"])],
        H,
    );
    let mut r = rule(3);
    r.answer_stage = Some(3);
    assert_eq!(
        check_hint_stages(&ir(&t), &r).unwrap(),
        [
            F::StageCount {
                uid: u("a"),
                found: 0,
                declared: 3
            },
            F::AnswerStageUnresolved {
                uid: u("a"),
                stage: 3
            }
        ]
    );
}

#[test]
fn guide8_declaration_errors_are_not_empty_reports() {
    let t = pack(&[("a", &["h1"])], H);
    let ir = ir(&t);
    assert_eq!(
        check_hint_stages(&ir, &rule(0)),
        Err(HintStageRuleError::ZeroStages)
    );
    let mut r = rule(3);
    r.answer_stage = Some(4);
    assert_eq!(
        check_hint_stages(&ir, &r),
        Err(HintStageRuleError::AnswerStageOutOfRange)
    );
}

#[test]
fn guide8_findings_do_not_copy_values() {
    let def = format!("{{text: '{SECRET}'}}");
    let hs: Vec<(&str, &str)> = vec![("h1", &def), ("h2", &def), ("h3", &def)];
    let t = pack(&[("a", &["h1", "h2", "h3"]), ("b", &["h1"])], &hs);
    let mut r = rule(3);
    r.answer_stage = Some(3);
    r.answer_text = answers("text", &[("main/a", "absent-answer"), ("main/b", SECRET)]);
    let got = check_hint_stages(&ir(&t), &r).unwrap();
    assert!(!got.is_empty());
    for f in &got {
        let s = format!("{f} {f:?}");
        assert!(!s.contains(SECRET), "{s}");
    }
}
