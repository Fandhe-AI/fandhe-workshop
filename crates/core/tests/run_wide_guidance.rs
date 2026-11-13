//! 案内レベルが実行全体で 1 つであることの観察の固定（TASK-303 1/2、GUIDE-13・D-55）。
//!
//! 2 周目を `Begin` した後に 1 周目の Step へ移っても、水準は 2 周目の開始点が決めた値のままで、表示に使う
//! 関数（ヒント・`do`・本文）もその水準で選ぶことを確かめる。実体ごとに水準を持たない決定（D-55）の現状の観察で、
//! 状態の形は変えない。`use` に `guidance` を書けないことは pack の `pack45_` が持つ。
//! チェックポイントへの `Restore` が写しの水準へ戻すのは MOD-9 の別の規則で、GUIDE-13 の「移る」には含めない
//! （`set_guidance.rs`・`docs/testing.md` 5.494）。変異は `GUIDE13-1`・`GUIDE13-2`。
//! TASK-303 2/2 で、中身のある状態の鍵の形と、実体の引数が水準を変えないことを足した。
//! 試験では `unwrap` を使ってよい。

mod common;

use common::contract_sample::testdata_dir;
use fandhe_workshop_contract::guidance::GuidanceLevel::{self, A, B};
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::body_level::select_body;
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::hint_select::current_hints;
use fandhe_workshop_core::history_replay::{first_difference, replay};
use fandhe_workshop_core::input::{CheckpointId, Control, Op};
use fandhe_workshop_core::ops_record::{apply_recorded, reload_recorded, OpsRecorder};
use fandhe_workshop_core::text_fill::step_do_at;
use fandhe_workshop_core::{apply, load_ir, SessionState};
use serde_json::{json, Value};

fn doc(t: &str) -> Value {
    json!({"version":1,"blocks":[{"t":"paragraph","children":[{"t":"text","text":t}]}]})
}

/// 全ての節点に、水準で変わる `do`・本文・ヒントを持たせる。表示の試験が現在の Step を選ばずに済むようにする。
fn node(base: &Value, scope: &str, unit: &str, sid: &str, enter: Option<&str>) -> Value {
    let mut n = base.clone();
    n["uid"] = json!(format!("{scope}/{sid}"));
    n["scope"] = json!(scope);
    n["unit"] = json!(unit);
    n["step"]["id"] = json!(sid);
    n["step"]["next"] = json!([]);
    n["step"]["do"] = json!("do-a");
    n["step"]["do_levels"] = json!({"B": "do-b"});
    n["step"]["hints"] = json!(["h-a", "h-b"]);
    n["body_doc"] = doc("body-a");
    n["body_levels_doc"] = json!({"B": doc("body-b")});
    if let Some(to) = enter {
        n["enter"] = json!(to);
    }
    n
}

