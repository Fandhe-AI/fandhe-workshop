//! 指せる行のクリックの契約試験（TASK-480 2/2。ART-11）。
//!
//! クリックは `data-action="ui"` と `data-payload` の組として host に届く。ここでは、host の
//! `EngineHost::decode_action` が `PointTarget::decode` だけを通して `Op::Input(Input::Ui)` に変えること、
//! 復号できない payload は何も受け付けないこと、`ui` の受け付けで教材が進むこと、世界状態の名札の経路
//! `screen/top` が行・属性・入力へ往復することを確かめる。宛先が今の Step に合うかの判定は core の
//! `select_judge` の担当で、host は判定しない（`tests/no_progress.rs`）。
//! 製品の画面への結線（`view()` の 7 本の一覧への本体の結線）は TASK-470・TASK-347 の後続の範囲である。
//! 試験では `unwrap` を使ってよい。

use std::collections::BTreeMap;
use std::path::PathBuf;

use fandhe_frontend_core::render;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::step_status::StepStatus;
use fandhe_workshop_core::world::{Element, World};
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::app_demo_view::app_demo_items;
use fandhe_workshop_web::artifact_mark::point_all;
use fandhe_workshop_web::artifact_rows::{derive_rows, DeriveError, RowParts};
use fandhe_workshop_web::artifact_view::{Cell, Phase, PointTarget, RowId, RowKind, Variant};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;

fn target(view: &str, row: &str) -> PointTarget {
    PointTarget::new(view, RowId::new(row).unwrap()).unwrap()
}

fn ui_op(view: &str, path: &str) -> HostAction {
    HostAction::Engine(Op::Input(Input::Ui {
        view: view.to_owned(),
        path: path.to_owned(),
    }))
}

/// `decode_action("ui", payload)` の結果を、`Input::Ui` の (view, path) として取り出す。それ以外は `None`。
fn decoded_ui(payload: &str) -> Option<(String, String)> {
    match EngineHost::decode_action("ui", payload) {
        Some(HostAction::Engine(Op::Input(Input::Ui { view, path }))) => Some((view, path)),
        Some(other) => panic!("ui が Input::Ui 以外になった: {other:?}"),
        None => None,
    }
}

fn pair(view: &str, path: &str) -> Option<(String, String)> {
    Some((view.to_owned(), path.to_owned()))
}

#[test]
fn 正規の_payload_は_ui_の入力になる() {
    let t = target("v-files", "notes/n3");
    assert_eq!(decoded_ui(&t.encode_payload()), pair("v-files", "notes/n3"));
}

#[test]
fn 危険な文字を含む_view_と_row_が往復する() {
    for (view, row) in [
        ("v\"<&>\n", "r/#<b>"),
        ("v-1", "a\"b'c"),
        ("日本語", "経路/子#1"),
        ("x:y", "1:2"),
    ] {
        let t = target(view, row);
        assert_eq!(
            decoded_ui(&t.encode_payload()),
            pair(view, row),
            "{view:?} {row:?}"
        );
    }
}

#[test]
fn 復号できない_payload_は何も受け付けない() {
    let long = format!("4:vvvv{}", "r".repeat(4_100));
    for bad in [
        "",
        "v-files",
        "7:v-files",
        "08:v-filesr",
        "00007:v-filesr",
        "9:v-filesr",
        "1:v",
        ":vr",
        "0:r",
        "-1:vr",
        long.as_str(),
    ] {
        assert_eq!(decoded_ui(bad), None, "{bad:?}");
    }
}

#[test]
fn ui_以外の名前の既存の分岐は変わらない() {
    assert!(matches!(
        EngineHost::decode_action("choose", "opt-a"),
        Some(HostAction::Engine(Op::Input(Input::Choice(ref id)))) if id == "opt-a"
    ));
    assert!(EngineHost::decode_action("choose", "").is_none());
    let t = target("v-files", "notes/n3");
    assert!(EngineHost::decode_action("uix", &t.encode_payload()).is_none());
}

