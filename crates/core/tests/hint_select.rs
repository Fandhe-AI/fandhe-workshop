//! 案内レベル別のヒントの出し分け（TASK-293、GUIDE-2）の試験。
//!
//! 規則の正は契約の `visible_hints` で、本試験は実行の状態（現在の Step と案内レベル）から引いた一覧が
//! 水準ごとに宣言どおりに変わり、規則と食い違わないことを確かめる。試験は作業ツリーを書き換えない。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::collections::BTreeMap;
use std::path::Path;

use common::property::{check, DEFAULT_CASES};
use fandhe_workshop_contract::guidance::{visible_hints, GuidanceLevel};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::hint_select::{current_hints, hints_at};
use fandhe_workshop_core::session::{RunState, Session};
use proptest::prelude::*;
use serde_json::{json, Value};

fn ir_value() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn ir() -> Ir {
    serde_json::from_value(ir_value()).unwrap()
}

fn run_at(uid: &str, level: GuidanceLevel) -> RunState {
    RunState {
        current: Some(StepUid(uid.to_string())),
        guidance_level: level,
        ..RunState::default()
    }
}

#[test]
fn the_list_changes_with_the_level() {
    let ir = ir();
    let at = |l| current_hints(&ir, &run_at("main/s1", l));
    assert_eq!(at(GuidanceLevel::A), vec!["h-all", "h-a"]);
    assert_eq!(at(GuidanceLevel::B), vec!["h-all", "h-bc"]);
    assert_eq!(at(GuidanceLevel::C), vec!["h-all", "h-bc"]);
    assert_ne!(at(GuidanceLevel::A), at(GuidanceLevel::B));
    // 未定義（h-missing）と読めない定義（h-bad）は、どの水準でも出ない。
    for l in GuidanceLevel::ALL {
        let v = at(l);
        assert!(!v.contains(&"h-missing") && !v.contains(&"h-bad"), "{v:?}");
    }
}

#[test]
fn the_list_follows_the_session_level() {
    let ir = ir();
    for l in GuidanceLevel::ALL {
        let mut state = Session::entered_at(ir.clone(), "main/s1").state().clone();
        state.run.guidance_level = l;
        let s = Session::from_state(ir.clone(), state);
        assert_eq!(
            current_hints(s.ir(), &s.state().run),
            current_hints(&ir, &run_at("main/s1", l))
        );
    }
}

#[test]
fn before_start_and_unknown_step_are_empty() {
    let ir = ir();
    for l in GuidanceLevel::ALL {
        let none = RunState {
            guidance_level: l,
            ..RunState::default()
        };
        assert!(current_hints(&ir, &none).is_empty());
        assert!(current_hints(&ir, &run_at("main/nope", l)).is_empty());
        assert!(hints_at(&ir, &StepUid("main/nope".to_string()), l).is_empty());
    }
}

#[test]
fn the_list_equals_the_contract_rule() {
    let ir = ir();
    for l in GuidanceLevel::ALL {
        let node = &ir.nodes[0];
        assert_eq!(
            hints_at(&ir, &StepUid(node.uid.clone()), l),
            visible_hints(&ir, &node.step, l)
        );
    }
}

/// 宣言の形: 省略・空・A/B/C の空でない部分集合。
fn levels_strategy() -> impl Strategy<Value = Option<Vec<usize>>> {
    prop_oneof![
        Just(None),
        Just(Some(Vec::new())),
        proptest::sample::subsequence(vec![0usize, 1, 2], 1..=3).prop_map(Some),
    ]
}

#[test]
fn property_list_matches_independent_expectation() {
    check(
        "hint-select-property",
        DEFAULT_CASES,
        proptest::collection::vec(levels_strategy(), 0..=5),
        |decls| {
            let mut hints = BTreeMap::new();
            let mut ids = Vec::new();
            for (i, d) in decls.iter().enumerate() {
                let id = format!("h{i}");
                let mut def = json!({"text": "t"});
                if let Some(ls) = d {
                    def["levels"] = Value::from(
                        ls.iter()
                            .map(|&k| GuidanceLevel::ALL[k].as_str())
                            .collect::<Vec<_>>(),
                    );
                }
                hints.insert(id.clone(), def);
                ids.push(id);
            }
            let mut v = ir_value();
            v["hints"] = json!(hints);
            v["nodes"][0]["step"]["hints"] = json!(ids);
            let ir: Ir = serde_json::from_value(v).unwrap();
            for (li, l) in GuidanceLevel::ALL.into_iter().enumerate() {
                let want: Vec<String> = decls
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| match d {
                        None => true,
                        Some(ls) => ls.is_empty() || ls.contains(&li),
                    })
                    .map(|(i, _)| format!("h{i}"))
                    .collect();
                let got: Vec<String> = current_hints(&ir, &run_at("main/s1", l))
                    .into_iter()
                    .map(str::to_string)
                    .collect();
                prop_assert_eq!(got, want);
            }
            Ok(())
        },
    );
}
