//! ヒントの上限の規則（TASK-296、GUIDE-6、D-55）の試験。
//!
//! 上限は「現在の案内レベルで出せる数 n まで」で、3 段に固定しない。`hint_open.rs` が n=2 の Step だけで確かめる
//! のに対し、ここでは n が 3 でない Step（1・2・4・5・7 段）で、ちょうど n 段まで開け、n+1 段目が `NoHint` で
//! 拒否され世代が変わらず、3 段で打ち切らず、3 段であることも要求しないことを確かめる。
//! 試験は作業ツリーを書き換えない。試験では `unwrap` を使ってよい。

mod common;

use std::path::Path;

use common::property::check;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Stamp, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::{apply, Session, SessionState};
use proptest::prelude::*;
use serde_json::{json, Value};

/// 性質テストの件数（1,000 通り）。
const CASES: u32 = 1_000;

/// 段数ごとの Step。`main/k7` は水準で段数が変わる（A=7・B=4・C=2）。
const COUNTS: [(&str, usize); 5] = [
    ("main/k1", 1),
    ("main/k2", 2),
    ("main/k4", 4),
    ("main/k5", 5),
    ("main/k7", 7),
];

fn ir() -> Ir {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let mut hints = serde_json::Map::new();
    for i in 0..7 {
        let mut def = json!({"text": "t"});
        // 5 段目以降は A だけ、3・4 段目は A と B だけ（k7 の水準ごとの段数を 7・4・2 にする）。
        if i >= 4 {
            def["levels"] = json!(["A"]);
        } else if i >= 2 {
            def["levels"] = json!(["A", "B"]);
        }
        hints.insert(format!("h{i}"), def);
    }
    v["hints"] = Value::Object(hints);
    let node = |uid: &str, n: usize| {
        let ids: Vec<String> = (0..n).map(|i| format!("h{i}")).collect();
        let id = uid.trim_start_matches("main/");
        json!({
            "uid": uid, "scope": "main", "unit": "ch1",
            "step": {"id": id, "title": "t", "hints": ids}
        })
    };
    v["nodes"] = Value::Array(COUNTS.iter().map(|(u, n)| node(u, *n)).collect());
    serde_json::from_value(v).unwrap()
}

fn open() -> Op {
    Op::Control(Control::OpenHint)
}

fn at_level(ir: &Ir, uid: &str, level: GuidanceLevel) -> SessionState {
    let mut state = Session::entered_at(ir.clone(), uid).state().clone();
    state.run.guidance_level = level;
    state
}

fn opened(state: &SessionState, uid: &str) -> u32 {
    state
        .hints_opened
        .get(&StepUid(uid.to_string()))
        .copied()
        .unwrap_or(0)
}

#[test]
fn opens_exactly_n_for_counts_other_than_three() {
    let ir = ir();
    for (uid, _) in COUNTS {
        for l in GuidanceLevel::ALL {
            let mut state = at_level(&ir, uid, l);
            let n = u32::try_from(current_hints(&ir, &state.run).len()).unwrap();
            assert_ne!(n, 3, "{uid} {l:?}");
            let g = state.run.generation;
            for i in 1..=n {
                let (s, events) = apply(&ir, state, open());
                state = s;
                let want = Event::HintOpened {
                    at: Stamp {
                        generation: g,
                        step: StepUid(uid.to_string()),
                    },
                    opened: i,
                    total: n,
                };
                assert_eq!(events, vec![want], "{uid} {l:?} open {i}");
            }
            let run = state.run.clone();
            let (s, events) = apply(&ir, state, open());
            let want = Event::Refused {
                at: Stamp {
                    generation: g,
                    step: StepUid(uid.to_string()),
                },
                op: ControlKind::OpenHint,
                reason: RefuseReason::NoHint,
                rejection: None,
            };
            assert_eq!(events, vec![want], "{uid} {l:?} beyond n");
            assert_eq!(s.run, run);
            assert_eq!(opened(&s, uid), n);
        }
    }
}

#[test]
fn four_or_more_visible_hints_are_not_capped_at_three() {
    let ir = ir();
    for (uid, n) in [("main/k4", 4), ("main/k5", 5)] {
        let mut state = at_level(&ir, uid, GuidanceLevel::A);
        for i in 1..=n {
            let (s, events) = apply(&ir, state, open());
            state = s;
            assert!(
                matches!(events.as_slice(), [Event::HintOpened { opened, .. }] if *opened == u32::try_from(i).unwrap()),
                "{uid} open {i}: {events:?}"
            );
        }
        assert_eq!(opened(&state, uid), u32::try_from(n).unwrap());
    }
}

