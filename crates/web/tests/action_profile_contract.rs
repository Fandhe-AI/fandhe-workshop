//! action の名前・宛先（`action`）と操作プロファイル・設定の渡し方（`profile`、TASK-343 2/3）の適合試験。
//! ダミーのホストと部品は試験の側にだけ置き、配信物には入れない。
//! 試験では `unwrap` を使ってよい（失敗はそのまま試験の失敗になる）。

use fandhe_frontend_core::{el, text, Node};
use fandhe_frontend_interactive::dispatch;
use fandhe_frontend_wasm_full::{render_component_html, DispatchOutcome};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_web::action::*;
use fandhe_workshop_web::parts::*;
use fandhe_workshop_web::profile::*;

struct Model {
    settings: DisplaySettings,
}

impl SettingsSource for Model {
    fn display_settings(&self) -> &DisplaySettings {
        &self.settings
    }
}

/// 設定を境界で求め、接頭辞を文字のノードとして描く部品。
struct PrefixPart(PartKind);

impl<M: SettingsSource> DisplayPart<M> for PrefixPart {
    fn kind(&self) -> PartKind {
        self.0
    }
    fn fields(&self) -> &'static [PartField] {
        &[]
    }
    fn view(&self, model: &M) -> Node {
        let prefix = model.display_settings().profile.command_prefix.as_str();
        el(
            "section",
            vec![],
            vec![text(format!("{}:{prefix}", self.0.name()))],
        )
    }
}

struct Host {
    registry: PartRegistry<Model>,
    model: Model,
    log: Vec<RuntimeAction>,
}

fn host(settings: DisplaySettings) -> Host {
    let mut registry = PartRegistry::new();
    for k in PartKind::ALL {
        registry.register(Box::new(PrefixPart(k))).unwrap();
    }
    Host {
        registry,
        model: Model { settings },
        log: vec![],
    }
}

impl Component for Host {
    type Action = RuntimeAction;
    fn update(&mut self, action: Self::Action) {
        self.log.push(action);
    }
    fn view(&self) -> Node {
        let s = &self.model.settings;
        let mut attrs: Vec<(&str, &str)> = ESC_KEYDOWN_ATTRS.to_vec();
        attrs.extend(s.root_attrs());
        let children = self
            .registry
            .visible_views(s, &self.model)
            .into_iter()
            .map(|v| v.1)
            .collect();
        el("main", attrs, children)
    }
    fn decode_action(name: &str, payload: &str) -> Option<Self::Action> {
        RuntimeAction::decode(name, payload)
    }
}

impl DirtyTracked for Host {
    fn dirty_fields(&self) -> &[&'static str] {
        &[]
    }
}

impl BindingSource for Host {
    fn bound_value(&self, _field: &str) -> Option<BoundValue> {
        None
    }
}

fn addr(g: u64, step: &str, i: u32) -> TickAddr {
    TickAddr {
        generation: Generation(g),
        step: StepUid::parse(step).unwrap(),
        index: i,
    }
}

#[test]
fn ダミーのホストは_mount_できる型の条件を満たす() {
    assert_mountable::<Host>();
}

#[test]
fn 名前の定数は三つで重複しない() {
    assert_eq!(ActionName::ALL.len(), 3);
    let names: Vec<&str> = ActionName::ALL.iter().map(|a| a.as_str()).collect();
    assert_eq!(names, ["tick", "complete", "esc"]);
    assert_eq!(
        (ACTION_TICK, ACTION_COMPLETE, ACTION_ESC),
        ("tick", "complete", "esc")
    );
    for a in ActionName::ALL {
        assert_eq!(ActionName::from_name(a.as_str()), Some(a));
    }
    assert_eq!(ActionName::from_name("Tick"), None);
}

#[test]
fn 宛先つきの_tick_は符号化と復号で往復する() {
    let cases = [
        addr(1, "main/a", 0),
        addr(7, "m@x/b@y/step", 3),
        addr(u64::MAX, "main/s", u32::MAX),
    ];
    for a in cases {
        let act = RuntimeAction::Tick(a);
        let (name, payload) = act.encode();
        assert_eq!(name, "tick");
        assert_eq!(RuntimeAction::decode(name, &payload), Some(act));
    }
    assert_eq!(
        RuntimeAction::Tick(addr(2, "main/a", 5)).encode().1,
        "2 5 main/a"
    );
}

#[test]
fn 不正な_payload_は_none_で_panic_しない() {
    let bad = [
        ("unknown", ""),
        ("tick", ""),
        ("tick", "1"),
        ("tick", "1 0"),
        ("tick", "1 0 "),
        ("tick", "1 0 main/a extra"),
        ("tick", "1 0 main/a b"),
        ("tick", "01 0 main/a"),
        ("tick", "1 00 main/a"),
        ("tick", "+1 0 main/a"),
        ("tick", "1 -0 main/a"),
        ("tick", " 1 0 main/a"),
        ("tick", "1  0 main/a"),
        ("tick", "1 0 main/a "),
        ("tick", "18446744073709551616 0 main/a"),
        ("tick", "1 4294967296 main/a"),
        ("tick", "1 0 noslash"),
        ("tick", "1 0 bad@/x"),
        ("complete", "x"),
        ("complete", " "),
        ("esc", ""),
        ("esc", "Esc"),
        ("esc", "escape"),
    ];
    for (n, p) in bad {
        assert_eq!(RuntimeAction::decode(n, p), None, "{n} {p:?}");
    }
    assert_eq!(
        RuntimeAction::decode("complete", ""),
        Some(RuntimeAction::Complete)
    );
    assert_eq!(
        RuntimeAction::decode("esc", "Escape"),
        Some(RuntimeAction::Esc)
    );
}

