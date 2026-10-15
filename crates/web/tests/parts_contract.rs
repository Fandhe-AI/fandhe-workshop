//! 表示部品の口（`parts`、TASK-343 1/3）の適合試験。ダミーの部品とホストが、契約どおりに登録・描画・振り分けできる
//! ことを確かめる。ダミーは試験の側にだけ置き、配信物には入れない。TASK-343 3/3 がこのファイルを拡張する。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::keyed::keyed_list;
use fandhe_frontend_core::{bind_text, el, text, Node};
use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_web::a11y::check_no_undefined;
use fandhe_workshop_web::parts::*;

#[derive(Default)]
struct DummyModel {
    label: String,
    rows: Vec<String>,
}

const F_LOG: &[PartField] = &[
    PartField {
        name: "log_text",
        kind: FieldKind::Bound,
    },
    PartField {
        name: "log_rows",
        kind: FieldKind::KeyedList,
    },
];
const F_ART: &[PartField] = &[PartField {
    name: "art_text",
    kind: FieldKind::Bound,
}];
const F_NONE: &[PartField] = &[];

struct DummyPart {
    kind: PartKind,
    fields: &'static [PartField],
}

impl DisplayPart<DummyModel> for DummyPart {
    fn kind(&self) -> PartKind {
        self.kind
    }
    fn fields(&self) -> &'static [PartField] {
        self.fields
    }
    fn view(&self, model: &DummyModel) -> Node {
        let mut children = vec![text(self.kind.name())];
        for f in self.fields {
            match f.kind {
                FieldKind::Bound => {
                    children.push(bind_text("span", vec![], f.name, &model.label));
                }
                FieldKind::KeyedList => {
                    let items: Vec<(String, Node)> = model
                        .rows
                        .iter()
                        .enumerate()
                        .map(|(i, r)| (format!("r{i}"), el("li", vec![], vec![text(r)])))
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
    fn bound_value(&self, field: &str, model: &DummyModel) -> Option<BoundValue> {
        self.fields
            .iter()
            .any(|f| f.name == field)
            .then(|| BoundValue::Text(model.label.clone()))
    }
}

enum DummyAction {
    Relabel(String),
}

struct DummyHost {
    registry: PartRegistry<DummyModel>,
    model: DummyModel,
    dirty: Vec<&'static str>,
}

fn part(kind: PartKind, fields: &'static [PartField]) -> Box<dyn DisplayPart<DummyModel>> {
    Box::new(DummyPart { kind, fields })
}

fn host() -> DummyHost {
    let mut registry = PartRegistry::new();
    registry.register(part(PartKind::Log, F_LOG)).unwrap();
    registry
        .register(part(PartKind::ArtifactView, F_ART))
        .unwrap();
    for k in [
        PartKind::Review,
        PartKind::Offers,
        PartKind::Checkpoints,
        PartKind::Candidates,
    ] {
        registry.register(part(k, F_NONE)).unwrap();
    }
    DummyHost {
        registry,
        model: DummyModel {
            label: "L".to_owned(),
            rows: vec!["row<1>".to_owned()],
        },
        dirty: vec![],
    }
}

impl Component for DummyHost {
    type Action = DummyAction;
    fn update(&mut self, action: Self::Action) {
        match action {
            DummyAction::Relabel(s) => {
                self.model.label = s;
                self.dirty = vec!["log_text", "art_text"];
            }
        }
    }
    fn view(&self) -> Node {
        let children = self.registry.views(&self.model).into_iter().map(|v| v.1);
        el("main", vec![], children.collect())
    }
    fn decode_action(_name: &str, _payload: &str) -> Option<Self::Action> {
        None
    }
}

impl DirtyTracked for DummyHost {
    fn dirty_fields(&self) -> &[&'static str] {
        &self.dirty
    }
}

impl BindingSource for DummyHost {
    fn bound_value(&self, field: &str) -> Option<BoundValue> {
        self.registry.bound_value(field, &self.model)
    }
}

#[test]
fn ダミーのホストは_mount_できる型の条件を満たす() {
    assert_mountable::<DummyHost>();
}

#[test]
fn 六種を登録すると_描画に各部品が含まれる() {
    let h = host();
    let html = render_component_html(&h);
    for k in PartKind::ALL {
        assert!(html.contains(k.name()), "{}: {html}", k.name());
    }
    assert!(html.contains("row&lt;1&gt;"), "{html}");
    assert_eq!(check_no_undefined(&h.view()), Ok(()));
}

#[test]
fn 同じ種類の二回目は拒否され_登録数は変わらない() {
    let mut h = host();
    let n = h.registry.len();
    assert_eq!(
        h.registry.register(part(PartKind::Log, F_NONE)),
        Err(RegisterError::DuplicateKind(PartKind::Log))
    );
    assert_eq!(h.registry.len(), n);
}

#[test]
fn field_名の重複と不正は拒否される() {
    static DUP_IN: &[PartField] = &[
        PartField {
            name: "a",
            kind: FieldKind::Bound,
        },
        PartField {
            name: "a",
            kind: FieldKind::Bound,
        },
    ];
    static ACROSS: &[PartField] = &[PartField {
        name: "log_text",
        kind: FieldKind::Bound,
    }];
    let mut r = PartRegistry::<DummyModel>::new();
    r.register(part(PartKind::Log, F_LOG)).unwrap();
    assert!(matches!(
        r.register(part(PartKind::Review, DUP_IN)),
        Err(RegisterError::DuplicateField("a"))
    ));
    assert!(matches!(
        r.register(part(PartKind::Review, ACROSS)),
        Err(RegisterError::DuplicateField("log_text"))
    ));
    for bad in ["", "Upper", "a-b", &"a".repeat(65)] {
        let name: &'static str = Box::leak(bad.to_owned().into_boxed_str());
        let fields: &'static [PartField] = Box::leak(Box::new([PartField {
            name,
            kind: FieldKind::Bound,
        }]));
        assert!(
            matches!(
                r.register(part(PartKind::Review, fields)),
                Err(RegisterError::InvalidFieldName(_))
            ),
            "{bad:?}"
        );
    }
    assert_eq!(r.len(), 1);
}

