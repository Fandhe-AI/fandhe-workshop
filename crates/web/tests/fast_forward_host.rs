//! 早送りの入・切の切り替えと状態の文字（SHELL-24。TASK-540 1/2）と、入・切で履歴が変わらないこと（SHELL-27）の host の層の試験。
//!
//! タイマーは試験の側で模す。ホストが通知した最後の宛先を届け、通知の第 2 引数（待たない印）と `tick_reset` の回数を記録する。
//! 通常の速さ・入・途中の切り替えの 3 通りで、最後の状態（ログを含む）と保存する履歴が一致することを固定する。
//! ブラウザでの押下・間隔は `tools/browser/fast-forward-toggle.test.mjs` の担当である。試験では `unwrap` を使ってよい。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_core::addr::TickAddr;
use fandhe_workshop_core::event::Event;
use fandhe_workshop_core::history_envelope::build_envelope;
use fandhe_workshop_core::history_export::export_bytes;
use fandhe_workshop_core::history_load::{parse_history_envelope, parse_history_json};
use fandhe_workshop_core::history_replay::replay;
use fandhe_workshop_core::input::{Control, Input, Op};
use fandhe_workshop_core::{load_ir, Session, SessionState};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::fast_forward_ops::{
    ACTION_TOGGLE_FAST_FORWARD, ACTIVE_TESTID, FAST_FORWARD_FIELD, FAST_FORWARD_LIMIT, OFF_LABEL,
    ON_LABEL, STATE_ATTR, STATUS_TESTID, TOGGLE_TESTID,
};
use fandhe_workshop_web::host::{EngineHost, HostAction, ImportOutcome};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::{self, ControlSection};

fn testdata() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../testdata")
}

/// `min-3step` の IR の JSON。`version` が空でなければ `pack.version` を書き換える（表示だけの履歴を作るため）。
fn ir_json(version: &str) -> Vec<u8> {
    let bytes = std::fs::read(testdata().join("engine-event-consistency/test-data/min-3step.json"))
        .unwrap();
    if version.is_empty() {
        return bytes;
    }
    let mut v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    v["pack"]["version"] = serde_json::json!(version);
    v["pack"]["vocab"] = serde_json::json!("1.2");
    serde_json::to_vec(&v).unwrap()
}

fn initial() -> SessionState {
    let v: serde_json::Value = serde_json::from_slice(
        &std::fs::read(testdata().join("contract/event-sequence.json")).unwrap(),
    )
    .unwrap();
    serde_json::from_value(v["initial"].clone()).unwrap()
}

/// 通知の記録（宛先と待たない印）。
type Notes = Rc<RefCell<Vec<(Option<TickAddr>, bool)>>>;

/// 模したタイマー。通知の記録と、`tick_reset` の回数。
struct Timer {
    notes: Notes,
    resets: Rc<Cell<usize>>,
}

impl Timer {
    fn last(&self) -> (Option<TickAddr>, bool) {
        self.notes.borrow().last().cloned().unwrap()
    }
}

fn host_at(version: &str) -> (EngineHost, Timer) {
    let ir = load_ir(&ir_json(version)).unwrap();
    let mut h =
        EngineHost::with_session(Session::from_state(ir, initial()), standard_registry()).unwrap();
    let notes: Notes = Rc::default();
    let resets: Rc<Cell<usize>> = Rc::default();
    let n = Rc::clone(&notes);
    h.set_tick_request(Box::new(move |next, fast| {
        n.borrow_mut().push((next, fast))
    }));
    let r = Rc::clone(&resets);
    h.set_tick_reset(Box::new(move || r.set(r.get() + 1)));
    (h, Timer { notes, resets })
}

fn host() -> (EngineHost, Timer) {
    host_at("")
}

fn make_note() -> HostAction {
    HostAction::Engine(Op::Input(Input::Command(
        "/make-note first-note".to_owned(),
    )))
}

fn toggle(h: &mut EngineHost) {
    h.update(EngineHost::decode_action(ACTION_TOGGLE_FAST_FORWARD, "").unwrap());
}

/// 最後に通知された宛先のタイマーが鳴ったものとして届ける。宛先が無ければ何もしない。
fn fire(h: &mut EngineHost, t: &Timer) {
    if let (Some(a), _) = t.last() {
        h.update(HostAction::Engine(Op::Tick(a)));
    }
}

fn fire_until_idle(h: &mut EngineHost, t: &Timer) {
    for _ in 0..50 {
        if h.next_tick().is_none() {
            return;
        }
        fire(h, t);
    }
    panic!("処理中が解けない");
}

fn progress_count(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Progress { .. }))
        .count()
}

fn ignored_count(h: &EngineHost) -> usize {
    h.model()
        .session()
        .unwrap()
        .log()
        .iter()
        .filter(|e| matches!(e, Event::Ignored { .. }))
        .count()
}

