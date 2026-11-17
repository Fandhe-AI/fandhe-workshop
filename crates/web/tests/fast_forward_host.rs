//! 早送りの入・切の切り替えと状態の文字（SHELL-24。TASK-540 1/2）と、入・切で履歴が変わらないこと（SHELL-27）の host の層の試験。
//!
//! タイマーは試験の側で模す。ホストが通知した最後の宛先を届け、通知の第 2 引数（待たない印）と `tick_reset` の回数を記録する。
//! 通常の速さ・入・途中の切り替えの 3 通りで、最後の状態（ログを含む）と保存する履歴が一致することを固定する。
//! 教材の `pack.fast_forward` による初期値（SHELL-25・SESS-3）と、作り直すと初期値に戻ることも固定する。
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
    ON_LABEL, PINNED_LABEL, PINNED_TESTID, STATE_ATTR, STATUS_TESTID, TOGGLE_TESTID,
};
use fandhe_workshop_web::host::{BootKind, EngineHost, HostAction, ImportOutcome};
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
    host_with(&ir_json(version))
}

fn host_with(ir_bytes: &[u8]) -> (EngineHost, Timer) {
    let ir = load_ir(ir_bytes).unwrap();
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

/// `st-2`（選択）に処理 3 件と `normal_speed: true` を差した `min-3step`（SHELL-26）。
fn pinned_host() -> (EngineHost, Timer) {
    let mut v: serde_json::Value = serde_json::from_slice(&ir_json("")).unwrap();
    v["nodes"][1]["step"]["processing"] = serde_json::json!([
        {"name": "q1", "effects": []}, {"name": "q2", "effects": []}, {"name": "q3", "effects": []}
    ]);
    v["nodes"][1]["step"]["normal_speed"] = serde_json::json!(true);
    host_with(&serde_json::to_vec(&v).unwrap())
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

/// st-1 を最後まで進めて確認し、固定の `st-2` の入力待ちに入る。
fn enter_pinned(h: &mut EngineHost, t: &Timer) {
    h.update(make_note());
    fire_until_idle(h, t);
    h.update(HostAction::Engine(Op::Input(Input::Ui {
        view: "v-files".to_owned(),
        path: "notes/draft".to_owned(),
    })));
}

fn choose(h: &mut EngineHost) {
    h.update(HostAction::Engine(Op::Input(Input::Choice(
        "opt-memo".to_owned(),
    ))));
}

#[test]
fn 固定のstepは入のままでも1件ずつ進みボタン0文言1早送り中0() {
    let (mut h, t) = pinned_host();
    toggle(&mut h);
    assert_eq!(count(&h.view(), PINNED_TESTID), 0, "st-1 は固定でない");
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 1);
    enter_pinned(&mut h, &t);
    assert_eq!(count(&h.view(), PINNED_TESTID), 1, "入力待ちの間も固定");
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 0);
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
    let before = progress_count(&h);
    let notes = t.notes.borrow().len();
    choose(&mut h);
    assert_eq!(t.last(), (h.next_tick(), false), "待たない印は偽");
    for i in 1..=3 {
        fire(&mut h, &t);
        assert_eq!(
            progress_count(&h),
            before + i,
            "1 件ずつ（連続適用に回らない）"
        );
        assert_eq!(count(&h.view(), ACTIVE_TESTID), 0);
    }
    // 固定の間の宛先つきの通知はすべて待たない印が偽（宛先なしの通知は固定を出た後で入になる）。
    let sent = t.notes.borrow();
    assert!(sent
        .iter()
        .skip(notes)
        .filter(|n| n.0.is_some())
        .all(|n| !n.1));
    drop(sent);
    assert_eq!(h.next_tick(), None);
}

