//! ホスト（`host::EngineHost`、TASK-347 1/2）の契約適合の試験。mount できること・入力欄の互換・部品の振り分け・
//! dirty の求め方・core との一致（表示層が進行管理を持たないことの動的な確認）を確かめる。
//! ダミーの部品は試験の側にだけ置く。試験では `unwrap` を使ってよい。

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_core::keyed::keyed_list;
use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::changed::replay_changes;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::{apply, SessionState};
use fandhe_workshop_core::world::World;
use fandhe_workshop_core::{load_ir, Projection, Session};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::engine_parts::{event_kind_name, standard_registry};
use fandhe_workshop_web::host::{EngineHost, HostAction, HostError, HostModel};
use fandhe_workshop_web::log_lines::line_from_event;
use fandhe_workshop_web::parts::*;

const F_LOG: &[PartField] = &[
    PartField {
        name: "log_len",
        kind: FieldKind::Bound,
    },
    PartField {
        name: "log_rows",
        kind: FieldKind::KeyedList,
    },
    PartField {
        name: "log_const",
        kind: FieldKind::Bound,
    },
];
const F_REVIEW: &[PartField] = &[PartField {
    name: "review_len",
    kind: FieldKind::Bound,
}];
const F_CLASH: &[PartField] = &[PartField {
    name: "input_notice",
    kind: FieldKind::Bound,
}];

struct Dummy {
    kind: PartKind,
    fields: &'static [PartField],
}

impl DisplayPart<HostModel> for Dummy {
    fn kind(&self) -> PartKind {
        self.kind
    }
    fn fields(&self) -> &'static [PartField] {
        self.fields
    }
    fn view(&self, model: &HostModel) -> Node {
        let mut children = vec![text(self.kind.name())];
        for f in self.fields {
            match f.kind {
                FieldKind::Bound => {
                    let v = self.value(f.name, model);
                    children.push(bind_text("span", vec![], f.name, &v));
                }
                FieldKind::KeyedList => {
                    let items: Vec<(String, Node)> = (0..model.projection().log().len())
                        .map(|i| (format!("r{i}"), el("li", vec![], vec![text("row")])))
                        .collect();
                    children.push(match keyed_list("ul", vec![], f.name, items) {
                        Ok(n) => n,
                        Err(_) => el("ul", vec![], vec![]),
                    });
                }
            }
        }
        el("section", vec![], children)
    }
    fn bound_value(&self, field: &str, model: &HostModel) -> Option<BoundValue> {
        self.fields
            .iter()
            .any(|f| f.name == field)
            .then(|| BoundValue::Text(self.value(field, model)))
    }
}

impl Dummy {
    fn value(&self, field: &str, model: &HostModel) -> String {
        match field {
            "log_len" => model.projection().log().len().to_string(),
            "review_len" => model.projection().review().len().to_string(),
            _ => "const".to_owned(),
        }
    }
}

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn load_min_ir() -> Ir {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    load_ir(&bytes).unwrap()
}

fn sample() -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(testdata().join("contract/event-sequence.json")).unwrap())
        .unwrap()
}

fn registry() -> PartRegistry<HostModel> {
    let mut r = PartRegistry::new();
    r.register(Box::new(Dummy {
        kind: PartKind::Log,
        fields: F_LOG,
    }))
    .unwrap();
    r.register(Box::new(Dummy {
        kind: PartKind::Review,
        fields: F_REVIEW,
    }))
    .unwrap();
    r
}

fn loaded_host() -> EngineHost {
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    let session = Session::from_state(load_min_ir(), state);
    EngineHost::with_session(session, registry()).unwrap()
}

