//! 分岐の区分（BUILD-7 の層 1。TASK-83 1/3）を、`apply` を通る固定列で確かめる試験。
//!
//! 規則の `when`・効果の `when`・遷移の辺の選び方・呼び出し（`enter`）と戻り（`exit_to`）は、判定器や
//! 効果の適用の単体の試験にも散っている。ここでは操作 1 回の全体（`apply`）を通し、分岐の結果が
//! 世界状態とイベント列にどう出るかを、3 Step の教材（`min-3step.json`）の写しを差し替えて固定する。
//! 条件の評価器は `apply` に結線されておらず（`NoConditions`）、`when` つきの規則・効果は現状は採られない。
//! 安全側に倒れる現状をここで固定し、結線されたときはこの試験を直す。
//! `next[].when` も core が評価しない（無条件の最初の辺だけを採る）。この現状の固定は `confirm_achieve.rs` の
//! `conditional_or_dangling_edges_do_not_transition` が持ち、ここでは扱わない。
//! 呼び出し元は `cargo test`（`make test`）のみである。

use std::path::PathBuf;

use fandhe_workshop_contract::ir::{Ir, Node, Transition};
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Decision, Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::{apply, load_ir};
use serde_json::{json, Value};

fn base_ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/engine-event-consistency/test-data/min-3step.json");
    load_ir(&std::fs::read(p).expect("試験データを読めること"))
        .expect("試験データが IR として読めること")
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn node_mut<'a>(ir: &'a mut Ir, id: &str) -> &'a mut Node {
    ir.nodes
        .iter_mut()
        .find(|n| n.uid == id)
        .expect("対象の節点があること")
}

/// `step` が入力待ち（`Entered`）で、`notes/draft` が世界にあり、`flags` が立っている状態。
fn at(step: &str, status: StepStatus, flags: &[&str]) -> SessionState {
    let u = uid(step);
    let mut s = SessionState::default();
    s.run.current = Some(u.clone());
    let slice = s.run.slices.entry(u.root()).or_default();
    slice.status.insert(u, status);
    for f in flags {
        slice.flags.insert((*f).to_string());
    }
    let mut el = fandhe_workshop_core::world::Element::default();
    el.attrs.insert("kind".into(), "none".into());
    s.run.world.insert("notes/draft".into(), el);
    s
}

fn kind_of(s: &SessionState) -> String {
    s.run.world["notes/draft"].attrs["kind"].to_string()
}

fn memo() -> Op {
    Op::Input(Input::Choice("opt-memo".into()))
}

fn accepted(events: &[Event]) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Accepted { .. }))
        .count()
}

fn count(events: &[Event], f: impl Fn(&Event) -> bool) -> usize {
    events.iter().filter(|e| f(e)).count()
}

fn rule(id: &str, when: Value, kind: &str) -> Value {
    json!({
        "id": id,
        "on": {
            "command": null, "missing": [], "args": {}, "option": "opt-memo", "approval": null,
            "target": null, "ui": null, "text": null, "cancel": null
        },
        "when": when,
        "outcome": {"kind": "accept", "effects": [
            {"set_attr": {"path": "notes/draft", "attr": "kind", "value": kind}}]}
    })
}

fn st2_rules(rules: Vec<Value>) -> Ir {
    let mut ir = base_ir();
    node_mut(&mut ir, "main/st-2").step.rules = rules
        .into_iter()
        .map(|v| serde_json::from_value(v).expect("規則の形が正しいこと"))
        .collect();
    ir
}

#[test]
fn rule_when_selects_branch_by_world_state() {
    // 現状の `apply` は条件の評価器を結線しておらず（`accept.rs` の `NoConditions`）、`when` つきの規則は
    // 旗の有無によらず採られない。`when` なしの規則だけが宣言の順で採られ、評価できない条件は安全側に倒れる。
    let ir = st2_rules(vec![
        rule("r-a", json!({"flag": "mode-a"}), "branch-a"),
        rule("r-unknown", json!({"no-such-word": 1}), "branch-unknown"),
        rule("r-plain", Value::Null, "plain"),
    ]);
    let entered = StepStatus::Entered;
    for flags in [&[][..], &["mode-a"][..]] {
        let (next, events) = apply(&ir, at("main/st-2", entered, flags), memo());
        assert_eq!(accepted(&events), 1, "{events:?}");
        assert_eq!(kind_of(&next), "plain");
        // 同じ入力を流し直すと、結果は同じ（決定性）。
        let (again, events2) = apply(&ir, at("main/st-2", entered, flags), memo());
        assert_eq!(next, again);
        assert_eq!(events, events2);
    }
    // `when` つきの規則しか無いなら、どの規則も採らず、世界状態は変わらない（安全側）。
    let ir = st2_rules(vec![
        rule("r-a", json!({"flag": "mode-a"}), "branch-a"),
        rule("r-unknown", json!({"no-such-word": 1}), "branch-unknown"),
    ]);
    let before = at("main/st-2", entered, &["mode-a"]);
    let (next, events) = apply(&ir, before.clone(), memo());
    assert_eq!(accepted(&events), 0, "{events:?}");
    assert_eq!(next.run.world, before.run.world);
    assert_eq!(next.run.current, before.run.current);
}