#[test]
fn dispatch_で三つの名前が_update_に届き_不正な組は届かない() {
    let mut h = host(DisplaySettings::default());
    for act in [
        RuntimeAction::Tick(addr(1, "main/a", 0)),
        RuntimeAction::Complete,
        RuntimeAction::Esc,
    ] {
        let before = h.log.len();
        let (name, payload) = act.encode();
        assert!(dispatch(&mut h, name, &payload));
        assert_eq!(h.log.len(), before + 1);
        assert_eq!(h.log.last(), Some(&act));
    }
    let before = h.log.len();
    assert!(!dispatch(&mut h, "tick", "1 0"));
    assert!(!dispatch(&mut h, "esc", ""));
    assert!(!dispatch(&mut h, "nope", ""));
    assert_eq!(h.log.len(), before);
}

#[test]
fn esc_の_keydown_属性は_main_に出て_prevent_default_は出ない() {
    let h = host(DisplaySettings::default());
    let html = render_component_html(&h);
    assert!(html.contains("data-action-keydown=\"esc\""), "{html}");
    assert!(html.contains("data-keys=\"Escape\""), "{html}");
    assert!(!html.contains("data-keydown-prevent-default"), "{html}");
    assert!(!html.contains("data-payload"), "{html}");
    assert_eq!(
        RuntimeAction::Esc.encode(),
        ("esc", ESC_KEY_TOKEN.to_owned())
    );
    assert_eq!(ESC_KEYDOWN_ATTRS[1].1, ESC_KEY_TOKEN);
}

#[test]
fn dispatch_effect_は三つの結果を読み替える() {
    assert_eq!(
        dispatch_effect(DispatchOutcome::Dispatched),
        DispatchEffect::Applied
    );
    assert_eq!(
        dispatch_effect(DispatchOutcome::UnknownAction),
        DispatchEffect::Rejected
    );
    assert_eq!(
        dispatch_effect(DispatchOutcome::Reentrant),
        DispatchEffect::Dropped
    );
}

#[test]
fn 設定の既定値() {
    let s = DisplaySettings::default();
    assert_eq!(s.profile.command_prefix.as_str(), "/");
    assert_eq!(s.profile.input_field.as_str(), "shown");
    assert_eq!(s.theme.as_str(), "standard");
    assert_eq!(s.left_pane.world_list.as_str(), "shown");
    assert_eq!(s.left_pane.view_tabs.as_str(), "all");
    assert!(s.left_pane.world_list.is_shown());
    assert!(s.left_pane.view_tabs.shows_buttons());
    assert!(!s.left_pane.view_tabs.limits_to_declared());
}

#[test]
fn 外観は_data_theme_だけで表す() {
    let names: Vec<&str> = Theme::ALL.iter().map(|t| t.as_str()).collect();
    assert_eq!(names, ["standard", "mono", "high_contrast", "reading"]);
    for t in Theme::ALL {
        let s = DisplaySettings {
            theme: t,
            ..Default::default()
        };
        assert_eq!(s.root_attrs(), [("data-theme", t.as_str())]);
        let html = render_component_html(&host(s));
        assert!(
            html.contains(&format!("data-theme=\"{}\"", t.as_str())),
            "{html}"
        );
        assert!(!html.contains("style"), "{html}");
    }
}

#[test]
fn 設定は境界を通して部品から読め_入力欄なしでは候補を描かない() {
    let shown = host(DisplaySettings::default());
    let html = render_component_html(&shown);
    for k in PartKind::ALL {
        assert!(
            html.contains(&format!("{}:/", k.name())),
            "{}: {html}",
            k.name()
        );
    }

    let hidden = host(DisplaySettings {
        profile: OperationProfile {
            command_prefix: CommandPrefix::new("!!").unwrap(),
            input_field: InputField::Hidden,
        },
        ..Default::default()
    });
    let html = render_component_html(&hidden);
    assert!(!html.contains("candidates"), "{html}");
    assert!(html.contains("offers:!!"), "{html}");
    assert!(html.contains("checkpoints:!!"), "{html}");
    let kinds: Vec<PartKind> = hidden
        .registry
        .visible_views(&hidden.model.settings, &hidden.model)
        .into_iter()
        .map(|v| v.0)
        .collect();
    assert_eq!(kinds.len(), 5);
    assert!(!kinds.contains(&PartKind::Candidates));
}

#[test]
fn 接頭辞は文字のノードとして出る() {
    let h = host(DisplaySettings {
        profile: OperationProfile {
            command_prefix: CommandPrefix::new("<b>").unwrap(),
            input_field: InputField::Shown,
        },
        ..Default::default()
    });
    let html = render_component_html(&h);
    assert!(html.contains("&lt;b&gt;"), "{html}");
    assert!(!html.contains("<b>"), "{html}");
}
