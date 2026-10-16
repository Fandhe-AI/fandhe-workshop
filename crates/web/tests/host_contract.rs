//! ホスト（`host::EngineHost`、TASK-347 1/2）の契約適合の試験。mount できること・入力欄の互換・部品の振り分け・
//! dirty の求め方・core との一致（表示層が進行管理を持たないことの動的な確認）を確かめる。
//! ダミーの部品は試験の側にだけ置く。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::keyed::keyed_list;
use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::session::{apply, SessionState};
use fandhe_workshop_core::{load_ir, Projection, Session};
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::host::{EngineHost, HostAction, HostError, HostModel};
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
    for name in ["tick", "complete", "click", ""] {
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
