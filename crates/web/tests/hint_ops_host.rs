//! 上部のヒントを 1 段ずつ開く操作と表示（TASK-530 1/2。SHELL-14・GUIDE-17）の host の層の試験。
//!
//! 出す条件・押下の復号・段の増え方・Step ごとの鍵・dirty・検査器と、上限を超える押下の断り（TASK-530 2/2）を固定する。
//! 画面の段数は web が `min(total)` で丸めるため、上限の判定は DOM ではなく core の状態（`hints_opened_at`）で行う。
//! ブラウザでの実際の押下は `tools/browser/hint-open.test.mjs`、通信の計数は `egress-count.test.mjs` の担当である。
//! 試験では `unwrap` を使ってよい。

use std::path::PathBuf;

use fandhe_frontend_core::Node;
use fandhe_frontend_interactive::DirtyTracked;
use fandhe_workshop_contract::ir::Ir;
use fandhe_workshop_core::addr::StepUid;
use fandhe_workshop_core::event::{ControlKind, Event, RefuseReason};
use fandhe_workshop_core::input::{Control, Op};
use fandhe_workshop_core::session::SessionState;
use fandhe_workshop_core::{load_ir, Session};
use fandhe_workshop_web::a11y;
use fandhe_workshop_web::engine_parts::standard_registry;
use fandhe_workshop_web::hint_ops::{
    ACTION_OPEN_HINT, HINT_OPS_FIELD, OPEN_TESTID, STAGE_TESTID, TEXT_TESTID,
};
use fandhe_workshop_web::host::{EngineHost, HostAction};
use fandhe_workshop_web::parts::Component;
use fandhe_workshop_web::shell::{self, TopSection};
use serde_json::{json, Value};

fn ir_value() -> Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ch0_3/ir.json");
    serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap()
}

fn ir() -> Ir {
    load_ir(&serde_json::to_vec(&ir_value()).unwrap()).unwrap()
}

/// `main/st-0-1` に 3 つのヒントを持たせた IR。
fn ir3() -> Ir {
    let mut v = ir_value();
    for (id, t) in [
        ("h-a", "一段目"),
        ("h-b", "<b>二段目</b>"),
        ("h-c", "三段目"),
    ] {
        v["hints"][id] = json!({"level": 1, "text": t});
    }
    for n in v["nodes"].as_array_mut().unwrap() {
        if n["uid"] == "main/st-0-1" {
            n["step"]["hints"] = json!(["h-a", "h-b", "h-c"]);
        }
    }
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

/// `ir3` に加え、水準 C だけで出す 4 つ目のヒントを持つ IR。既定の案内レベルでは 3 段が上限になる。
fn ir4_hidden() -> Ir {
    let mut v = ir_value();
    for (id, t) in [
        ("h-a", "一段目"),
        ("h-b", "<b>二段目</b>"),
        ("h-c", "三段目"),
        ("h-d", "伏せる四段目"),
    ] {
        v["hints"][id] = json!({"level": 1, "text": t});
    }
    v["hints"]["h-d"]["levels"] = json!(["C"]);
    for n in v["nodes"].as_array_mut().unwrap() {
        if n["uid"] == "main/st-0-1" {
            n["step"]["hints"] = json!(["h-a", "h-b", "h-c", "h-d"]);
        }
    }
    load_ir(&serde_json::to_vec(&v).unwrap()).unwrap()
}

fn state_at(uid: &str) -> SessionState {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/contract/event-sequence.json");
    let s: Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
    let text = serde_json::to_string(&s["initial"])
        .unwrap()
        .replace("\"main/st-1\"", &format!("\"{uid}\""));
    serde_json::from_str(&text).unwrap()
}

fn host_with(ir: Ir, uid: &str) -> EngineHost {
    EngineHost::with_session(Session::from_state(ir, state_at(uid)), standard_registry()).unwrap()
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

fn count(n: &Node, testid: &str) -> usize {
    usize::from(attr(n, "data-testid") == Some(testid))
        + kids(n).iter().map(|c| count(c, testid)).sum::<usize>()
}

fn all_text(n: &Node, out: &mut Vec<String>) {
    if let Node::Text(t) = n {
        out.push(t.clone());
    }
    for c in kids(n) {
        all_text(c, out);
    }
}

fn texts_of(n: &Node, testid: &str, out: &mut Vec<String>) {
    if attr(n, "data-testid") == Some(testid) {
        let mut t = Vec::new();
        all_text(n, &mut t);
        out.push(t.concat());
    }
    for c in kids(n) {
        texts_of(c, testid, out);
    }
}

fn stage_texts(h: &EngineHost) -> Vec<String> {
    let mut v = Vec::new();
    texts_of(&h.view(), TEXT_TESTID, &mut v);
    v
}

fn open(h: &mut EngineHost) {
    h.update(EngineHost::decode_action(ACTION_OPEN_HINT, "").unwrap());
}

fn opened(h: &EngineHost, uid: &str) -> u32 {
    h.model()
        .session()
        .unwrap()
        .hints_opened_at(&StepUid(uid.to_owned()))
}

#[test]
fn ヒントのある_step_だけボタンが出て_session_が無いときは0件() {
    let v = host_with(ir(), "main/st-0-1").view();
    assert_eq!(count(&v, OPEN_TESTID), 1);
    assert_eq!(count(&v, STAGE_TESTID), 0);
    assert_eq!(count(&v, TopSection::Hint.testid()), 1);
    let v = host_with(ir(), "main/st-0-2a").view();
    assert_eq!(count(&v, OPEN_TESTID), 0);
    assert_eq!(count(&v, TopSection::Hint.testid()), 1);
    let v = EngineHost::new().view();
    assert_eq!(count(&v, OPEN_TESTID), 0);
    assert_eq!(count(&v, TopSection::Hint.testid()), 1);
}

#[test]
fn 押下で段が1つ増え文面が_ir_と一致し_dirty_が積まれる() {
    let mut h = host_with(ir(), "main/st-0-1");
    let _ = h.dirty_fields();
    open(&mut h);
    assert_eq!(opened(&h, "main/st-0-1"), 1);
    assert!(h.dirty_fields().contains(&HINT_OPS_FIELD.name));
    assert_eq!(stage_texts(&h), vec!["右側を選ぶ場面です。".to_owned()]);
}

#[test]
fn 三段は定義の順に1つずつ増え_ボタンの要素は残る() {
    let mut h = host_with(ir3(), "main/st-0-1");
    let want = ["一段目", "<b>二段目</b>", "三段目"];
    for k in 1..=3 {
        open(&mut h);
        let got = stage_texts(&h);
        assert_eq!(got, want[..k].to_vec());
        assert_eq!(count(&h.view(), OPEN_TESTID), 1);
    }
}

#[test]
fn step_を移るとその_step_の段だけが出て戻ると元の段数が出る() {
    let mut h = host_with(ir3(), "main/st-0-1");
    open(&mut h);
    open(&mut h);
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-1-1".to_owned(),
    )))));
    assert_eq!(count(&h.view(), STAGE_TESTID), 0);
    h.update(HostAction::Engine(Op::Control(Control::GoTo(StepUid(
        "main/st-0-1".to_owned(),
    )))));
    assert_eq!(count(&h.view(), STAGE_TESTID), 2);
}