/// 比べる最後の結果（状態とログ、保存する履歴）。
fn outcome(h: &EngineHost) -> (String, Option<String>) {
    (
        serde_json::to_string(h.model().session().unwrap().state()).unwrap(),
        h.history_to_save(),
    )
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

fn find<'a>(n: &'a Node, testid: &str, out: &mut Vec<&'a Node>) {
    if attr(n, "data-testid") == Some(testid) {
        out.push(n);
    }
    for c in kids(n) {
        find(c, testid, out);
    }
}

fn count(n: &Node, testid: &str) -> usize {
    let mut v = Vec::new();
    find(n, testid, &mut v);
    v.len()
}

fn text_of(n: &Node) -> String {
    match n {
        Node::Text(t) => t.clone(),
        _ => kids(n).iter().map(text_of).collect(),
    }
}

/// 状態の文字（1 個のはず）と、その `data-fast-forward` の値。
fn status(h: &EngineHost) -> (String, String) {
    let v = h.view();
    let mut found = Vec::new();
    find(&v, STATUS_TESTID, &mut found);
    assert_eq!(found.len(), 1);
    (
        text_of(found[0]),
        attr(found[0], STATE_ATTR).unwrap().to_owned(),
    )
}

#[test]
fn 復号は空の_payload_だけを受ける() {
    assert!(matches!(
        EngineHost::decode_action(ACTION_TOGGLE_FAST_FORWARD, ""),
        Some(HostAction::ToggleFastForward)
    ));
    for p in ["1", " ", "on", "off", "<b>"] {
        assert!(
            EngineHost::decode_action(ACTION_TOGGLE_FAST_FORWARD, p).is_none(),
            "{p:?}"
        );
    }
}

#[test]
fn 開いた直後は切でボタン1個と早送り中0個で検査器を通る() {
    let (h, _) = host();
    let v = h.view();
    assert_eq!(status(&h), (OFF_LABEL.to_owned(), "off".to_owned()));
    assert_eq!(count(&v, ControlSection::FastForward.testid()), 1);
    assert_eq!(count(&v, TOGGLE_TESTID), 1);
    assert_eq!(count(&v, ACTIVE_TESTID), 0);
    assert!(shell::check_complete_v2(&v).is_ok());
    assert!(a11y::check_no_undefined(&v).is_ok());
}

#[test]
fn 切り替えで入と切が替わり_dirty_は早送りの区画だけ() {
    let (mut h, _) = host();
    toggle(&mut h);
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
    assert_eq!(h.dirty_fields(), &[FAST_FORWARD_FIELD.name]);
    toggle(&mut h);
    assert_eq!(status(&h), (OFF_LABEL.to_owned(), "off".to_owned()));
    assert_eq!(h.dirty_fields(), &[FAST_FORWARD_FIELD.name]);
}

#[test]
fn 切り替えは_ops_にも保存にも出ない() {
    let (mut h, _) = host();
    assert_eq!(h.history_to_save(), None);
    toggle(&mut h);
    assert_eq!(
        h.history_to_save(),
        None,
        "切り替えだけでは履歴が生まれない"
    );
    h.update(make_note());
    let saved = h.history_to_save();
    let ops = h.model().session().unwrap().log().len();
    toggle(&mut h);
    toggle(&mut h);
    assert_eq!(h.history_to_save(), saved);
    assert_eq!(h.model().session().unwrap().log().len(), ops);
}

#[test]
fn 入で1件届くと上限の数だけ進み次で終わる() {
    let (mut h, t) = host();
    toggle(&mut h);
    h.update(make_note());
    assert!(t.last().1, "入の間は待たない印で通知する");
    fire(&mut h, &t);
    assert_eq!(progress_count(&h), FAST_FORWARD_LIMIT as usize);
    assert!(h.next_tick().is_some(), "上限で区切り、処理中のまま残る");
    assert_eq!(t.last(), (h.next_tick(), true));
    fire(&mut h, &t);
    assert_eq!(progress_count(&h), 5);
    assert_eq!(h.next_tick(), None);
    assert_eq!(t.last(), (None, true));
}

#[test]
fn 通常の速さと入と途中の切り替えで最後の状態と履歴が一致する() {
    let normal = {
        let (mut h, t) = host();
        h.update(make_note());
        fire_until_idle(&mut h, &t);
        assert_eq!(t.notes.borrow().iter().filter(|n| n.1).count(), 0);
        outcome(&h)
    };
    let fast = {
        let (mut h, t) = host();
        toggle(&mut h);
        h.update(make_note());
        fire_until_idle(&mut h, &t);
        outcome(&h)
    };
    let switched = {
        let (mut h, t) = host();
        h.update(make_note());
        fire(&mut h, &t);
        toggle(&mut h);
        fire(&mut h, &t);
        toggle(&mut h);
        fire_until_idle(&mut h, &t);
        outcome(&h)
    };
    assert!(normal.1.is_some());
    assert_eq!(normal, fast, "SHELL-27");
    assert_eq!(normal, switched, "SHELL-27");
}

