//! 条件の語彙 11 語の網羅パック（`testdata/condition-vocab-pack`。TASK-42 2/2、PACK-8）の結合試験。
//!
//! 受入基準は 3 つ。(a) ST-1.4・CH-2・CH-7・CH-13 の Step を表すパックが、診断 0 件で検証と変換を通る。
//! (b) パックに式やスクリプトの欄が無い。(c) 語彙の 11 語がすべてパックの中で使われ、`param`・`clarifying` を
//! 含む条件が core の評価器で期待どおりに評価できる。CLI の `validate` への結線はまだ無いため、ライブラリの
//! `validate_dir` と `compile_dir` の両方を見る。試験名の `pack8_` で 04 の ID を追える。

mod common;

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use common::testdata;

use fandhe_workshop_contract::condition::Condition;
use fandhe_workshop_contract::ir::{Ir, Node};
use fandhe_workshop_core::addr::Root;
use fandhe_workshop_core::condition::VocabConditions;
use fandhe_workshop_core::effect::ConditionEval;
use fandhe_workshop_core::load_ir;
use fandhe_workshop_core::session::{RunState, Slice};
use fandhe_workshop_pack::compile::compile_dir;
use fandhe_workshop_pack::validate::validate_dir;
use serde_json::{json, Value};

const PACK: &str = "condition-vocab-pack";

/// 語彙の 11 語。
const WORDS: [&str; 11] = [
    "step",
    "flag",
    "world_has",
    "world_attr",
    "count",
    "chose",
    "all",
    "any",
    "not",
    "param",
    "clarifying",
];

fn ir() -> Ir {
    let bytes = compile_dir(&testdata(PACK)).expect("compile");
    load_ir(&bytes).expect("load_ir")
}

fn node<'a>(ir: &'a Ir, uid: &str) -> &'a Node {
    ir.nodes
        .iter()
        .find(|n| n.uid == uid)
        .unwrap_or_else(|| panic!("node {uid} not found"))
}

#[test]
fn pack8_vocab_pack_validates_and_compiles() {
    let dir = testdata(PACK);
    let d = validate_dir(&dir);
    assert!(d.is_empty(), "{d:?}");
    // validate は通るが compile は失敗する状態がありうるため、両方を見る。
    let bytes = compile_dir(&dir).expect("compile");
    load_ir(&bytes).expect("load_ir");
}

#[test]
fn pack8_vocab_pack_has_every_step_of_the_four_ranges() {
    let ir = ir();
    let have: BTreeSet<&str> = ir.nodes.iter().map(|n| n.uid.as_str()).collect();
    let mut want: Vec<String> = vec!["main/st-1-4".into()];
    want.extend((1..=8).map(|k| format!("main/st-2-{k}")));
    want.extend(
        [
            "main/st-7-1",
            "main/u-impl",
            "main/st-13-1",
            "main/st-13-2",
            "main/st-13-2x",
            "main/st-13-3",
        ]
        .map(String::from),
    );
    for round in ["round1", "round2"] {
        for s in ["st-7-2", "st-7-3", "u-merge"] {
            want.push(format!("impl-run@{round}/{s}"));
        }
        want.push(format!("impl-run@{round}/merge-gate@gate/st-7-4"));
    }
    for w in &want {
        assert!(have.contains(w.as_str()), "missing {w}: {have:?}");
    }
}

/// パックの全 YAML を `Value` にして返す。
fn yaml_files() -> Vec<(String, yaml_serde::Value)> {
    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        for e in fs::read_dir(dir).expect("read_dir") {
            let p = e.expect("entry").path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "yaml") {
                out.push(p);
            }
        }
    }
    let mut files = Vec::new();
    walk(&testdata(PACK), &mut files);
    files.sort();
    assert!(files.len() >= 7, "{files:?}");
    files
        .into_iter()
        .map(|p| {
            let text = fs::read_to_string(&p).expect("read");
            let v: yaml_serde::Value = yaml_serde::from_str(&text).expect("yaml");
            (p.display().to_string(), v)
        })
        .collect()
}

/// 条件を持つ欄（`when`・`requires`）の値を集める。
fn collect_conditions(v: &yaml_serde::Value, out: &mut Vec<Value>) {
    match v {
        yaml_serde::Value::Mapping(m) => {
            for (k, val) in m {
                let key = k.as_str().unwrap_or("");
                if key == "when" || key == "requires" {
                    out.push(serde_json::to_value(val).expect("json"));
                } else {
                    collect_conditions(val, out);
                }
            }
        }
        yaml_serde::Value::Sequence(s) => s.iter().for_each(|x| collect_conditions(x, out)),
        _ => {}
    }
}