#[test]
fn 空でない_payload_は復号で断る() {
    for p in ["1", " ", "x", "<b>"] {
        assert!(
            EngineHost::decode_action(ACTION_OPEN_HINT, p).is_none(),
            "{p:?}"
        );
    }
    assert!(EngineHost::decode_action(ACTION_OPEN_HINT, "").is_some());
}

#[test]
fn ヒントの無い_step_での押下は状態を変えない() {
    let mut h = host_with(ir(), "main/st-0-2a");
    let before = serde_json::to_string(h.model().session().unwrap().state()).unwrap();
    open(&mut h);
    let after = serde_json::to_string(h.model().session().unwrap().state()).unwrap();
    assert_eq!(before, after);
}

#[test]
fn 完成した画面は検査器を通り区画は上部の中に1個() {
    let mut h = host_with(ir3(), "main/st-0-1");
    open(&mut h);
    let v = h.view();
    assert!(shell::check_complete_v2(&v).is_ok());
    assert!(a11y::check_no_undefined(&v).is_ok());
    assert_eq!(count(&v, TopSection::Hint.testid()), 1);
}

#[test]
fn 上限を超える押下は段を増やさず_nohint_で断る() {
    let mut h = host_with(ir4_hidden(), "main/st-0-1");
    for _ in 0..3 {
        open(&mut h);
    }
    assert_eq!(opened(&h, "main/st-0-1"), 3);
    // 拒否は log に追記されるため、log を除いた状態（run・hints_opened ほか）の同一で比べる。
    let without_log = |h: &EngineHost| {
        let mut v = serde_json::to_value(h.model().session().unwrap().state()).unwrap();
        v.as_object_mut().unwrap().remove("log");
        v
    };
    let state_before = without_log(&h);
    let logs_before = h.model().session().unwrap().log().len();
    for _ in 0..4 {
        open(&mut h);
    }
    // 状態（開いた段数）で判定する。画面の段数は web が丸めるため、core の上限が壊れても DOM は変わらない。
    assert_eq!(opened(&h, "main/st-0-1"), 3);
    assert_eq!(state_before, without_log(&h));
    let log = h.model().session().unwrap().log();
    assert_eq!(log.len(), logs_before + 4);
    for e in &log[logs_before..] {
        assert!(matches!(
            e,
            Event::Refused {
                op: ControlKind::OpenHint,
                reason: RefuseReason::NoHint,
                ..
            }
        ));
    }
    let want = ["一段目", "<b>二段目</b>", "三段目"];
    assert_eq!(stage_texts(&h), want.to_vec());
    let v = h.view();
    assert_eq!(count(&v, STAGE_TESTID), 3);
    assert_eq!(count(&v, OPEN_TESTID), 1);
    let mut all = Vec::new();
    all_text(&v, &mut all);
    assert!(!all.concat().contains("伏せる四段目"));
}