#[test]
fn owner_と_bound_value_は持ち主へ振り分けられる() {
    let h = host();
    assert_eq!(h.registry.owner("art_text"), Some(PartKind::ArtifactView));
    assert_eq!(
        h.registry.bound_value("art_text", &h.model),
        Some(BoundValue::Text("L".to_owned()))
    );
    assert_eq!(h.registry.owner("nope"), None);
    assert_eq!(h.registry.bound_value("nope", &h.model), None);
    assert_eq!(h.registry.bound_value("log_rows", &h.model), None);
    assert_eq!(h.registry.fields().count(), 3);
}

#[test]
fn update_の後の_dirty_はすべて登録済みで_未登録が混ざると_err() {
    let mut h = host();
    h.update(DummyAction::Relabel("N".to_owned()));
    assert_eq!(h.registry.check_dirty(h.dirty_fields()), Ok(()));
    assert_eq!(
        h.bound_value("log_text"),
        Some(BoundValue::Text("N".to_owned()))
    );
    assert_eq!(
        h.registry.check_dirty(&["log_text", "ghost"]),
        Err(UnknownField("ghost"))
    );
}

#[test]
fn 未登録の種類は_none_で_views_は登録順() {
    let mut r = PartRegistry::<DummyModel>::new();
    let m = DummyModel::default();
    r.register(part(PartKind::Review, F_NONE)).unwrap();
    r.register(part(PartKind::Log, F_NONE)).unwrap();
    assert!(r.view(PartKind::Offers, &m).is_none());
    assert!(r.view(PartKind::Log, &m).is_some());
    let kinds: Vec<PartKind> = r.views(&m).into_iter().map(|v| v.0).collect();
    assert_eq!(kinds, [PartKind::Review, PartKind::Log]);
}

#[test]
fn decode_action_は任意の名前で_none() {
    for n in ["tick", "complete", "esc", ""] {
        assert!(DummyHost::decode_action(n, "").is_none());
    }
}