// --- host の更新: 8 Step の教材で、ui を画面の経路で送っても Op を直接送っても同じ結果になる ---

const LAST: &str = "main/st-8";
const FINISH_LIMIT: usize = 100;

fn page() -> EngineHost {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/reload_points_ch2/ir.json");
    let ir = load_ir(&std::fs::read(path).unwrap()).unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../testdata/contract/event-sequence.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap()
}

fn act(name: &str, payload: &str) -> HostAction {
    EngineHost::decode_action(name, payload).unwrap()
}

fn state(h: &EngineHost) -> SessionState {
    h.model().session().unwrap().state().clone()
}

fn status_of(s: &SessionState, step: &StepUid) -> StepStatus {
    s.run
        .slices
        .get(&step.root())
        .and_then(|sl| sl.status.get(step))
        .copied()
        .unwrap_or_default()
}

fn done(s: &SessionState) -> bool {
    s.run.completed || status_of(s, &StepUid(LAST.to_owned())) == StepStatus::Achieved
}

/// `ui` の手を、画面のクリック（`decode_action("ui", ..)`）で送るか、`Op` を直接送るか。
#[derive(Clone, Copy, PartialEq, Eq)]
enum UiRoute {
    Click,
    Direct,
}

fn send_ui(h: &mut EngineHost, route: UiRoute, path: &str) {
    match route {
        UiRoute::Click => {
            let t = target("v-files", path);
            h.update(act("ui", &t.encode_payload()));
        }
        UiRoute::Direct => h.update(ui_op("v-files", path)),
    }
}

/// 誘導つきの次の 1 手。`reload_points_host.rs` の `step` と同じ手順で、`ui` の送り方だけを差し替える。
fn step(h: &mut EngineHost, route: UiRoute) -> bool {
    let s = state(h);
    if done(&s) {
        return false;
    }
    if let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        return true;
    }
    let current = s.run.current.clone().unwrap();
    let n: usize = current.0.strip_prefix("main/st-").unwrap().parse().unwrap();
    let accepted = status_of(&s, &current) == StepStatus::Accepted;
    let path = format!("notes/n{n}");
    let submit = |h: &mut EngineHost, text: &str| {
        h.update(act("draft", text));
        h.update(act("submit", ""));
    };
    if accepted {
        match n {
            2 | 7 => h.update(act("choose", "opt-a")),
            6 => h.update(act("decide", "approve")),
            _ => send_ui(h, route, &path),
        }
    } else {
        match n {
            2 => submit(h, "メモ 2"),
            3 | 8 => h.update(act("decide", "approve")),
            4 | 5 => h.update(HostAction::Engine(Op::Input(Input::Choice("opt-a".into())))),
            6 => h.update(act("revise-target", "expect")),
            7 => send_ui(h, route, &path),
            _ => submit(h, &format!("/make-note note-{n}")),
        }
    }
    true
}

fn finish(route: UiRoute) -> EngineHost {
    let mut h = page();
    for _ in 0..FINISH_LIMIT {
        if !step(&mut h, route) {
            return h;
        }
    }
    panic!("did not finish within {FINISH_LIMIT} steps");
}

#[test]
fn クリックの_ui_で完走した状態は_op_を直接送った状態と一致する() {
    let clicked = finish(UiRoute::Click);
    let direct = finish(UiRoute::Direct);
    assert!(done(&state(&clicked)));
    assert_eq!(state(&clicked), state(&direct));
}

#[test]
fn 誤った経路のクリックでは_ui_を待つ_step_が達成にならない() {
    // Step 7 は `ui` を待つ。手前まで進め、誤った経路をクリックしても達成にならないことを確かめる。
    let mut h = page();
    for _ in 0..FINISH_LIMIT {
        let s = state(&h);
        let at7 = s.run.current.as_ref().is_some_and(|c| c.0 == "main/st-7")
            && h.next_tick().is_none()
            && status_of(&s, &StepUid("main/st-7".into())) != StepStatus::Accepted;
        if at7 {
            break;
        }
        assert!(step(&mut h, UiRoute::Click), "Step 7 に着く前に完走した");
    }
    let before = state(&h);
    let current = before.run.current.clone().unwrap();
    assert_eq!(current.0, "main/st-7");
    let wrong = target("v-files", "no/such/path");
    h.update(act("ui", &wrong.encode_payload()));
    let other_view = target("v-other", "notes/n7");
    h.update(act("ui", &other_view.encode_payload()));
    let after = state(&h);
    assert_ne!(status_of(&after, &current), StepStatus::Achieved);
    assert_ne!(status_of(&after, &current), StepStatus::Accepted);
}

