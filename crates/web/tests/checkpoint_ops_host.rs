//! 上部のチェックポイントの保存と復帰の操作（TASK-529 1/2。SHELL-13）の host の層の試験。
//!
//! 出す条件・押下の復号と照合・保存 → 先へ進む → 復帰・表示だけのモード・検査器・dirty を固定する。
//! ブラウザでの実際の押下は `tools/browser/checkpoint-restore.test.mjs` の担当である。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::checkpoint_ops::{
    ACTION_RESTORE_CHECKPOINT, ACTION_SAVE_CHECKPOINT, CHECKPOINT_OPS_FIELD, RESTORE_TESTID,
    SAVE_TESTID,
};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::{self, TopSection};
use serde_json::Value;

fn ir() -> Ir {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json");
    load_ir(&std::fs::read(p).unwrap()).unwrap()
}

fn state_at(uid: &str) -> SessionState {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let text = serde_json::to_string(&s["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{uid}\""));
    serde_json::from_str(&text).unwrap()
}

fn host_at(uid: &str) -> EngineHost {
    EngineHost::with_session(
        Session::from_state(ir(), state_at(uid)),
        standard_registry(),
    )
    .unwrap()
}

fn attr<'a>(n: &'a Node, name: &str) -> Option<&'a str> {
    match n {
        Node::Element { attrs, .. } => attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str()),
        _ => None,
    }
}

fn kids(n: &Node) -> &[Node] {
    match n {
        Node::Element { children, .. } => children,
        _ => &[],
    }
}

fn count(n: &Node, testid: &str) -> usize {
    usize::from(attr(n, "data-testid") == Some(testid))
        + kids(n).iter().map(|c| count(c, testid)).sum::<usize>()
}

fn act(name: &str, payload: &str) -> Option<HostAction> {
    EngineHost::decode_action(name, payload)
}

fn save(h: &mut EngineHost) {
    h.update(act(ACTION_SAVE_CHECKPOINT, "").unwrap());
}

fn goto(uid: &str) -> HostAction {
    HostAction::Engine(Op::Control(Control::GoTo(StepUid(uid.to_owned()))))
}

fn snapshot(h: &EngineHost) -> (String, usize) {
    let s = h.model().session().unwrap();
    (serde_json::to_string(s.state()).unwrap(), s.log().len())
}

fn restored_count(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Restored { .. }))
        .count()
}

fn checkpoints(h: &EngineHost) -> usize {
    h.model().session().unwrap().state().checkpoints.len()
}

fn current(h: &EngineHost) -> String {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .current
        .clone()
        .unwrap()
        .0
}

#[test]
fn 現在の_step_があるときは保存だけが出て_session_が無いときは0件() {
    let v = host_at("main/st-1-1").view();
    assert_eq!(count(&v, SAVE_TESTID), 1);
    assert_eq!(count(&v, RESTORE_TESTID), 0);
    assert_eq!(count(&v, TopSection::Checkpoint.testid()), 1);
    let v = EngineHost::new().view();
    assert_eq!(count(&v, SAVE_TESTID), 0);
    assert_eq!(count(&v, TopSection::Checkpoint.testid()), 1);
}

#[test]
fn 保存で復帰のボタンが増え_dirty_が積まれ世代は変わらない() {
    let mut h = host_at("main/st-1-1");
    let _ = h.dirty_fields();
    let gen_before = h.model().session().unwrap().state().run.generation;
    save(&mut h);
    assert_eq!(checkpoints(&h), 1);
    assert!(h.dirty_fields().contains(&CHECKPOINT_OPS_FIELD.name));
    assert_eq!(
        h.model().session().unwrap().state().run.generation,
        gen_before
    );
    let v = h.view();
    assert_eq!(count(&v, SAVE_TESTID), 1);
    assert_eq!(count(&v, RESTORE_TESTID), 1);
}

#[test]
fn 保存_先へ進む_復帰で位置と状態が保存の時点に戻る() {
    let mut h = host_at("main/st-1-1");
    save(&mut h);
    let at_save = serde_json::to_value(h.model().session().unwrap().state()).unwrap();
    h.update(goto("main/st-1-4"));
    assert_eq!(current(&h), "main/st-1-4");
    let gen_before = h.model().session().unwrap().state().run.generation;
    let restored_before = restored_count(&h);
    h.update(act(ACTION_RESTORE_CHECKPOINT, "1").unwrap());
    assert_eq!(current(&h), "main/st-1-1");
    assert_eq!(restored_count(&h), restored_before + 1);
    assert_ne!(
        h.model().session().unwrap().state().run.generation,
        gen_before,
        "復帰は世代を進める"
    );
    let now = serde_json::to_value(h.model().session().unwrap().state()).unwrap();
    assert_eq!(now["run"]["current"], at_save["run"]["current"]);
    assert_eq!(now["world"], at_save["world"]);
    // 同じ ID に何度でも戻れる。
    assert_eq!(checkpoints(&h), 1);
    assert_eq!(count(&h.view(), RESTORE_TESTID), 1);
}

#[test]
fn 改ざんした_payload_は復号で断る() {
    assert!(act(ACTION_SAVE_CHECKPOINT, "1").is_none());
    for bad in [
        "",
        "0",
        "01",
        "+1",
        " 1",
        "1 ",
        "12345678901",
        "4294967296",
        "x",
    ] {
        assert!(act(ACTION_RESTORE_CHECKPOINT, bad).is_none(), "{bad:?}");
    }
}