/// 条件の木に出る語を集める。
fn words_of(c: &Condition, out: &mut BTreeSet<&'static str>) {
    match c {
        Condition::Step(_) => out.insert("step"),
        Condition::Flag(_) => out.insert("flag"),
        Condition::WorldHas(_) => out.insert("world_has"),
        Condition::WorldAttr(_) => out.insert("world_attr"),
        Condition::Count(_) => out.insert("count"),
        Condition::Chose(_) => out.insert("chose"),
        Condition::Param(_) => out.insert("param"),
        Condition::Clarifying(_) => out.insert("clarifying"),
        Condition::All(l) => {
            l.iter().for_each(|x| words_of(x, out));
            out.insert("all")
        }
        Condition::Any(l) => {
            l.iter().for_each(|x| words_of(x, out));
            out.insert("any")
        }
        Condition::Not(x) => {
            words_of(x, out);
            out.insert("not")
        }
        _ => panic!("unknown condition word"),
    };
}

#[test]
fn pack8_vocab_pack_conditions_cover_all_eleven_words() {
    let mut values = Vec::new();
    for (_, y) in yaml_files() {
        collect_conditions(&y, &mut values);
    }
    assert!(!values.is_empty());
    let mut used = BTreeSet::new();
    for v in &values {
        let c = Condition::from_value(v).unwrap_or_else(|e| panic!("{v}: {e:?}"));
        words_of(&c, &mut used);
    }
    let want: BTreeSet<&str> = WORDS.into_iter().collect();
    assert_eq!(used, want);
}

#[test]
fn pack8_vocab_pack_has_no_expression_or_script_fields() {
    const BANNED: [&str; 10] = [
        "expr",
        "expression",
        "script",
        "code",
        "eval",
        "js",
        "javascript",
        "lua",
        "formula",
        "lambda",
    ];
    fn keys(v: &yaml_serde::Value, bad: &mut Vec<String>) {
        match v {
            yaml_serde::Value::Mapping(m) => {
                for (k, val) in m {
                    if let Some(s) = k.as_str() {
                        if BANNED.contains(&s) {
                            bad.push(s.to_string());
                        }
                    }
                    keys(val, bad);
                }
            }
            yaml_serde::Value::Sequence(s) => s.iter().for_each(|x| keys(x, bad)),
            _ => {}
        }
    }
    for (name, y) in yaml_files() {
        let mut bad = Vec::new();
        keys(&y, &mut bad);
        assert!(bad.is_empty(), "{name}: {bad:?}");
    }
}

#[test]
fn pack8_param_branch_differs_between_round1_and_round2() {
    let ir = ir();
    let ev = VocabConditions::new(&ir.instances);
    let run = RunState::default();
    for (round, want) in [("round1", true), ("round2", false)] {
        let uid = format!("impl-run@{round}/st-7-2");
        let when = node(&ir, &uid).step.next[0]
            .when
            .clone()
            .expect("param branch");
        let root = Root(format!("impl-run@{round}"));
        assert_eq!(ev.eval(&when, &run, &root), Some(want), "{round}");
    }
}

#[test]
fn pack8_clarifying_cancel_rule_holds_only_while_waiting() {
    let ir = ir();
    let rule = node(&ir, "main/st-1-4")
        .step
        .rules
        .iter()
        .find(|r| r["id"] == "r-cancel")
        .expect("r-cancel");
    let when = rule["when"].clone();
    assert_eq!(when, json!({"clarifying": "cl-new-idea-name"}));
    let ev = VocabConditions::new(&ir.instances);
    let main = Root("main".into());
    let mut run = RunState::default();
    run.slices.insert(main.clone(), Slice::default());
    assert_eq!(ev.eval(&when, &run, &main), Some(false));
    run.slices.get_mut(&main).unwrap().clarifying = Some("cl-new-idea-name".into());
    assert_eq!(ev.eval(&when, &run, &main), Some(true));
    run.slices.get_mut(&main).unwrap().clarifying = Some("cl-other".into());
    assert_eq!(ev.eval(&when, &run, &main), Some(false));
    // 別の根の待ちは見ない。
    let other = Root("impl-run@round1".into());
    run.slices.insert(
        other.clone(),
        Slice {
            clarifying: Some("cl-new-idea-name".into()),
            ..Slice::default()
        },
    );
    run.slices.get_mut(&main).unwrap().clarifying = None;
    assert_eq!(ev.eval(&when, &run, &main), Some(false));
    assert_eq!(ev.eval(&when, &run, &other), Some(true));
}