#[test]
fn 固定のstepの文言は文字どおりで出ると入に戻る() {
    let (mut h, t) = pinned_host();
    toggle(&mut h);
    enter_pinned(&mut h, &t);
    let v = h.view();
    let mut found = Vec::new();
    find(&v, PINNED_TESTID, &mut found);
    assert_eq!(text_of(found[0]), PINNED_LABEL);
    choose(&mut h);
    fire_until_idle(&mut h, &t);
    // st-2 を出て st-3 へ。ボタンが戻り、入の印で通知する。
    assert_eq!(
        h.model()
            .session()
            .unwrap()
            .state()
            .run
            .current
            .as_ref()
            .unwrap()
            .0,
        "main/st-3"
    );
    assert_eq!(count(&h.view(), PINNED_TESTID), 0);
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 1);
    assert!(
        h.dirty_fields().contains(&FAST_FORWARD_FIELD.name),
        "出入りで dirty"
    );
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
}

#[test]
fn 固定のstepでは切り替えを送っても入切が変わらず通知も捨てもしない() {
    let (mut h, t) = pinned_host();
    toggle(&mut h);
    enter_pinned(&mut h, &t);
    choose(&mut h);
    let (notes, resets) = (t.notes.borrow().len(), t.resets.get());
    toggle(&mut h);
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
    assert_eq!(t.notes.borrow().len(), notes);
    assert_eq!(t.resets.get(), resets);
    assert!(h.dirty_fields().is_empty());
}

#[test]
fn 固定のstepの処理中の停止は通常の速さと同じ結果になる() {
    let run = |fast: bool, esc: bool| {
        let (mut h, t) = pinned_host();
        if fast {
            toggle(&mut h);
        }
        enter_pinned(&mut h, &t);
        choose(&mut h);
        fire(&mut h, &t);
        let stale = t.last().0.unwrap();
        if esc {
            h.update(EngineHost::decode_action("esc", "Escape").unwrap());
        } else {
            h.update(HostAction::Engine(Op::Control(Control::Stop)));
        }
        assert_eq!(h.next_tick(), None);
        h.update(HostAction::Engine(Op::Tick(stale)));
        assert_eq!(ignored_count(&h), 1);
        outcome(&h)
    };
    for esc in [false, true] {
        assert_eq!(run(true, esc), run(false, esc), "esc={esc}");
    }
}

#[test]
fn 固定のstepを挟んでも入と切で最後の状態と履歴が一致する() {
    let run = |fast: bool| {
        let (mut h, t) = pinned_host();
        if fast {
            toggle(&mut h);
        }
        enter_pinned(&mut h, &t);
        choose(&mut h);
        fire_until_idle(&mut h, &t);
        outcome(&h)
    };
    assert_eq!(run(false), run(true), "SHELL-27");
}

/// `st-1` の確認を外し、処理の完了そのもので固定の `st-2` へ移る `min-3step`（`pinned_host` と同じ `st-2`）。
fn pinned_after_processing_host() -> (EngineHost, Timer) {
    let mut v: serde_json::Value = serde_json::from_slice(&ir_json("")).unwrap();
    v["nodes"][0]["step"]["confirm"] = serde_json::json!({"kind": "none", "reason": "試験"});
    v["nodes"][1]["step"]["processing"] = serde_json::json!([
        {"name": "q1", "effects": []}, {"name": "q2", "effects": []}, {"name": "q3", "effects": []}
    ]);
    v["nodes"][1]["step"]["normal_speed"] = serde_json::json!(true);
    host_with(&serde_json::to_vec(&v).unwrap())
}

#[test]
fn 処理の完了で固定のstepへ移ると遷移の後の印が偽で1件ずつ進む() {
    let (mut h, t) = pinned_after_processing_host();
    toggle(&mut h);
    h.update(make_note());
    assert!(t.last().1, "固定でない間は待たない印が真");
    // 入の間は 4 件 + 1 件の 2 区切り。2 回目の進行で st-2 へ移る。
    fire(&mut h, &t);
    fire(&mut h, &t);
    let cur = h.model().session().unwrap().state().run.current.clone();
    assert_eq!(cur.unwrap().0, "main/st-2", "処理の完了で遷移した");
    assert_eq!(count(&h.view(), PINNED_TESTID), 1);
    assert_eq!(t.last(), (None, false), "遷移の後の通知は固定の判定で偽");
    // 固定の Step の進行は 1 件ずつ。連続適用に回らない。
    let before = progress_count(&h);
    choose(&mut h);
    assert_eq!(t.last(), (h.next_tick(), false));
    fire(&mut h, &t);
    assert_eq!(progress_count(&h), before + 1);
    assert!(h.next_tick().is_some());
    assert_eq!(t.last(), (h.next_tick(), false));
}