fn ir() -> Ir {
    let path = testdata_dir().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let base = v["nodes"][0].clone();
    v["format_version"] = json!(2);
    v["pack"]["ir_format"] = json!(2);
    v["pack"]["guidance"] = json!("A");
    v["hints"] = json!({
        "h-a": {"levels": ["A"], "text": "hint-a"},
        "h-b": {"levels": ["B"], "text": "hint-b"},
    });
    v["nodes"] = json!([
        node(&base, "main", "ch-x", "x", None),
        node(&base, "main", "ch-a", "a0", None),
        node(&base, "main", "ch-a", "a1", Some("m@a/s1")),
        node(&base, "main", "ch-b", "b1", Some("m@b/s1")),
        node(&base, "m@a", "m", "s1", None),
        node(&base, "m@a", "m", "s2", None),
        node(&base, "m@b", "m", "s1", None),
    ]);
    v["instances"] = json!([
        {"id": "main", "module": null, "args": {}, "caller": null},
        {"id": "m@a", "module": "m", "args": {}, "caller": "main/a1"},
        {"id": "m@b", "module": "m", "args": {}, "caller": "main/b1"},
    ]);
    v["starts"] = json!([
        {"id": "s-a", "node": "main/a1", "effects": []},
        {"id": "s-b", "node": "main/b1", "effects": [], "guidance": "B"},
    ]);
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

fn uid(s: &str) -> StepUid {
    StepUid(s.to_string())
}

fn goto(to: &str) -> Op {
    Op::Control(Control::GoTo(uid(to)))
}

fn begin(id: &str) -> Op {
    Op::Control(Control::Begin(id.to_string()))
}

fn start_state() -> SessionState {
    let mut st = SessionState::default();
    st.run.current = Some(uid("main/x"));
    st
}

fn run(ir: &Ir, mut st: SessionState, ops: Vec<Op>) -> (SessionState, Vec<Event>) {
    let mut last = Vec::new();
    for op in ops {
        let (next, ev) = apply(ir, st, op);
        st = next;
        last = ev;
    }
    (st, last)
}

/// 現在の Step で、表示に使う 3 つの関数が水準 `l` の結果を返すこと。
fn shows(ir: &Ir, st: &SessionState, l: GuidanceLevel) {
    let cur = st.run.current.clone().unwrap();
    let node = ir.nodes.iter().find(|n| n.uid == cur.0).unwrap();
    let (hint, d, body) = match l {
        A => ("hint-a", "do-a", doc("body-a")),
        _ => ("hint-b", "do-b", doc("body-b")),
    };
    let ids = current_hints(ir, &st.run);
    let texts: Vec<&str> = ids
        .iter()
        .map(|id| ir.hints[*id]["text"].as_str().unwrap())
        .collect();
    assert_eq!(texts, vec![hint]);
    assert_eq!(step_do_at(&node.step, st.run.guidance_level), Some(d));
    let sel = select_body(node, st.run.guidance_level).unwrap();
    assert_eq!(sel.level, l);
    assert_eq!(serde_json::to_value(sel.doc).unwrap(), body);
}

#[test]
fn guide13_goto_main_step_of_round1_chapter_keeps_b() {
    let ir = ir();
    let (st, _) = run(&ir, start_state(), vec![begin("s-a"), begin("s-b")]);
    assert_eq!(st.run.guidance_level, B);
    let (st, ev) = run(&ir, st, vec![goto("main/a0")]);
    assert!(
        ev.iter().any(|e| matches!(e, Event::Moved { .. })),
        "{ev:?}"
    );
    assert_eq!(st.run.current, Some(uid("main/a0")));
    assert_eq!(st.run.guidance_level, B);
    shows(&ir, &st, B);
}

#[test]
fn guide13_goto_into_closed_round1_is_refused_and_keeps_b() {
    let ir = ir();
    let (before, _) = run(&ir, start_state(), vec![begin("s-a"), begin("s-b")]);
    for to in ["m@a/s1", "main/a1"] {
        let (after, ev) = run(&ir, before.clone(), vec![goto(to)]);
        assert!(
            matches!(
                ev.as_slice(),
                [Event::Refused {
                    op: ControlKind::GoTo,
                    reason: RefuseReason::ClosedRound,
                    ..
                }]
            ),
            "{ev:?}"
        );
        assert_eq!(after.run, before.run);
        assert_eq!(after.run.guidance_level, B);
        shows(&ir, &after, B);
    }
}

#[test]
fn guide13_crossing_chapters_without_begin_keeps_level() {
    let ir = ir();
    for (start, level) in [("s-a", A), ("s-b", B)] {
        let (st, _) = run(&ir, start_state(), vec![begin(start)]);
        assert_eq!(st.run.guidance_level, level);
        for to in ["main/x", "main/a0", "m@b/s1", "main/x"] {
            let (next, _) = run(&ir, st.clone(), vec![goto(to)]);
            assert_eq!(next.run.guidance_level, level, "{start} -> {to}");
        }
    }
}

#[test]
fn guide13_rebegin_round1_start_without_declaration_keeps_b() {
    let ir = ir();
    let (st, _) = run(&ir, start_state(), vec![begin("s-a"), begin("s-b")]);
    // 許されるかどうかは問わない。どちらでも水準は B のまま。
    let (st, _) = run(&ir, st, vec![begin("s-a")]);
    assert_eq!(st.run.guidance_level, B);
}

#[test]
fn guide13_restore_to_round1_checkpoint_follows_mod9_not_guide13() {
    let ir = ir();
    let (st, _) = run(
        &ir,
        start_state(),
        vec![begin("s-a"), Op::Control(Control::Save)],
    );
    let (st, _) = run(&ir, st, vec![begin("s-b")]);
    assert_eq!(st.run.guidance_level, B);
    // 移動ではなく復帰。写しの水準 A へ戻るのは MOD-9 の規則で、GUIDE-13 の対象外（5.494）。
    let (st, ev) = run(
        &ir,
        st,
        vec![Op::Control(Control::Restore(CheckpointId(1)))],
    );
    assert!(
        ev.iter().any(|e| matches!(e, Event::Restored { .. })),
        "{ev:?}"
    );
    assert_eq!(st.run.guidance_level, A);
}

#[test]
fn guide13_level_after_round1_move_survives_replay() {
    let ir = ir();
    let initial = start_state();
    let mut rec = OpsRecorder::new();
    let mut st = initial.clone();
    for op in [begin("s-a"), begin("s-b"), goto("main/a0")] {
        st = apply_recorded(&ir, st, &mut rec, op).0;
    }
    let (st, _) = reload_recorded(st, &mut rec);
    assert_eq!(st.run.guidance_level, B);
    let replayed = replay(&ir, initial, rec.ops()).expect("replay");
    assert_eq!(first_difference(&st, &replayed), None);
    assert_eq!(replayed.run.guidance_level, B);
}

/// 木を辿り、名前に `guidance` を含む鍵を (親の欄名, 鍵) の組で集める。
fn guidance_keys(v: &Value, parent: &str, out: &mut Vec<(String, String)>) {
    match v {
        Value::Object(m) => {
            for (k, c) in m {
                if k.contains("guidance") {
                    out.push((parent.to_string(), k.clone()));
                }
                guidance_keys(c, k, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|c| guidance_keys(c, parent, out)),
        _ => {}
    }
}

#[test]
fn guide13_state_shape_has_only_run_level_after_round1_move() {
    let ir = ir();
    let (st, _) = run(
        &ir,
        start_state(),
        vec![
            begin("s-a"),
            Op::Control(Control::Save),
            begin("s-b"),
            Op::Control(Control::Save),
            goto("main/a0"),
        ],
    );
    assert!(!st.checkpoints.is_empty());
    let v = serde_json::to_value(&st).unwrap();
    let mut found = Vec::new();
    guidance_keys(&v, "", &mut found);
    assert!(!found.is_empty());
    for (parent, key) in &found {
        assert_eq!(key, "guidance_level", "{found:?}");
        assert_eq!(parent, "run", "根ごとの写しの下に水準が現れた: {found:?}");
    }
    assert_eq!(v["run"]["guidance_level"], json!("B"));
}

#[test]
fn guide13_instance_args_named_guidance_do_not_change_level() {
    // 基の IR は ir() と同じ。args だけ差し替える。
    let base = ir();
    let mut v = serde_json::to_value(&base).unwrap();
    for inst in v["instances"].as_array_mut().unwrap() {
        if inst["id"] == json!("m@b") {
            inst["args"] = json!({"guidance": "C"});
        }
    }
    let ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let (st, _) = run(&ir, start_state(), vec![begin("s-a"), begin("s-b")]);
    assert_eq!(st.run.guidance_level, B);
    let (st, _) = run(&ir, st, vec![goto("main/a0")]);
    assert_eq!(st.run.guidance_level, B);
    shows(&ir, &st, B);
}
