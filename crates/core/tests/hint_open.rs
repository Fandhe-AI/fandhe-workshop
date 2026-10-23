//! ヒントを開く操作と上限での拒否（TASK-295、GUIDE-17）の試験。
//!
//! n 段まで 1 段ずつ開け、超えると `NoHint` で拒否され、現在の Step が無いときも拒否されること、
//! 成功・拒否のどちらでも世代が変わらないことを確かめる。試験は作業ツリーを書き換えない。
//! 試験では `unwrap` を使ってよい。

mod common;

use std::path::Path;

use common::property::check;
use fandhe_workshop_contract::guidance::GuidanceLevel;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, Stamp, StepUid};
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::{apply, Session, SessionState};
use proptest::prelude::*;
use serde_json::{json, Value};

/// 性質テストの件数（1,000 通り）。
const CASES: u32 = 1_000;

fn ir_value() -> Value {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../contract/tests/data/guidance_1/hints-ir.json");
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn ir() -> Ir {
    serde_json::from_value(ir_value()).unwrap()
}

/// `main/s1`（水準 A・B・C のどれでも 2 本）に、ヒントの無い `main/s2` と、
/// 水準 A だけ 3 本・B と C は 1 本の `main/s3` を足した IR。
fn multi_ir() -> Ir {
    let mut v = ir_value();
    v["hints"]["h-a2"] = json!({"levels": ["A"], "text": "A2"});
    v["nodes"][0]["step"]["hints"] = json!(["h-all", "h-a", "h-bc"]);
    let n = |uid: &str, id: &str, hints: Value| {
        json!({
            "uid": uid, "scope": "main", "unit": "ch1",
            "step": {"id": id, "title": "t", "hints": hints}
        })
    };
    v["nodes"] = json!([
        v["nodes"][0].clone(),
        n("main/s2", "s2", json!([])),
        n("main/s3", "s3", json!(["h-all", "h-a", "h-a2"])),
    ]);
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

fn refused(generation: Generation, step: &str) -> Event {
    Event::Refused {
        at: Stamp {
            generation,
            step: StepUid(step.to_string()),
        },
        op: ControlKind::OpenHint,
        reason: RefuseReason::NoHint,
        rejection: None,
    }
}

/// 戻りのイベント列がログの末尾と一致することを確かめて、新しい状態とイベント列を返す。
fn step(ir: &Ir, state: SessionState, op: Op) -> (SessionState, Vec<Event>) {
    let before = state.log.len();
    let (next, events) = apply(ir, state, op);
    assert_eq!(&next.log[before..], events.as_slice());
    (next, events)
}

#[test]
fn opens_one_by_one_up_to_n() {
    let ir = ir();
    for l in GuidanceLevel::ALL {
        let mut state = at_level(&ir, "main/s1", l);
        let n = current_hints(&ir, &state.run).len();
        assert_eq!(n, 2);
        let g = state.run.generation;
        for i in 1..=n {
            let (s, events) = step(&ir, state, open());
            state = s;
            let want = Event::HintOpened {
                at: Stamp {
                    generation: g,
                    step: StepUid("main/s1".to_string()),
                },
                opened: u32::try_from(i).unwrap(),
                total: u32::try_from(n).unwrap(),
            };
            assert_eq!(events, vec![want]);
            assert_eq!(
                state.hints_opened[&StepUid("main/s1".to_string())],
                u32::try_from(i).unwrap()
            );
            assert_eq!(state.run.generation, g);
        }
    }
}

#[test]
fn refuses_beyond_n_without_changing_anything_but_the_log() {
    let ir = ir();
    let mut state = at_level(&ir, "main/s1", GuidanceLevel::A);
    for _ in 0..2 {
        state = step(&ir, state, open()).0;
    }
    let before = state.clone();
    for _ in 0..3 {
        let (s, events) = step(&ir, state, open());
        state = s;
        assert_eq!(events, vec![refused(before.run.generation, "main/s1")]);
        assert_eq!(state.run, before.run);
        assert_eq!(state.hints_opened, before.hints_opened);
        assert_eq!(state.checkpoints, before.checkpoints);
        assert_eq!(state.drafts, before.drafts);
        assert_eq!(state.entry, before.entry);
    }
    assert_eq!(state.log.len(), before.log.len() + 3);
}

#[test]
fn refuses_without_current_step() {
    let ir = ir();
    let state = Session::new(ir.clone()).state().clone();
    assert!(state.run.current.is_none());
    let before = state.clone();
    let (next, events) = step(&ir, state, open());
    assert_eq!(events, vec![refused(before.run.generation, "")]);
    assert_eq!(next.run, before.run);
    assert!(next.hints_opened.is_empty());
}

#[test]
fn refuses_on_step_without_visible_hints() {
    let ir = multi_ir();
    let state = at_level(&ir, "main/s2", GuidanceLevel::A);
    let (next, events) = step(&ir, state, open());
    assert_eq!(events, vec![refused(next.run.generation, "main/s2")]);
    assert!(next.hints_opened.is_empty());
    // IR に無い Step も件数 0 で拒否される。
    let mut state = at_level(&ir, "main/s1", GuidanceLevel::A);
    state.run.current = Some(StepUid("main/nope".to_string()));
    let (next, events) = step(&ir, state, open());
    assert_eq!(events, vec![refused(next.run.generation, "main/nope")]);
    assert!(next.hints_opened.is_empty());
}

#[test]
fn lowering_the_level_refuses_and_keeps_the_count() {
    let ir = multi_ir();
    let mut state = at_level(&ir, "main/s3", GuidanceLevel::A);
    for _ in 0..3 {
        state = step(&ir, state, open()).0;
    }
    let key = StepUid("main/s3".to_string());
    assert_eq!(state.hints_opened[&key], 3);
    state.run.guidance_level = GuidanceLevel::B;
    assert_eq!(current_hints(&ir, &state.run).len(), 1);
    let (next, events) = step(&ir, state, open());
    assert_eq!(events, vec![refused(next.run.generation, "main/s3")]);
    assert_eq!(next.hints_opened[&key], 3);
}

#[test]
fn the_limit_follows_the_current_level() {
    let ir = multi_ir();
    for (l, n) in [
        (GuidanceLevel::A, 3u32),
        (GuidanceLevel::B, 1),
        (GuidanceLevel::C, 1),
    ] {
        let mut state = at_level(&ir, "main/s3", l);
        for i in 1..=n {
            let (s, events) = step(&ir, state, open());
            state = s;
            assert!(matches!(
                events[0],
                Event::HintOpened { opened, total, .. } if opened == i && total == n
            ));
        }
        let (_, events) = step(&ir, state, open());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], Event::Refused { .. }));
    }
}