#[test]
fn 存在しない_id_の復帰は状態とログを変えない() {
    let mut h = host_at("main/st-1-1");
    save(&mut h);
    h.update(goto("main/st-1-3"));
    let before = snapshot(&h);
    h.update(act(ACTION_RESTORE_CHECKPOINT, "999").unwrap());
    assert_eq!(snapshot(&h), before);
}

#[test]
fn 現在の_step_が無いときは保存も復帰も何もしない() {
    let mut h = host_at("main/st-1-1");
    save(&mut h);
    // 現在の Step を外した状態で押下しても core を呼ばない。
    let mut state = h.model().session().unwrap().state().clone();
    state.run.current = None;
    let mut h2 =
        EngineHost::with_session(Session::from_state(ir(), state), standard_registry()).unwrap();
    assert_eq!(count(&h2.view(), SAVE_TESTID), 0);
    assert_eq!(count(&h2.view(), RESTORE_TESTID), 0);
    let before = snapshot(&h2);
    save(&mut h2);
    h2.update(act(ACTION_RESTORE_CHECKPOINT, "1").unwrap());
    assert_eq!(snapshot(&h2), before);
}

#[test]
fn 項目のある_view_は検査器を通り上部の中に1個だけある() {
    let mut h = host_at("main/st-1-1");
    save(&mut h);
    let v = h.view();
    assert_eq!(shell::check_complete_v2(&v), Ok(()));
    assert_eq!(shell::check_no_undefined(&v), Ok(()));
    assert_eq!(a11y::check_no_undefined(&v), Ok(()));
    fn find<'a>(n: &'a Node, id: &str) -> Option<&'a Node> {
        if attr(n, "data-testid") == Some(id) {
            return Some(n);
        }
        kids(n).iter().find_map(|c| find(c, id))
    }
    let top = find(&v, TopSection::Top.testid()).unwrap();
    assert_eq!(count(top, TopSection::Checkpoint.testid()), 1);
    assert_eq!(count(top, SAVE_TESTID), 1);
    assert_eq!(count(&v, TopSection::Checkpoint.testid()), 1);
}

#[test]
fn 提示を変えない更新では_dirty_にしない() {
    let mut h = host_at("main/st-1-1");
    save(&mut h);
    let _ = h.dirty_fields();
    h.update(HostAction::SelectPane(a11y::PaneTab::ALL[0]));
    assert!(!h.dirty_fields().contains(&CHECKPOINT_OPS_FIELD.name));
}

#[test]
fn 履歴を開くと_dirty_になる() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_web::host::ImportOutcome;

    let ir = ir();
    let env = build_envelope(&ir, &ir.starts[0].id, vec![], state_at("main/st-1-4")).unwrap();
    let bytes = export_bytes(&env).unwrap();
    let mut h = host_at("main/st-1-4");
    let _ = h.dirty_fields();
    let outcome = h.open_history(&bytes);
    assert!(!matches!(outcome, ImportOutcome::Refused(_)), "{outcome:?}");
    assert!(h.dirty_fields().contains(&CHECKPOINT_OPS_FIELD.name));
}

/// 表示だけのモード（版の違う履歴を開いた）では、項目が 0 件で、保存・復帰の押下は状態とログを変えない（VER-8）。
#[test]
fn 表示だけのモードでは出さず押下も捨てる() {
    use fandhe_workshop_core::history_envelope::build_envelope;
    use fandhe_workshop_core::history_export::export_bytes;
    use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
    use fandhe_workshop_core::history_replay::replay;
    use fandhe_workshop_web::host::ImportOutcome;

    let td = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata");
    let ir_json = |version: &str| {
        let mut v: Value = serde_json::from_slice(
            &std::fs::read(td.join("engine-event-consistency/test-data/min-3step.json")).unwrap(),
        )
        .unwrap();
        v["pack"]["version"] = serde_json::json!(version);
        v["pack"]["vocab"] = serde_json::json!("1.2");
        serde_json::to_vec(&v).unwrap()
    };
    let initial: SessionState = {
        let v: Value = serde_json::from_slice(
            &std::fs::read(td.join("contract/event-sequence.json")).unwrap(),
        )
        .unwrap();
        serde_json::from_value(v["initial"].clone()).unwrap()
    };
    let mut h = EngineHost::with_session(
        Session::from_state(load_ir(&ir_json("1.0.0")).unwrap(), initial.clone()),
        standard_registry(),
    )
    .unwrap();
    assert_eq!(count(&h.view(), SAVE_TESTID), 1, "前提: 開く前は出る");
    let ir2 = load_ir(&ir_json("2.0.0")).unwrap();
    let sample = std::fs::read(td.join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir2, initial, &env.ops).unwrap();
    let bytes = export_bytes(&build_envelope(&ir2, &env.start, env.ops, state).unwrap()).unwrap();
    let _ = h.dirty_fields();
    assert!(matches!(h.open_history(&bytes), ImportOutcome::ViewOnly(_)));
    assert!(h.dirty_fields().contains(&CHECKPOINT_OPS_FIELD.name));
    let v = h.view();
    assert_eq!(count(&v, SAVE_TESTID), 0);
    assert_eq!(count(&v, RESTORE_TESTID), 0);
    let before = snapshot(&h);
    save(&mut h);
    h.update(act(ACTION_RESTORE_CHECKPOINT, "1").unwrap());
    assert_eq!(snapshot(&h), before);
    assert!(h.history_to_save().is_none());
}