fn input(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

#[test]
fn mount_できる境界を満たす() {
    assert_mountable::<EngineHost>();
}

#[test]
fn 既定のホストの描画に入力欄の_testid_が揃い_a11y_を満たす() {
    let host = EngineHost::new();
    let h = html(&host);
    for id in ["hello-root", "draft-input", "submit-input", "input-notice"] {
        assert!(h.contains(&format!("data-testid=\"{id}\"")), "{id}: {h}");
    }
    assert!(h.contains("data-testid=\"workshop-root\""), "{h}");
    assert!(check_no_undefined(&host.view()).is_ok());
}

#[test]
fn action_の復号は入力欄と互換で_未知の名前は_none() {
    assert!(matches!(
        EngineHost::decode_action("draft", "x"),
        Some(HostAction::Input(_))
    ));
    assert!(matches!(
        EngineHost::decode_action("submit", ""),
        Some(HostAction::Input(_))
    ));
    #[cfg(feature = "test-hooks")]
    assert!(matches!(
        EngineHost::decode_action("keydown-probe", ""),
        Some(HostAction::Input(_))
    ));
    for name in ["tick", "click", ""] {
        assert!(EngineHost::decode_action(name, "").is_none(), "{name}");
    }
}

#[test]
fn 入力欄の経路の_dirty_と束縛値() {
    let mut host = EngineHost::new();
    let over = "あ".repeat(20_001);
    host.update(input("draft", &over));
    assert_eq!(host.dirty_fields(), ["input_notice"]);
    let notice = host.bound_value("input_notice");
    assert!(matches!(notice, Some(BoundValue::Text(ref s)) if !s.is_empty()));
    host.update(input("draft", "ok"));
    assert_eq!(host.dirty_fields(), ["input_notice"]);
    assert!(
        matches!(host.bound_value("input_notice"), Some(BoundValue::Text(ref s)) if s.is_empty())
    );
    host.update(input("submit", ""));
    assert_eq!(host.dirty_fields(), ["learner_log"]);
    assert!(host.bound_value("unknown").is_none());
}

#[test]
fn 部品の描画と束縛値は持ち主へ振り分けられる() {
    let host = loaded_host();
    let h = html(&host);
    assert!(h.contains("log"), "{h}");
    assert!(h.contains("review"), "{h}");
    assert!(matches!(
        host.bound_value("log_len"),
        Some(BoundValue::Text(_))
    ));
    assert!(matches!(
        host.bound_value("review_len"),
        Some(BoundValue::Text(_))
    ));
    assert!(host.bound_value("log_rows").is_none());
    assert!(host.bound_value("nothing").is_none());
}

#[test]
fn field_がホストと衝突する部品は拒否される() {
    let mut r = PartRegistry::new();
    r.register(Box::new(Dummy {
        kind: PartKind::Log,
        fields: F_CLASH,
    }))
    .unwrap();
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    let session = Session::from_state(load_min_ir(), state);
    let err = EngineHost::with_session(session, r).unwrap_err();
    assert_eq!(err, HostError::FieldCollision("input_notice"));
}

#[test]
fn engine_の操作の_dirty_は登録済みで_値の変わらない_bound_は入らない() {
    let mut host = loaded_host();
    host.update(HostAction::Engine(Op::Input(Input::Command(
        "/make-note Bad_Name".to_owned(),
    ))));
    let dirty: Vec<&'static str> = host.dirty_fields().to_vec();
    assert!(host.registry().check_dirty(&dirty).is_ok(), "{dirty:?}");
    assert!(dirty.contains(&"log_len"), "{dirty:?}");
    assert!(dirty.contains(&"log_rows"), "{dirty:?}");
    assert!(!dirty.contains(&"log_const"), "{dirty:?}");
    assert!(!host.model().projection().log().is_empty());
}

#[test]
fn 未読込で_engine_の操作を送っても_panic_せず_dirty_は空() {
    let mut host = EngineHost::new();
    host.update(input("draft", &"あ".repeat(20_001)));
    assert!(!host.dirty_fields().is_empty());
    host.update(HostAction::Engine(Op::Control(Control::Save)));
    assert!(host.dirty_fields().is_empty());
    assert!(host.model().session().is_none());
}

#[test]
fn ホストの結果は_core_の純関数の畳み込みと一致する() {
    let sample = sample();
    let ir = load_min_ir();
    let initial: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let ops: Vec<Op> = sample["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e["op"].clone()).unwrap())
        .collect();
    assert!(!ops.is_empty());

    let mut host = loaded_host();
    let mut state = initial;
    let mut emitted = 0usize;
    for op in ops {
        let (next, events) = apply(&ir, state, op.clone());
        state = next;
        emitted += events.len();
        host.update(HostAction::Engine(op));
    }
    assert!(emitted > 0, "イベントが 1 件も出ていない");
    let session = host.model().session().unwrap();
    assert!(!session.log().is_empty());
    assert_eq!(session.state(), &state);
    assert_eq!(
        host.model().projection(),
        &Projection::from_events(&state.log)
    );
}

// ---- TASK-347 2/2: 3 つの表示が同じイベント列から導かれること ----

fn begun_host() -> EngineHost {
    let state: SessionState = serde_json::from_value(sample()["initial"].clone()).unwrap();
    let session = Session::from_state(load_min_ir(), state);
    EngineHost::with_session(session, standard_registry()).unwrap()
}

fn sample_ops() -> Vec<Op> {
    sample()["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| serde_json::from_value(e["op"].clone()).unwrap())
        .collect()
}

fn consumed_of(host: &EngineHost, field: &str) -> String {
    match host.bound_value(field) {
        Some(BoundValue::Text(s)) => s,
        other => panic!("{field}: {other:?}"),
    }
}

#[test]
fn 標準の登録簿は_3_部品で_入力欄の_field_と衝突しない() {
    let r = standard_registry();
    assert_eq!(r.len(), 3);
    assert_eq!(r.fields().count(), 8);
    assert!(r
        .fields()
        .all(|f| !["input_notice", "learner_log"].contains(&f.name)));
    assert!(EngineHost::with_ir(load_min_ir(), standard_registry()).is_ok());
}

#[test]
fn 操作のたびに_3_つの表示の元が同じイベント列から導かれ_consumed_が揃う() {
    let mut host = begun_host();
    let mut kinds = std::collections::BTreeSet::new();
    for op in sample_ops() {
        host.update(HostAction::Engine(op));
        let session = host.model().session().unwrap();
        let log = session.log();
        // 増分と一括の一致（EVT-1）。
        assert_eq!(host.model().projection(), &Projection::from_events(log));
        // 左はイベント列だけから導け、core の状態の世界と一致する（P-1b）。
        let mut replayed = World::new();
        replay_changes(&mut replayed, log);
        assert_eq!(host.model().world(), &replayed);
        assert_eq!(host.model().world(), &session.state().run.world);
        // 3 部品の consumed は、どれもログの長さと等しい。
        let n = log.len().to_string();
        for f in [
            "engine_log_consumed",
            "artifact_consumed",
            "review_consumed",
        ] {
            assert_eq!(consumed_of(&host, f), n, "{f}");
        }
        for e in log {
            kinds.insert(event_kind_name(e));
        }
        let h = html(&host);
        assert!(check_no_undefined(&host.view()).is_ok(), "{h}");
        // ログの行は、行にできるイベント（入力・進行・停止。TASK-434）だけである。行数は一括で導いた行のうち
        // `line_from_event` が行にするものの数と比べる。
        assert_eq!(
            h.matches("data-event=").count(),
            Projection::from_events(log)
                .log()
                .iter()
                .filter(|r| line_from_event(
                    &log[r.event as usize],
                    host.model().judged_text(r.event)
                )
                .is_some())
                .count(),
            "{h}"
        );
    }
    eprintln!("出たイベントの種類: {kinds:?}");
    assert!(kinds.contains("changed"), "{kinds:?}");
}

#[test]
fn 復習のうち現在の世界に残る経路は成果物の部品にも出る() {
    let mut host = begun_host();
    for op in sample_ops() {
        host.update(HostAction::Engine(op));
    }
    let h = html(&host);
    let world = host.model().world();
    for item in host.model().projection().review() {
        for c in &item.changes {
            if c.after.is_some() && world.contains_key(&c.path) {
                assert!(h.contains(&format!("data-path=\"{}\"", c.path)), "{h}");
            }
        }
    }
}

#[test]
fn 未読込のホストでも標準の部品は空の表示で描かれる() {
    let host = EngineHost::new();
    let h = html(&host);
    for id in ["part-log", "part-artifact-view", "part-review"] {
        assert!(h.contains(&format!("data-testid=\"{id}\"")), "{h}");
    }
    assert!(!h.contains("data-event="), "{h}");
    assert!(check_no_undefined(&host.view()).is_ok());
}

#[cfg(feature = "test-hooks")]
#[test]
fn raw_は_op_の_json_を復号し_壊れた_payload_は_none() {
    let ok = EngineHost::decode_action("raw", "{\"control\":{\"begin\":\"s-1\"}}");
    assert!(matches!(
        ok,
        Some(HostAction::Engine(Op::Control(Control::Begin(ref s)))) if s == "s-1"
    ));
    for bad in ["", "{", "{\"control\":\"nope\"}", "[]"] {
        assert!(EngineHost::decode_action("raw", bad).is_none(), "{bad}");
    }
    let h = html(&EngineHost::new());
    assert!(h.contains("data-testid=\"raw-trigger\""), "{h}");
    assert!(h.contains("data-action=\"raw\""), "{h}");
}

#[cfg(not(feature = "test-hooks"))]
#[test]
fn 配信する設定では_raw_を復号せず_raw_trigger_も描かない() {
    assert!(EngineHost::decode_action("raw", "{\"control\":{\"begin\":\"s-1\"}}").is_none());
    assert!(!html(&EngineHost::new()).contains("raw-trigger"));
}

// ---- TASK-349 1/2: tick・complete を Runtime の口で届ける ----

fn addr(generation: u64, step: &str, index: u32) -> TickAddr {
    TickAddr {
        generation: Generation(generation),
        step: StepUid::parse(step).unwrap(),
        index,
    }
}

/// 通知された宛先を記録する通知先を結んだホストと、その記録。
fn recording_host() -> (EngineHost, Rc<RefCell<Vec<Option<TickAddr>>>>) {
    let mut host = begun_host();
    let seen: Rc<RefCell<Vec<Option<TickAddr>>>> = Rc::default();
    let sink = Rc::clone(&seen);
    host.set_tick_request(Box::new(move |next| sink.borrow_mut().push(next)));
    (host, seen)
}

fn make_note() -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

fn ignored_count(host: &EngineHost) -> usize {
    host.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| event_kind_name(e) == "ignored")
        .count()
}

#[test]
fn tick_と_complete_は_runtime_の復号で_op_とホストの_action_になる() {
    let tick = EngineHost::decode_action("tick", "1 0 main/st-1");
    assert!(matches!(
        tick,
        Some(HostAction::Engine(Op::Tick(ref a))) if *a == addr(1, "main/st-1", 0)
    ));
    assert!(matches!(
        EngineHost::decode_action("complete", ""),
        Some(HostAction::Complete)
    ));
    for bad in [
        "",
        "01 0 main/st-1",
        "1 0",
        "1 0 main/st-1 x",
        "1 -1 main/st-1",
        "x",
    ] {
        assert!(EngineHost::decode_action("tick", bad).is_none(), "{bad}");
    }
    assert!(EngineHost::decode_action("complete", "x").is_none());
    // esc は従来どおり入力欄の action になる。
    assert!(matches!(
        EngineHost::decode_action("esc", "Escape"),
        Some(HostAction::Input(_))
    ));
}

#[test]
fn complete_は状態もログも_dirty_も変えない() {
    let mut host = begun_host();
    host.update(make_note());
    let before = (
        host.model().session().unwrap().state().clone(),
        host.model().projection().clone(),
    );
    host.update(HostAction::Complete);
    assert!(host.dirty_fields().is_empty());
    assert_eq!(host.model().session().unwrap().state(), &before.0);
    assert_eq!(host.model().projection(), &before.1);
}

#[test]
fn 進行の宛先は設定した時点と更新のたびに通知される() {
    let (mut host, seen) = recording_host();
    assert_eq!(seen.borrow().as_slice(), &[None]);
    host.update(make_note());
    assert_eq!(seen.borrow().last(), Some(&Some(addr(1, "main/st-1", 0))));
    assert_eq!(host.next_tick(), Some(addr(1, "main/st-1", 0)));
    let tick0 = addr(1, "main/st-1", 0);
    host.update(HostAction::Engine(Op::Tick(tick0)));
    assert_eq!(seen.borrow().last(), Some(&Some(addr(1, "main/st-1", 1))));
}

#[test]
fn 最後の進行の後は_none_が通知される() {
    let (mut host, seen) = recording_host();
    host.update(make_note());
    for i in 0..5 {
        let next = host.next_tick().expect("処理中のはず");
        assert_eq!(next, addr(1, "main/st-1", i));
        host.update(HostAction::Engine(Op::Tick(next)));
    }
    assert_eq!(host.next_tick(), None);
    assert_eq!(seen.borrow().last(), Some(&None));
}

#[test]
fn 停止の後の古い宛先は_ignored_1件で状態を変えず_再入力で新しい世代の宛先になる() {
    let (mut host, seen) = recording_host();
    host.update(make_note());
    let stale = host.next_tick().unwrap();
    host.update(HostAction::Engine(Op::Control(Control::Stop)));
    assert_eq!(seen.borrow().last(), Some(&None));
    let state_after_stop = host.model().session().unwrap().state().clone();
    let ignored_before = ignored_count(&host);
    host.update(HostAction::Engine(Op::Tick(stale.clone())));
    assert_eq!(ignored_count(&host), ignored_before + 1);
    let session = host.model().session().unwrap();
    assert_eq!(session.state().run, state_after_stop.run);
    assert_eq!(session.generation(), Generation(2));
    assert_eq!(seen.borrow().last(), Some(&None));
    // 同じ宛先をもう一度届けても、記録が 1 件ずつ増えるだけで状態は変わらない。
    host.update(HostAction::Engine(Op::Tick(stale)));
    assert_eq!(
        host.model().session().unwrap().state().run,
        state_after_stop.run
    );
    host.update(make_note());
    assert_eq!(seen.borrow().last(), Some(&Some(addr(2, "main/st-1", 0))));
}