/// 水準 `li` に出せる数を、`levels` の宣言から独立に数える（None は全水準、空は全水準）。
fn expected_count(decls: &[Option<Vec<usize>>], li: usize) -> usize {
    decls
        .iter()
        .filter(|d| match d {
            None => true,
            Some(ls) => ls.is_empty() || ls.contains(&li),
        })
        .count()
}

fn levels_strategy() -> impl Strategy<Value = Option<Vec<usize>>> {
    prop_oneof![
        Just(None),
        Just(Some(Vec::new())),
        proptest::sample::subsequence(vec![0usize, 1, 2], 1..=3).prop_map(Some),
    ]
}

#[test]
fn property_open_up_to_visible_count() {
    check(
        "hint-open-property-count",
        CASES,
        (
            proptest::collection::vec(levels_strategy(), 0..=5),
            0usize..3,
            0usize..=8,
        ),
        |(decls, li, k)| {
            let mut hints = serde_json::Map::new();
            let mut ids = Vec::new();
            for (i, d) in decls.iter().enumerate() {
                let id = format!("h{i}");
                let mut def = json!({"text": "t"});
                if let Some(ls) = d {
                    def["levels"] = Value::from(
                        ls.iter()
                            .map(|&x| GuidanceLevel::ALL[x].as_str())
                            .collect::<Vec<_>>(),
                    );
                }
                hints.insert(id.clone(), def);
                ids.push(id);
            }
            let mut v = ir_value();
            v["hints"] = Value::Object(hints);
            v["nodes"][0]["step"]["hints"] = json!(ids);
            let ir: Ir = serde_json::from_value(v).unwrap();
            let n = expected_count(&decls, li);
            let mut state = at_level(&ir, "main/s1", GuidanceLevel::ALL[li]);
            let run = state.run.clone();
            for i in 0..k {
                let (s, events) = apply(&ir, state, open());
                state = s;
                prop_assert_eq!(events.len(), 1);
                if i < n {
                    let want = Event::HintOpened {
                        at: Stamp {
                            generation: run.generation,
                            step: StepUid("main/s1".to_string()),
                        },
                        opened: u32::try_from(i + 1).unwrap(),
                        total: u32::try_from(n).unwrap(),
                    };
                    prop_assert_eq!(&events[0], &want);
                } else {
                    prop_assert_eq!(&events[0], &refused(run.generation, "main/s1"));
                }
                prop_assert_eq!(&state.run, &run);
            }
            let got = state
                .hints_opened
                .get(&StepUid("main/s1".to_string()))
                .copied();
            let m = k.min(n);
            let want = if m == 0 {
                None
            } else {
                Some(u32::try_from(m).unwrap())
            };
            prop_assert_eq!(got, want);
            Ok(())
        },
    );
}