/// `pack.fast_forward` を `on` にした `min-3step`（SHELL-25）。`pinned` なら `st-2` を固定の Step にする（SHELL-26）。
fn ff_ir(on: bool, pinned: bool) -> Vec<u8> {
    let mut v: serde_json::Value = serde_json::from_slice(&ir_json("")).unwrap();
    v["pack"]["fast_forward"] = serde_json::json!(on);
    if pinned {
        v["nodes"][1]["step"]["processing"] = serde_json::json!([
            {"name": "q1", "effects": []}, {"name": "q2", "effects": []}, {"name": "q3", "effects": []}
        ]);
        v["nodes"][1]["step"]["normal_speed"] = serde_json::json!(true);
    }
    serde_json::to_vec(&v).unwrap()
}

#[test]
fn 教材の設定で初期値が入になり省略とfalseは切() {
    let (mut h, t) = host_with(&ff_ir(true, false));
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
    assert_eq!(count(&h.view(), TOGGLE_TESTID), 1);
    h.update(make_note());
    assert!(t.last().1, "初期値の入でも待たない印で通知する");
    fire(&mut h, &t);
    assert_eq!(progress_count(&h), FAST_FORWARD_LIMIT as usize);
    for bytes in [ff_ir(false, false), ir_json("")] {
        let (h, _) = host_with(&bytes);
        assert_eq!(status(&h), (OFF_LABEL.to_owned(), "off".to_owned()));
    }
}

#[test]
fn 設定ありでも切り替えは_ops_にも保存にも出ない() {
    let (mut h, _) = host_with(&ff_ir(true, false));
    toggle(&mut h);
    assert_eq!(status(&h), (OFF_LABEL.to_owned(), "off".to_owned()));
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
fn 作り直すと切り替えた値でなく教材の初期値に戻る() {
    for on in [true, false] {
        let (mut h, t) = host_with(&ff_ir(on, false));
        toggle(&mut h);
        h.update(make_note());
        fire_until_idle(&mut h, &t);
        let saved = h.history_to_save().unwrap();
        // 読み込み直し相当: 同じ教材から作り直し、保存した履歴で起動する。
        let (mut again, _) = host_with(&ff_ir(on, false));
        assert_eq!(again.boot(Some(saved.as_bytes())), BootKind::Restored);
        let want = if on { ON_LABEL } else { OFF_LABEL };
        assert_eq!(status(&again).0, want, "on={on}");
        // 履歴の差し替えでは今の値を戻さない（`host.rs` の `fast_forward` の規則）。
        let exported = again.history_to_save().unwrap();
        toggle(&mut again);
        assert_eq!(again.open_history(exported.as_bytes()), ImportOutcome::Open);
        let flipped = if on { OFF_LABEL } else { ON_LABEL };
        assert_eq!(status(&again).0, flipped, "on={on}");
    }
}

#[test]
fn 設定ありでも固定のstepは通常の速さで進む() {
    let (mut h, t) = host_with(&ff_ir(true, true));
    enter_pinned(&mut h, &t);
    assert_eq!(count(&h.view(), PINNED_TESTID), 1);
    assert_eq!(status(&h), (ON_LABEL.to_owned(), "on".to_owned()));
    let before = progress_count(&h);
    choose(&mut h);
    assert_eq!(t.last(), (h.next_tick(), false), "待たない印は偽");
    fire(&mut h, &t);
    assert_eq!(progress_count(&h), before + 1, "1 件ずつ");
}
