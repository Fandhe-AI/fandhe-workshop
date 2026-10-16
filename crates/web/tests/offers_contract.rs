//! `answer` の選択肢のボタン（`offers`。TASK-445 1/2。TERM-24）の契約の試験。
//!
//! 確認待ちのときだけ、表示名（`label`、無ければ ID）を文字にしたボタンが出ること、それ以外では出ないこと
//! （fail-closed）、教材由来の文字列がエスケープされることを固定する。クリックの結線は `choice_answer.rs` が確かめる（TASK-445 2/2）。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_wasm_full::render_component_html;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::offers::{answer_offers, choice_button, FIELD_OFFERS};
use fandhe_workshop_web::parts::{Component, DirtyTracked, PartKind};
use fandhe_workshop_web::terminal_area::check_area;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

fn base_json() -> serde_json::Value {
    let p = testdata().join("engine-event-consistency/test-data/min-3step.json");
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn initial() -> SessionState {
    let p = testdata().join("contract/event-sequence.json");
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

fn options() -> serde_json::Value {
    serde_json::json!([
        { "id": "opt-wrong", "correct": false, "label": "間違いの案" },
        { "id": "opt-right", "correct": true, "label": "正しい案" },
        { "id": "opt-plain", "correct": false },
        { "id": "opt-xss", "correct": false, "label": "<script>alert(1)</script>" }
    ])
}

fn host_with_confirm(confirm: Option<serde_json::Value>) -> EngineHost {
    let mut v = base_json();
    if let Some(c) = confirm {
        v["nodes"][0]["step"]["confirm"] = c;
    }
    let ir: Ir = load_ir(&serde_json::to_vec(&v).unwrap()).unwrap();
    EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap()
}

fn answer(options: serde_json::Value) -> serde_json::Value {
    serde_json::json!({ "kind": "answer", "options": options, "on_wrong": "rj-decline" })
}

fn html(h: &EngineHost) -> String {
    render_component_html(h)
}

fn buttons(h: &EngineHost) -> usize {
    html(h).matches("data-testid=\"choice-btn\"").count()
}

fn make_note() -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

/// 処理中でなくなるまで進行を届ける。
fn run_ticks(h: &mut EngineHost) {
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        guard += 1;
        assert!(guard < 50, "進行が終わらない");
    }
}

fn confirmed_host() -> EngineHost {
    let mut h = host_with_confirm(Some(answer(options())));
    h.update(make_note());
    run_ticks(&mut h);
    h
}

#[test]
fn 受付の前はボタンが無く_親は_hidden_で一つだけ描かれる() {
    let h = host_with_confirm(Some(answer(options())));
    let out = html(&h);
    assert_eq!(buttons(&h), 0, "{out}");
    assert_eq!(out.matches("data-bind-list=\"offer_choices\"").count(), 1);
    let start = out.find("data-bind-list=\"offer_choices\"").unwrap();
    let tag_start = out[..start].rfind('<').unwrap();
    let tag_end = start + out[start..].find('>').unwrap();
    assert!(out[tag_start..tag_end].contains("hidden"), "{out}");
}

#[test]
fn 処理中はボタンが無く_処理が終わると選択肢の数だけ出る() {
    let mut h = host_with_confirm(Some(answer(options())));
    h.update(make_note());
    assert_eq!(buttons(&h), 0, "受付済みでも処理中は出さない");
    let mut last_dirty = Vec::new();
    let mut guard = 0;
    while let Some(addr) = h.next_tick() {
        h.update(HostAction::Engine(Op::Tick(addr)));
        last_dirty = h.dirty_fields().to_vec();
        guard += 1;
        assert!(guard < 50);
    }
    assert!(last_dirty.contains(&FIELD_OFFERS), "{last_dirty:?}");
    assert_eq!(buttons(&h), 4, "{}", html(&h));
}

#[test]
fn ボタンの文字は表示名で_label_があれば_id_を文字として出さない() {
    let h = confirmed_host();
    let offers = answer_offers(h.model().session().unwrap());
    assert_eq!(offers.len(), 4);
    assert_eq!(offers[0].label, "間違いの案");
    assert_eq!(offers[1].label, "正しい案");
    let out = html(&h);
    for id in ["opt-wrong", "opt-right", "opt-xss"] {
        assert!(out.contains(&format!("data-choice=\"{id}\"")), "{out}");
        assert!(!out.contains(&format!(">{id}<")), "{id}: {out}");
    }
    assert!(out.contains(">間違いの案<") && out.contains(">正しい案<"));
    // label の無い選択肢は ID がそのまま見える（contract の規則）。
    assert!(out.contains(">opt-plain<"), "{out}");
    // クリックの結線（2/2）: ID は属性値にだけ入る。
    for id in ["opt-wrong", "opt-right"] {
        assert!(out.contains(&format!("data-payload=\"{id}\"")), "{out}");
    }
    assert_eq!(out.matches("data-action=\"choose\"").count(), 4, "{out}");
}

#[test]
fn script_を含む_label_はエスケープされる() {
    let out = html(&confirmed_host());
    assert!(!out.contains("<script"), "{out}");
    assert!(out.contains("&lt;script"), "{out}");
}

#[test]
fn 描いた_li_は端末エリアの検査を通り_親の識別子は無い() {
    let h = confirmed_host();
    let view = h.registry().view(PartKind::Offers, h.model()).unwrap();
    assert!(check_area(&view).is_ok(), "{:?}", check_area(&view));
    assert!(check_area(&choice_button("a", "A")).is_ok());
    assert!(check_area(&choice_button("a", "")).is_err());
    assert!(check_area(&choice_button("", "A")).is_err());
}

#[test]
fn 表示しない場合はボタンが無い() {
    // 確認が answer でない。
    let mut h = host_with_confirm(None);
    h.update(make_note());
    run_ticks(&mut h);
    assert_eq!(buttons(&h), 0);
    // 正答が無い・選択肢が空。
    for opts in [
        serde_json::json!([{ "id": "a", "correct": false }]),
        serde_json::json!([]),
    ] {
        let mut h = host_with_confirm(Some(answer(opts)));
        h.update(make_note());
        run_ticks(&mut h);
        assert_eq!(buttons(&h), 0, "{}", html(&h));
    }
    // session が無い。
    assert_eq!(buttons(&EngineHost::new()), 0);
}

#[test]
fn 達成して次の_step_へ移るとボタンは消える() {
    let mut h = confirmed_host();
    assert_eq!(buttons(&h), 4);
    h.update(HostAction::Engine(Op::Input(Input::Choice(
        "opt-right".to_owned(),
    ))));
    assert_eq!(buttons(&h), 0, "{}", html(&h));
}

#[test]
fn 標準の登録簿に_offers_が一つあり_field_は入力欄と重ならない() {
    let r = standard_registry();
    assert!(r
        .view(PartKind::Offers, EngineHost::new().model())
        .is_some());
    assert_eq!(r.fields().filter(|f| f.name == FIELD_OFFERS).count(), 1);
    assert!(!["input_notice", "learner_log", "input_candidates"].contains(&FIELD_OFFERS));
}