#[derive(Debug, Clone)]
enum Act {
    Open,
    GoTo(usize),
    Level(usize),
}

fn act_strategy() -> impl Strategy<Value = Act> {
    prop_oneof![
        4 => Just(Act::Open),
        1 => (0usize..4).prop_map(Act::GoTo),
        1 => (0usize..3).prop_map(Act::Level),
    ]
}

#[test]
fn property_mixed_sequences_keep_the_rules() {
    check(
        "hint-open-property-mixed",
        CASES,
        (
            any::<bool>(),
            proptest::collection::vec(act_strategy(), 0..=30),
        ),
        |(started, acts)| {
            let ir = multi_ir();
            let mut state = if started {
                at_level(&ir, "main/s1", GuidanceLevel::A)
            } else {
                Session::new(ir.clone()).state().clone()
            };
            let uids = ["main/s1", "main/s2", "main/s3", "main/nope"];
            for act in acts {
                match act {
                    Act::Level(li) => state.run.guidance_level = GuidanceLevel::ALL[li],
                    Act::GoTo(i) => {
                        let op = Op::Control(Control::GoTo(StepUid(uids[i].to_string())));
                        state = apply(&ir, state, op).0;
                    }
                    Act::Open => {
                        let before = state.clone();
                        let total = u32::try_from(current_hints(&ir, &before.run).len()).unwrap();
                        let prior = before
                            .run
                            .current
                            .as_ref()
                            .and_then(|c| before.hints_opened.get(c))
                            .copied()
                            .unwrap_or(0);
                        let (next, events) = apply(&ir, state, open());
                        prop_assert_eq!(events.len(), 1);
                        prop_assert_eq!(&next.run, &before.run);
                        match &events[0] {
                            Event::HintOpened {
                                opened, total: t, ..
                            } => {
                                prop_assert_eq!(*opened, prior + 1);
                                prop_assert_eq!(*t, total);
                                prop_assert!(*opened <= *t);
                                let cur = before.run.current.clone().unwrap();
                                let mut want = before.hints_opened.clone();
                                want.insert(cur, *opened);
                                prop_assert_eq!(&next.hints_opened, &want);
                            }
                            Event::Refused { reason, op, .. } => {
                                prop_assert_eq!(*reason, RefuseReason::NoHint);
                                prop_assert_eq!(*op, ControlKind::OpenHint);
                                prop_assert!(before.run.current.is_none() || prior >= total);
                                prop_assert_eq!(&next.hints_opened, &before.hints_opened);
                            }
                            other => prop_assert!(false, "unexpected {:?}", other),
                        }
                        state = next;
                    }
                }
            }
            Ok(())
        },
    );
}