#[test]
fn effect_when_branches_applied_changes() {
    // 効果の `when` も同じ。評価できない項目は適用されず、`Warning` が 1 件残り、`when` なしの項目は適用される。
    let mut ir = base_ir();
    node_mut(&mut ir, "main/st-2").step.on_achieved = [
        json!({
            "set_attr": {"path": "notes/draft", "attr": "kind", "value": "late"},
            "when": {"flag": "late-ok"}
        }),
        json!({"set_attr": {"path": "notes/draft", "attr": "state", "value": "done"}}),
    ]
    .iter()
    .map(|v| serde_json::from_value(v.clone()).expect("効果の形が正しいこと"))
    .collect();
    let (next, events) = apply(
        &ir,
        at("main/st-2", StepStatus::Entered, &["late-ok"]),
        memo(),
    );
    assert_eq!(kind_of(&next), "memo");
    assert_eq!(
        next.run.world["notes/draft"].attrs["state"].to_string(),
        "done"
    );
    assert_eq!(
        count(&events, |e| matches!(e, Event::Warning { .. })),
        1,
        "{events:?}"
    );
}

#[test]
fn first_unconditional_edge_is_taken_among_several() {
    let mut ir = base_ir();
    node_mut(&mut ir, "main/st-2").step.next = vec![
        Transition {
            to: "main/st-3".into(),
            when: None,
        },
        Transition {
            to: "main/st-1".into(),
            when: None,
        },
    ];
    let before = at("main/st-2", StepStatus::Entered, &[]);
    let (next, events) = apply(&ir, before.clone(), memo());
    assert_eq!(next.run.current, Some(uid("main/st-3")));
    let landed: Vec<&StepUid> = events
        .iter()
        .filter_map(|e| match e {
            Event::Transitioned { to, .. } => Some(to),
            _ => None,
        })
        .collect();
    assert_eq!(landed, [&uid("main/st-3")], "{events:?}");
    let (again, _) = apply(&ir, before, memo());
    assert_eq!(next, again);
}

#[test]
fn use_enters_callee_and_exit_to_returns_to_caller() {
    // st-1 -> call（enter = st-2）。st-2 -> st-3（final, exit_to = call）。call -> after。
    let mut ir = base_ir();
    let mut call = ir.nodes[0].clone();
    call.uid = "main/call".into();
    call.step.id = "call".into();
    call.enter = Some("main/st-2".into());
    call.step.next = vec![Transition {
        to: "main/after".into(),
        when: None,
    }];
    let mut after = ir.nodes[0].clone();
    after.uid = "main/after".into();
    after.step.id = "after".into();
    after.step.next.clear();
    node_mut(&mut ir, "main/st-1").step.next = vec![Transition {
        to: "main/call".into(),
        when: None,
    }];
    node_mut(&mut ir, "main/st-3").exit_to = Some("main/call".into());
    ir.nodes.push(call);
    ir.nodes.push(after);

    // 呼び出し元の Step の達成で、呼び出し先の入口（st-2）に着く。
    let s = at("main/st-1", StepStatus::Accepted, &[]);
    let (s, ev1) = apply(
        &ir,
        s,
        Op::Input(Input::Ui {
            view: "v-files".into(),
            path: "notes/draft".into(),
        }),
    );
    assert_eq!(s.run.current, Some(uid("main/st-2")), "{ev1:?}");
    assert_eq!(count(&ev1, |e| matches!(e, Event::ModuleEntered { .. })), 1);
    assert_eq!(count(&ev1, |e| matches!(e, Event::ModuleExited { .. })), 0);

    // 呼び出し先の中を進み、最後の Step の達成で呼び出し元へ戻り、その先（after）に着く。
    let (s, ev2) = apply(&ir, s, memo());
    assert_eq!(s.run.current, Some(uid("main/st-3")), "{ev2:?}");
    let approve = || {
        Op::Input(Input::Approval {
            decision: Decision::Approve,
            target: None,
        })
    };
    // 承認の 1 回目は受付、2 回目が確認（達成）になる。
    let (s, _) = apply(&ir, s, approve());
    let (s, ev3) = apply(&ir, s, approve());
    assert_eq!(
        count(&ev3, |e| matches!(e, Event::ModuleExited { .. })),
        1,
        "{ev3:?}"
    );
    assert_eq!(s.run.current, Some(uid("main/after")), "{ev3:?}");
    assert_eq!(
        s.run.slices[&uid("main/call").root()].status[&uid("main/call")],
        StepStatus::Achieved
    );
}