// --- 世界状態の名札から入力までの往復 ---

fn nameplate_world() -> World {
    let mut w = BTreeMap::new();
    for (path, title, role) in [
        ("screen/top", "上部（教材の名前と、今の位置）", "heading"),
        (
            "screen/left",
            "左（成果物。変わった所に印が付きます）",
            "item",
        ),
        ("screen/right", "右（操作。ここに入力します）", "item"),
    ] {
        let mut attrs = BTreeMap::new();
        attrs.insert("title".to_owned(), title.to_owned());
        attrs.insert("role".to_owned(), role.to_owned());
        w.insert(
            path.to_owned(),
            Element {
                fixture: Some("fx-screen-part".to_owned()),
                attrs,
            },
        );
    }
    w
}

fn read_screen(_: &str, e: &Element) -> Result<Option<RowParts<Cell>>, DeriveError> {
    if e.fixture.as_deref() != Some("fx-screen-part") {
        return Ok(None);
    }
    let title = e.attrs.get("title").ok_or(DeriveError::Element)?;
    let kind = match e.attrs.get("role").map(String::as_str) {
        Some("heading") => RowKind::Heading,
        Some("item") => RowKind::Item,
        _ => return Err(DeriveError::Element),
    };
    Ok(Some(RowParts {
        depth: 0,
        kind,
        content: Cell {
            label: title.clone(),
            detail: String::new(),
            state: String::new(),
            tags: vec![],
        },
    }))
}

#[test]
fn 世界状態の名札は行と属性と入力へ往復する() {
    let rows = derive_rows(&nameplate_world(), &[] as &[Event], read_screen).unwrap();
    assert_eq!(rows.len(), 3);
    // 経路が行の識別子になる（`Input::Ui.path` と一致する不変条件）。
    assert!(rows.iter().any(|r| r.id().as_str() == "screen/top"));
    // `screen/top` だけを宛先にする（左・右は表示だけ）。
    let only_top = |id: &RowId| (id.as_str() == "screen/top").then(|| target("v-app", id.as_str()));
    for variant in [Variant::A, Variant::B] {
        for phase in [Phase::Before, Phase::After] {
            let items = app_demo_items(&rows, phase, variant, &only_top);
            assert_eq!(items.len(), 3);
            let html: String = items.iter().map(|(_, n)| render(n)).collect();
            assert_eq!(html.matches("data-row=\"screen/top\"").count(), 1, "{html}");
            assert_eq!(html.matches("data-row=").count(), 1, "{html}");
            assert_eq!(html.matches("data-action=\"ui\"").count(), 1, "{html}");
            assert_eq!(html.matches("tabindex=\"0\"").count(), 1, "{html}");
            // 属性の payload を host に通すと、世界状態の経路を指す入力になる。
            let payload = format!("{}:{}{}", "v-app".len(), "v-app", "screen/top");
            assert!(
                html.contains(&format!("data-payload=\"{payload}\"")),
                "{html}"
            );
            assert_eq!(decoded_ui(&payload), pair("v-app", "screen/top"));
        }
    }
    // 全行を宛先にすると、名札の 3 行が全部指せる行になる（属性の組が行ごとに付く）。
    let all = point_all("v-app");
    let items = app_demo_items(&rows, Phase::Before, Variant::B, &all);
    let html: String = items.iter().map(|(_, n)| render(n)).collect();
    assert_eq!(html.matches("data-row=").count(), 3, "{html}");
}