#[test]
fn fewer_than_three_hints_are_not_refused_for_being_short() {
    let ir = ir();
    for (uid, n) in [("main/k1", 1u32), ("main/k2", 2)] {
        let mut state = at_level(&ir, uid, GuidanceLevel::B);
        for i in 1..=n {
            let (s, events) = apply(&ir, state, open());
            state = s;
            assert!(
                matches!(events.as_slice(), [Event::HintOpened { opened, total, .. }] if *opened == i && *total == n),
                "{uid} open {i}: {events:?}"
            );
        }
    }
}

#[test]
fn limit_follows_level_and_count_never_decreases() {
    let ir = ir();
    let mut state = at_level(&ir, "main/k7", GuidanceLevel::A);
    for _ in 0..5 {
        state = apply(&ir, state, open()).0;
    }
    assert_eq!(opened(&state, "main/k7"), 5);
    state.run.guidance_level = GuidanceLevel::C;
    let (s, events) = apply(&ir, state, open());
    state = s;
    assert!(matches!(
        events.as_slice(),
        [Event::Refused {
            reason: RefuseReason::NoHint,
            ..
        }]
    ));
    assert_eq!(opened(&state, "main/k7"), 5);
    state.run.guidance_level = GuidanceLevel::A;
    for want in [6, 7] {
        let (s, events) = apply(&ir, state, open());
        state = s;
        assert!(
            matches!(events.as_slice(), [Event::HintOpened { opened, total: 7, .. }] if *opened == want),
            "{events:?}"
        );
    }
    assert_eq!(opened(&state, "main/k7"), 7);
}

/// 水準ごとの宣言（`None` は全水準、`Some` は水準の部分列）。
fn decl_strategy() -> impl Strategy<Value = Option<Vec<usize>>> {
    prop_oneof![
        Just(None),
        proptest::collection::vec(0usize..3, 0..=3).prop_map(Some),
    ]
}

#[test]
fn property_limit_is_visible_count_not_three() {
    // 4 本以上の全水準のヒントを必ず足して、出せる数が 3 を超える場合を毎回含める。
    check(
        "hint-limit-property-count",
        CASES,
        (
            proptest::collection::vec(decl_strategy(), 0..=8),
            4usize..=6,
            0usize..3,
            0usize..=16,
        ),
        |(decls, always, li, k)| {
            let mut hints = serde_json::Map::new();
            let mut ids = Vec::new();
            let mut expected = 0usize;
            for (i, d) in decls.iter().enumerate() {
                let id = format!("h{i}");
                let mut def = json!({"text": "t"});
                match d {
                    None => expected += 1,
                    Some(ls) => {
                        def["levels"] = Value::from(
                            ls.iter()
                                .map(|&x| GuidanceLevel::ALL[x].as_str())
                                .collect::<Vec<_>>(),
                        );
                        if ls.is_empty() || ls.contains(&li) {
                            expected += 1;
                        }
                    }
                }
                hints.insert(id.clone(), def);
                ids.push(id);
            }
            for j in 0..always {
                let id = format!("always{j}");
                hints.insert(id.clone(), json!({"text": "t"}));
                ids.push(id);
                expected += 1;
            }
            let mut v = {
                let p = Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../contract/tests/data/guidance_1/hints-ir.json");
                serde_json::from_slice::<Value>(&std::fs::read(p).unwrap()).unwrap()
            };
            v["hints"] = Value::Object(hints);
            v["nodes"][0]["step"]["hints"] = json!(ids);
            let ir: Ir = serde_json::from_value(v).unwrap();
            prop_assert!(expected >= 4);
            let mut state = at_level(&ir, "main/s1", GuidanceLevel::ALL[li]);
            let mut ok = 0usize;
            for _ in 0..k {
                let (s, events) = apply(&ir, state, open());
                state = s;
                if let [Event::HintOpened { total, .. }] = events.as_slice() {
                    prop_assert_eq!(usize::try_from(*total).unwrap(), expected);
                    ok += 1;
                }
            }
            prop_assert_eq!(ok, k.min(expected));
            Ok(())
        },
    );
}
