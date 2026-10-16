//! 選択肢のクリックの結線（TASK-445 2/2。TERM-24）の試験。
//!
//! ボタンの `data-action`・`data-payload` を `decode_action` に通して `update` に渡し、誤答では拒否の理由と
//! 次の行動が出てボタンが残る（選び直せる）こと、正答では Step が達成になりボタンが消えることを固定する。
//! 判定は core の仕事で、ホストは照合しない（偽造した ID も core が拒否に倒す）。試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::offers::FIELD_OFFERS;
use fandhe_workshop_web::parts::{Component, DirtyTracked};

const MESSAGE: &str = "公開しない場合は、この Step を先へ進めません。";
const NEXT: &str = "公開してよいかをもう一度決めます。";

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn host() -> EngineHost {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    let mut v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    v["nodes"][0]["step"]["confirm"] = serde_json::json!({
        "kind": "answer",
        "options": [
            { "id": "opt-wrong", "correct": false, "label": "誤った案" },
            { "id": "opt-right", "correct": true, "label": "正しい案" }
        ],
        "on_wrong": "rj-decline"
    });
    let ir: Ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    let p = testdata().join("contract/event-sequence.json");
    let s: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let state: SessionState = serde_json::from_value(s["initial"].clone()).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(ir, state), standard_registry()).unwrap();
    h.update(HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    ))));
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
    h
}

/// ボタンが持つ action 名と payload のとおりに送る（実際のクリックと同じ経路）。
fn click(h: &mut EngineHost, id: &str) {
    let action = EngineHost::decode_action("choose", id).unwrap();
    h.update(action);
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn buttons(h: &EngineHost) -> usize {
    html(h).matches("data-testid=\"choice-btn\"").count()
}

fn rejected_rows(h: &EngineHost) -> usize {
    html(h).matches("data-kind=\"rejected\"").count()
}

fn achieved(h: &EngineHost) -> usize {
    let log = h.model().session().unwrap().log();
    log.iter()
        .filter(|e| matches!(e, Event::Achieved { .. }))
        .count()
}

fn current(h: &EngineHost) -> String {
    h.model()
        .session()
        .unwrap()
        .state()
        .run
        .current
        .as_ref()
        .unwrap()
        .0
        .clone()
}

#[test]
fn choose_は_id_を_choice_の_op_にし_空の_payload_は何もしない() {
    assert!(matches!(
        EngineHost::decode_action("choose", "opt-wrong"),
        Some(HostAction::Engine(Op::Input(Input::Choice(id)))) if id == "opt-wrong"
    ));
    assert!(EngineHost::decode_action("choose", "").is_none());
}

#[test]
fn ボタンの_action_と_payload_は選択肢の_id_で_文字には_id_が出ない() {
    let h = host();
    let out = html(&h);
    for id in ["opt-wrong", "opt-right"] {
        assert!(out.contains(&format!("data-payload=\"{id}\"")), "{out}");
        assert!(!out.contains(&format!(">{id}<")), "{out}");
    }
    assert_eq!(out.matches("data-action=\"choose\"").count(), 2, "{out}");
}

#[test]
fn 誤答は理由と次の行動を出し_ボタンは残り_達成にならない() {
    let mut h = host();
    assert_eq!(buttons(&h), 2);
    let step = current(&h);
    click(&mut h, "opt-wrong");
    assert_eq!(rejected_rows(&h), 1, "{}", html(&h));
    let out = html(&h);
    assert!(out.contains(MESSAGE) && out.contains(NEXT), "{out}");
    assert_eq!(buttons(&h), 2, "選び直せる");
    assert_eq!(achieved(&h), 0);
    assert_eq!(current(&h), step);
}

#[test]
fn 誤答の後の正答で達成になりボタンが消える() {
    let mut h = host();
    let step = current(&h);
    click(&mut h, "opt-wrong");
    click(&mut h, "opt-right");
    assert_eq!(achieved(&h), 1);
    assert_ne!(current(&h), step, "次の Step へ進む");
    assert_eq!(buttons(&h), 0);
    assert!(
        h.dirty_fields().contains(&FIELD_OFFERS),
        "{:?}",
        h.dirty_fields()
    );
    let out = html(&h);
    let start = out.find("data-bind-list=\"offer_choices\"").unwrap();
    let tag = &out[out[..start].rfind('<').unwrap()..start + out[start..].find('>').unwrap()];
    assert!(tag.contains("hidden"), "{tag}");
    assert_eq!(rejected_rows(&h), 1, "正答で拒否の行は増えない");
}

#[test]
fn 誤答を重ねても毎回_1_件ずつ出て_その後の正答で達成になる() {
    let mut h = host();
    for n in 1..=3 {
        click(&mut h, "opt-wrong");
        assert_eq!(rejected_rows(&h), n, "{}", html(&h));
        assert_eq!(buttons(&h), 2);
    }
    click(&mut h, "opt-right");
    assert_eq!(achieved(&h), 1);
    assert_eq!(buttons(&h), 0);
}

#[test]
fn 表示していない_id_は_core_が拒否に倒し_達成にならない() {
    let mut h = host();
    let step = current(&h);
    click(&mut h, "opt-forged");
    assert_eq!(achieved(&h), 0);
    assert_eq!(current(&h), step);
    assert_eq!(buttons(&h), 2);
}

#[cfg(feature = "test-hooks")]
mod load_fixture {
    use super::*;
    use fandhe_workshop_web::host::{FixtureName, RAW_LOAD_FIXTURE};

    fn load() -> EngineHost {
        let mut h = EngineHost::new();
        let action =
            EngineHost::decode_action("raw", &format!("{RAW_LOAD_FIXTURE}answer")).unwrap();
        assert!(matches!(
            action,
            HostAction::LoadFixture(FixtureName::Answer)
        ));
        h.update(action);
        h
    }

    #[test]
    fn 合図は_op_としては読めず_未知の名前は何もしない() {
        assert!(serde_json::from_str::<Op>(&format!("{RAW_LOAD_FIXTURE}answer")).is_err());
        for bad in [
            "load-fixture:",
            "load-fixture:unknown",
            "load-fixture:Answer",
        ] {
            assert!(EngineHost::decode_action("raw", bad).is_none(), "{bad}");
        }
        // raw 以外の名前では復号されない。
        for name in ["tick", "esc", "choose"] {
            assert!(
                !matches!(
                    EngineHost::decode_action(name, "load-fixture:answer"),
                    Some(HostAction::LoadFixture(_))
                ),
                "{name}"
            );
        }
    }

    #[test]
    fn 差し替えの後に受付を終えると選択肢が出て_誤答と正答の流れが通る() {
        let mut h = load();
        assert_eq!(current(&h), "main/st-1");
        assert_eq!(buttons(&h), 0);
        h.update(HostAction::Engine(Op::Input(Input::Command(
            "/make-note first-note".to_owned(),
        ))));
        let mut guard = 0;
        while let Some(addr) = h.next_tick() {
            h.update(HostAction::Engine(Op::Tick(addr)));
            guard += 1;
            assert!(guard < 50);
        }
        assert_eq!(buttons(&h), 2, "{}", html(&h));
        click(&mut h, "opt-wrong");
        assert_eq!(rejected_rows(&h), 1);
        assert_eq!(buttons(&h), 2);
        click(&mut h, "opt-right");
        assert_eq!(achieved(&h), 1);
        assert_eq!(buttons(&h), 0);
    }
}
