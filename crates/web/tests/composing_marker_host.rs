//! 自前の変換中の印（JUDGE-14・JUDGE-16。TASK-276 1/3）がホストの読み取り口 `ComposingProbe` 経由で
//! Enter・Esc の判定に届くことの native 試験。
//!
//! glue は wasm32 専用で native では試せないため、glue が呼ぶ `EngineHost::key_field_state` と、Esc の経路
//! （`HostAction::Input(HelloAction::Esc)`）をここで確かめる。印を外す・口を結ばない変異の検出先になる（TASK-276 3/3）。
//! 変換中の補完は適用せず書き戻しも頼まない（#2121。c8・c9）。
//! 試験では `unwrap` を使ってよい。

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_interactive::Component;
use fandhe_workshop_contract::key_decide::{decide, KeyInput};
use fandhe_workshop_core::addr::{Generation, StepUid, TickAddr};
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::input::{Input, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::key_route::effect_for;

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// 印の口を `flag` に結んだホスト（`None` なら結ばない）。
fn host_with(flag: Option<&Rc<Cell<bool>>>) -> EngineHost {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    let sample: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    let state: SessionState = serde_json::from_value(sample["initial"].clone()).unwrap();
    let session = Session::from_state(load_ir(&bytes).unwrap(), state);
    let mut host = EngineHost::with_session(session, standard_registry()).unwrap();
    if let Some(flag) = flag {
        let flag = Rc::clone(flag);
        host.set_composing_probe(Box::new(move || flag.get()));
    }
    host
}

fn busy(host: &mut EngineHost) {
    host.update(HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    ))));
    host.update(HostAction::Engine(Op::Tick(TickAddr {
        generation: Generation(1),
        step: StepUid::parse("main/st-1").unwrap(),
        index: 0,
    })));
    assert!(host.model().session().unwrap().state().run.busy.is_some());
}

fn draft(host: &mut EngineHost, text: &str) {
    host.update(EngineHost::decode_action("draft", text).expect("draft"));
}

fn esc(host: &mut EngineHost) {
    host.update(EngineHost::decode_action("esc", "Escape").expect("esc"));
}

fn generation(host: &EngineHost) -> u64 {
    host.model().session().unwrap().state().run.generation.0
}

fn log_len(host: &EngineHost) -> usize {
    host.model().session().unwrap().log().len()
}

fn plain_enter() -> KeyInput {
    KeyInput {
        key: "Enter".to_owned(),
        key_code: 13,
        is_composing: false,
        shift: false,
        ctrl: false,
        alt: false,
        meta: false,
    }
}

#[test]
fn c1_印が立つと処理中の_esc_でも世代とログが変わらない() {
    let flag = Rc::new(Cell::new(true));
    let mut host = host_with(Some(&flag));
    draft(&mut host, "/he");
    busy(&mut host);
    let (gen0, len0) = (generation(&host), log_len(&host));
    esc(&mut host);
    assert_eq!(generation(&host), gen0);
    assert_eq!(log_len(&host), len0);
    assert!(host.model().session().unwrap().state().run.busy.is_some());
    assert!(host.candidates_open(), "候補も閉じない");
}

#[test]
fn c2_対照_印が下りていれば処理中の_esc_は停止の行を1件足す() {
    for flag in [Some(Rc::new(Cell::new(false))), None] {
        let mut host = host_with(flag.as_ref());
        busy(&mut host);
        let (gen0, len0) = (generation(&host), log_len(&host));
        esc(&mut host);
        assert_eq!(generation(&host), gen0 + 1);
        let stopped = host.model().session().unwrap().log()[len0..]
            .iter()
            .filter(|e| matches!(e, Event::Stopped { .. }))
            .count();
        assert_eq!(stopped, 1);
    }
}

#[test]
fn c3_印が立つと待機中の_esc_は取り消しの拒否を足さない() {
    let flag = Rc::new(Cell::new(true));
    let mut host = host_with(Some(&flag));
    draft(&mut host, "下書き");
    let len0 = log_len(&host);
    esc(&mut host);
    assert_eq!(log_len(&host), len0);
    assert_eq!(host.draft(), "下書き");
    flag.set(false);
    esc(&mut host);
    assert!(
        log_len(&host) > len0,
        "対照: 印が下りれば取り消しが記録される"
    );
}

#[test]
fn c4_印が立つと待機中の_esc_は候補を閉じない() {
    let flag = Rc::new(Cell::new(true));
    let mut host = host_with(Some(&flag));
    draft(&mut host, "/he");
    assert!(host.candidates_open());
    esc(&mut host);
    assert!(host.candidates_open());
    flag.set(false);
    esc(&mut host);
    assert!(!host.candidates_open(), "対照: 印が下りれば閉じる");
}

#[test]
fn c5_key_field_state_は口の値と下書きに従う() {
    let flag = Rc::new(Cell::new(false));
    let mut host = host_with(Some(&flag));
    assert!(!host.key_field_state().composing_marker);
    draft(&mut host, "/he");
    flag.set(true);
    let s = host.key_field_state();
    assert!(s.composing_marker);
    assert!(s.has_text);
    assert_eq!(s.candidates_open, host.candidates_open());
    assert!(!host_with(None).key_field_state().composing_marker);
}

#[test]
fn c6_c7_印が立つ_enter_は送信せず_立たなければ送信する() {
    for text in ["下書き", "一行目\n二行目"] {
        let flag = Rc::new(Cell::new(true));
        let mut host = host_with(Some(&flag));
        draft(&mut host, text);
        let e = effect_for(decide(&plain_enter(), &host.key_field_state()));
        assert!(!e.click_submit && !e.prevent_default, "{text:?}");
        flag.set(false);
        let e = effect_for(decide(&plain_enter(), &host.key_field_state()));
        assert!(e.click_submit && e.prevent_default, "{text:?}");
    }
}

/// 書き戻しの通知の回数を数える口を付ける。
fn with_sync(host: &mut EngineHost) -> Rc<Cell<u32>> {
    let n = Rc::new(Cell::new(0));
    let c = Rc::clone(&n);
    host.set_draft_sync_request(Box::new(move || c.set(c.get() + 1)));
    n
}

#[test]
fn c8_印が立つと補完は下書きを変えず書き戻しも頼まない() {
    let flag = Rc::new(Cell::new(true));
    let mut host = host_with(Some(&flag));
    let sync = with_sync(&mut host);
    draft(&mut host, "/he");
    host.update(EngineHost::decode_action("complete", "").expect("complete"));
    assert_eq!(host.draft(), "/he");
    assert_eq!(sync.get(), 0);
}

#[test]
fn c9_対照_印が下りていれば補完は置き換え書き戻しを1回頼む() {
    let flag = Rc::new(Cell::new(false));
    let mut host = host_with(Some(&flag));
    let sync = with_sync(&mut host);
    draft(&mut host, "/he");
    host.update(EngineHost::decode_action("complete", "").expect("complete"));
    assert_eq!(host.draft(), "/help");
    assert_eq!(sync.get(), 1);
}