#[test]
fn 区切りの間の停止は通常の速さと同じ結果になる() {
    let stop_at_four = |fast: bool, esc: bool| {
        let (mut h, t) = host();
        if fast {
            toggle(&mut h);
        }
        h.update(make_note());
        if fast {
            fire(&mut h, &t);
        } else {
            for _ in 0..FAST_FORWARD_LIMIT {
                fire(&mut h, &t);
            }
        }
        // 区切りの後に予約された宛先（停止の後に届く古い宛先）。
        let stale = t.last().0.unwrap();
        if esc {
            h.update(EngineHost::decode_action("esc", "Escape").unwrap());
        } else {
            h.update(HostAction::Engine(Op::Control(Control::Stop)));
        }
        assert_eq!(h.next_tick(), None, "停止で処理中が解ける");
        h.update(HostAction::Engine(Op::Tick(stale)));
        assert_eq!(ignored_count(&h), 1);
        outcome(&h)
    };
    for esc in [false, true] {
        assert_eq!(
            stop_at_four(false, esc),
            stop_at_four(true, esc),
            "esc={esc}"
        );
    }
}

#[test]
fn 切り替えのたびに予約を捨て今の宛先を新しい印で通知する() {
    let (mut h, t) = host();
    h.update(make_note());
    let resets = t.resets.get();
    toggle(&mut h);
    assert_eq!(t.resets.get(), resets + 1);
    assert_eq!(t.last(), (h.next_tick(), true));
    toggle(&mut h);
    assert_eq!(t.resets.get(), resets + 2);
    assert_eq!(t.last(), (h.next_tick(), false));
    // 処理中でなくても切り替えられ、宛先なしを通知する。
    fire_until_idle(&mut h, &t);
    toggle(&mut h);
    assert_eq!(t.last(), (None, true));
}

#[test]
fn 入でも古い宛先は_ignored_1件だけで状態を変えない() {
    let (mut h, t) = host();
    toggle(&mut h);
    h.update(make_note());
    let stale = t.last().0.unwrap();
    h.update(HostAction::Engine(Op::Control(Control::Stop)));
    let run = h.model().session().unwrap().state().run.clone();
    h.update(HostAction::Engine(Op::Tick(stale)));
    assert_eq!(ignored_count(&h), 1);
    assert_eq!(progress_count(&h), 0);
    assert_eq!(h.model().session().unwrap().state().run, run);
}

#[test]
fn 早送り中は入かつ処理中だけ出る() {
    let (mut h, t) = host();
    toggle(&mut h);
    assert_eq!(count(&h.view(), ACTIVE_TESTID), 0, "入でも処理中でない");
    toggle(&mut h);
    h.update(make_note());
    assert_eq!(count(&h.view(), ACTIVE_TESTID), 0, "処理中でも切");
    toggle(&mut h);
    assert_eq!(count(&h.view(), ACTIVE_TESTID), 1);
    assert!(h.dirty_fields().contains(&FAST_FORWARD_FIELD.name));
    fire_until_idle(&mut h, &t);
    assert_eq!(count(&h.view(), ACTIVE_TESTID), 0, "処理中が解けると消える");
    assert!(h.dirty_fields().contains(&FAST_FORWARD_FIELD.name));
}

/// `version` の教材で書いた履歴（`ops` は空でない）。
fn history(version: &str) -> Vec<u8> {
    let ir = load_ir(&ir_json(version)).unwrap();
    let sample = std::fs::read(testdata().join("contract/history/two-rounds.json")).unwrap();
    let env = parse_history_envelope(parse_history_json(&sample).unwrap()).unwrap();
    let state = replay(&ir, initial(), &env.ops).unwrap();
    let built = build_envelope(&ir, &env.start, env.ops, state).unwrap();
    export_bytes(&built).unwrap()
}

#[test]
fn 表示だけの間は切り替えを捨てボタンを出さない() {
    let (mut h, t) = host_at("1.0.0");
    assert!(matches!(
        h.open_history(&history("2.0.0")),
        ImportOutcome::ViewOnly(_)
    ));
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 0);
    let notes = t.notes.borrow().len();
    toggle(&mut h);
    assert!(h.dirty_fields().is_empty());
    assert_eq!(status(&h), (OFF_LABEL.to_owned(), "off".to_owned()));
    assert_eq!(t.notes.borrow().len(), notes, "通知もしない");
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 0);
}
